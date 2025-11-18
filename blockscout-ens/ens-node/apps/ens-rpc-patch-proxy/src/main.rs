use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use reqwest::Client;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    net::SocketAddr,
    str::FromStr,
    sync::{Arc},
    sync::atomic::{AtomicUsize, Ordering},
};
use tokio::signal;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{fmt, EnvFilter};

const MAX_RPC_RETRIES: usize = 5;

#[derive(Clone)]
struct AppState {
    rpcs: Arc<Vec<String>>,
    current_rpc_index: Arc<AtomicUsize>,
    client: Client,
    patch_disabled: bool,
}

#[tokio::main]
async fn main() {
    // Logging
    let _guard = fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .compact()
        .init();

    // --- Read RPC list (required)
    let rpcs_env = std::env::var("UPSTREAM_RPC")
        .expect("UPSTREAM_RPC must be set (comma-separated list of RPC URLs)");
    let rpcs: Vec<String> = rpcs_env
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if rpcs.is_empty() {
        panic!("UPSTREAM_RPC must contain at least one valid URL");
    }

    // Validate URLs early
    for (i, rpc) in rpcs.iter().enumerate() {
        if let Err(e) = reqwest::Url::from_str(rpc) {
            panic!("Invalid RPC URL at index {i}: {rpc} — {e}");
        }
    }

    let patch_disabled = std::env::var("PATCH_DISABLED")
        .unwrap_or_else(|_| "false".into())
        .eq_ignore_ascii_case("true");

    let bind_addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8545".into())
        .parse()
        .expect("invalid BIND_ADDR");

    info!("RPC backends: {:?}", rpcs);
    info!("Patch disabled: {}", patch_disabled);
    info!("Binding on: {}", bind_addr);

    let client = Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .pool_max_idle_per_host(32)
        .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
        .build()
        .expect("failed to build reqwest client");

    let state = Arc::new(AppState {
        rpcs: Arc::new(rpcs),
        current_rpc_index: Arc::new(AtomicUsize::new(0)),
        client,
        patch_disabled,
    });

    let app = Router::new()
        .route("/health", get(|| async { (StatusCode::OK, "ok") }))
        .route("/", post(proxy))
        .route("/rpc", post(proxy))
        .with_state(state.clone());

    info!("Listening on {bind_addr}");
    axum::serve(
        tokio::net::TcpListener::bind(bind_addr).await.expect("bind failed"),
        app,
    )
    .with_graceful_shutdown(async {
        let _ = signal::ctrl_c().await;
        info!("Shutdown signal received");
    })
    .await
    .expect("server error");
}

async fn proxy(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let mut attempt = 0;
    let mut idx = state.current_rpc_index.load(Ordering::SeqCst);
    let total_rpcs = state.rpcs.len();

    let mut last_err: Option<String> = None;

    // --- Try up to MAX_RPC_RETRIES or until all RPCs tried
    while attempt < MAX_RPC_RETRIES {
        let rpc_url = &state.rpcs[idx % total_rpcs];
        attempt += 1;
        debug!("→ [{}] Sending request (attempt {attempt}) to {}", idx, rpc_url);

        match state
            .client
            .post(rpc_url)
            .headers(filter_headers(&headers))
            .body(body.clone())
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let status = resp.status();
                let upstream_text = match resp.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        last_err = Some(format!("upstream read failed: {e}"));
                        break;
                    }
                };

                // Update current index
                state.current_rpc_index.store(idx % total_rpcs, Ordering::SeqCst);
                debug!("✓ Request served successfully after {attempt} RPC attempts");

                let patched_text = if state.patch_disabled {
                    upstream_text
                } else {
                    patch_if_needed(&body, upstream_text)
                };

                return Response::builder()
                    .status(status)
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(patched_text))
                    .unwrap();
            }
            Ok(bad_resp) => {
                warn!("RPC {} returned HTTP {}", rpc_url, bad_resp.status());
                last_err = Some(format!("HTTP {}", bad_resp.status()));
            }
            Err(e) => {
                warn!("RPC {} failed: {}", rpc_url, e);
                last_err = Some(e.to_string());
            }
        }

        // move to next RPC
        idx = (idx + 1) % total_rpcs;
    }

    error!(
        "❌ All {} attempts failed ({} RPCs). Last error: {:?}",
        attempt, total_rpcs, last_err
    );

    (
        StatusCode::BAD_GATEWAY,
        Json(json!({
            "jsonrpc":"2.0",
            "error":{"code":-32000,"message":format!("All RPCs failed after {attempt} attempts: {:?}", last_err)},
            "id":null
        })),
    )
        .into_response()
}

// ---- Helper functions ----

