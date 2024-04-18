use celestia_rpc::{Error, Result};
use http::{header, HeaderValue};
use jsonrpsee::{
    http_client::{HeaderMap, HttpClientBuilder},
    ws_client::WsClientBuilder,
};

/// The maximum request size in the default client in celestia_rpc is not sufficient for some blocks,
/// therefore, we need to customize client initialization
pub async fn new_celestia_client(
    conn_str: &str,
    auth_token: Option<&str>,
    max_request_size: u32,
    max_response_size: u32,
) -> Result<celestia_rpc::Client> {
    let mut headers = HeaderMap::new();

    if let Some(token) = auth_token {
        let val = HeaderValue::from_str(&format!("Bearer {token}"))?;
        headers.insert(header::AUTHORIZATION, val);
    }

    let protocol = conn_str.split_once(':').map(|(proto, _)| proto);
    let client = match protocol {
        Some("http") | Some("https") => celestia_rpc::Client::Http(
            HttpClientBuilder::default()
                .set_headers(headers)
                .max_request_size(max_request_size)
                .max_response_size(max_response_size)
                .build(conn_str)?,
        ),
        Some("ws") | Some("wss") => celestia_rpc::Client::Ws(
            WsClientBuilder::default()
                .set_headers(headers)
                .max_request_size(max_request_size)
                .max_response_size(max_response_size)
                .build(conn_str)
                .await?,
        ),
        _ => return Err(Error::ProtocolNotSupported(conn_str.into())),
    };

    Ok(client)
}
