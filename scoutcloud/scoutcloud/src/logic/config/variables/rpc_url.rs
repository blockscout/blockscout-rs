use crate::logic::{ParsedVariable, ParsedVariableKey, UserVariable};
use anyhow::Context;
use ethers::{prelude::*, providers::Provider, types::BlockNumber};
use url::Url;

pub struct RpcUrl;

#[async_trait::async_trait]
impl UserVariable<Url> for RpcUrl {
    async fn build_config_vars(url: Url) -> Result<Vec<ParsedVariable>, anyhow::Error> {
        let mut parsed = vec![];

        // check json rpc
        let provider =
            Provider::<Http>::try_from(url.as_str()).context("failed to parse url as http")?;

        check_jsonrpc_health(&provider).await?;
        parsed.push((
            ParsedVariableKey::BackendEnv("ETHEREUM_JSONRPC_HTTP_URL".to_string()),
            serde_json::Value::String(url.to_string()),
        ));

        // check trace method
        // TODO: check trace method according to node_type
        match check_any_trace_method(&provider).await {
            Ok(()) => {
                parsed.push((
                    ParsedVariableKey::BackendEnv("ETHEREUM_JSONRPC_TRACE_URL".to_string()),
                    serde_json::Value::String(url.to_string()),
                ));
                parsed.push((
                    ParsedVariableKey::BackendEnv(
                        "INDEXER_DISABLE_INTERNAL_TRANSACTIONS_FETCHER".to_string(),
                    ),
                    serde_json::Value::String("false".to_string()),
                ));
            }
            Err(err) => {
                tracing::warn!(
                    err =? err,
                    "`rpc_url` does not support tracing, disabling internal transactions"
                );
                parsed.push((
                    ParsedVariableKey::BackendEnv(
                        "INDEXER_DISABLE_INTERNAL_TRANSACTIONS_FETCHER".to_string(),
                    ),
                    serde_json::Value::String("true".to_string()),
                ));
            }
        };

        // check websocket
        if let Some(ws_url) = get_any_healthy_ws_url(url.clone()).await? {
            parsed.push((
                ParsedVariableKey::BackendEnv("ETHEREUM_JSONRPC_WS_URL".to_string()),
                serde_json::Value::String(ws_url.to_string()),
            ));
        } else {
            tracing::warn!(
                "no valid websocket url found for `rpc_url`, skipping websocket configuration"
            );
        }
        Ok(parsed)
    }
}
async fn check_jsonrpc_health(provider: &Provider<Http>) -> Result<(), anyhow::Error> {
    provider
        .get_chainid()
        .await
        .map_err(|e| anyhow::anyhow!("failed to check health of `rpc_url`: {e}"))?;

    Ok(())
}

async fn check_any_trace_method(provider: &Provider<Http>) -> Result<(), anyhow::Error> {
    let err = match provider.trace_block(BlockNumber::Latest).await {
        Ok(_) => return Ok(()),
        Err(e) => e,
    };

    if provider
        .debug_trace_block_by_number(None, GethDebugTracingOptions::default())
        .await
        .is_ok()
    {
        return Ok(());
    };

    let block = provider
        .get_block(BlockNumber::Latest)
        .await?
        .context("no blocks in blockchain")?;
    let transaction = block
        .transactions
        .first()
        .cloned()
        .context("no transactions in blockchain")?;

    if provider.trace_transaction(transaction).await.is_ok() {
        return Ok(());
    };

    Err(err)?
}

async fn get_any_healthy_ws_url(url: Url) -> Result<Option<Url>, anyhow::Error> {
    let possible_urls = generate_possible_ws_urls(url.clone())?;
    for url in possible_urls {
        match Ws::connect(url.clone())
            .await
            .context("failed to connect to ws")
        {
            Ok(_) => {
                return Ok(Some(url));
            }
            Err(e) => {
                tracing::warn!("failed to check health of `rpc_url` as websocket: {e}");
            }
        };
    }
    Ok(None)
}

fn generate_possible_ws_urls(mut url: Url) -> Result<Vec<Url>, anyhow::Error> {
    url.set_scheme("ws")
        .map_err(|_| anyhow::anyhow!("failed to set scheme to ws"))?;
    Ok(vec![url.join("ws").unwrap(), url.join("wss").unwrap(), url])
}