fn patch_if_needed(req_body: &str, upstream_text: String) -> String {
    let maybe_req_json: Result<Value, _> = serde_json::from_str(req_body);
    let maybe_resp_json: Result<Value, _> = serde_json::from_str(&upstream_text);
    match (maybe_req_json, maybe_resp_json) {
        (Ok(req_json), Ok(mut resp_json)) => {
            patch_jsonrpc_if_needed(&req_json, &mut resp_json);
            serde_json::to_string(&resp_json).unwrap_or(upstream_text)
        }
        _ => upstream_text,
    }
}

fn filter_headers(incoming: &HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    out.insert("content-type", "application/json".parse().unwrap());
    if let Some(v) = incoming.get("authorization") {
        out.insert("authorization", v.clone());
    }
    out
}

// ---- Patching logic (unchanged) ----
fn patch_jsonrpc_if_needed(req: &Value, resp: &mut Value) {
    match (req, resp) {
        (Value::Object(req_obj), Value::Object(resp_obj)) => {
            if method_is_get_logs(req_obj) {
                if let Some(result) = resp_obj.get_mut("result") {
                    if result.is_array() {
                        patch_logs_array(result);
                    }
                }
            }
        }
        (Value::Array(reqs), Value::Array(resps)) => {
            let mut id_is_getlogs: HashMap<Value, bool> = HashMap::new();
            for r in reqs {
                if let Some((id, is_get)) = req_id_and_is_getlogs(r) {
                    id_is_getlogs.insert(id, is_get);
                }
            }
            for r in resps.iter_mut() {
                if let Some(id) = r.get("id").cloned() {
                    if id_is_getlogs.get(&id).copied().unwrap_or(false) {
                        if let Some(res) = r.get_mut("result") {
                            if res.is_array() {
                                patch_logs_array(res);
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn method_is_get_logs(req_obj: &serde_json::Map<String, Value>) -> bool {
    matches!(req_obj.get("method"), Some(Value::String(m)) if m == "eth_getLogs")
}

fn req_id_and_is_getlogs(req_val: &Value) -> Option<(Value, bool)> {
    if let Value::Object(o) = req_val {
        let is = method_is_get_logs(o);
        if let Some(id) = o.get("id") {
            return Some((id.clone(), is));
        }
    }
    None
}

fn patch_logs_array(arr: &mut Value) {
    let Some(logs) = arr.as_array_mut() else { return };
    if logs.is_empty() {
        return;
    }
    
    // Store original indices for stable sorting when logIndex values are equal
    let mut indexed_logs: Vec<(usize, Value)> = logs.drain(..).enumerate().map(|(i, v)| (i, v)).collect();
    // indexed_logs.sort_by(|(i_a, a), (i_b, b)| {
    //     let key_a = log_sort_key(a);
    //     let key_b = log_sort_key(b);
    //     // If keys are equal, use original index as tiebreaker for stable sort
    //     key_a.cmp(&key_b).then_with(|| i_a.cmp(i_b))
    // });
    
    // Rebuild the array in sorted order
    logs.extend(indexed_logs.into_iter().map(|(_, log)| log));

    let mut current_block: Option<u64> = None;
    let mut current_tx: Option<u64> = None;
    let mut next_idx: u64 = 0;
    for log in logs.iter_mut() {
        let block = get_hex_u64(log.get("blockNumber"));
        let tx = get_hex_u64(log.get("transactionIndex"));
        
        if current_block != Some(block) {
            current_block = Some(block);
            current_tx = None;
            next_idx = 0;
        }
        
        // Reset logIndex when we move to a new transaction within the same block
        if current_tx != Some(tx) {
            current_tx = Some(tx);
            // Don't reset next_idx - logIndex should be sequential across all transactions in a block
            // This matches Ethereum's behavior where logIndex is block-scoped, not transaction-scoped
        }
        
        if let Some(obj) = log.as_object_mut() {
            obj.insert("logIndex".to_string(), Value::String(format!("0x{:x}", next_idx)));
        }
        next_idx += 1;
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct LogKey(u64, u64, i64);

fn log_sort_key(v: &Value) -> LogKey {
    let log_index_str = v.get("logIndex")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    
    // For logs with "0xffffffff", treat as 0 since they always appear first
    // This is a marker used by some RPC nodes to indicate the first log
    let log_index = if log_index_str == "0xffffffff" || log_index_str == "0xFFFFFFFF" {
        -1
    } else {
        get_hex_u64(v.get("logIndex")) as i64
    };
    
    LogKey(
        get_hex_u64(v.get("blockNumber")),
        get_hex_u64(v.get("transactionIndex")),
        log_index,
    )
}

fn get_hex_u64(v: Option<&Value>) -> u64 {
    match v {
        Some(Value::String(s)) => parse_hex_u64(s).unwrap_or(0),
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        _ => 0,
    }
}

fn parse_hex_u64(s: &str) -> Option<u64> {
    let s = s.trim();
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    u64::from_str_radix(stripped, 16).ok()
}
