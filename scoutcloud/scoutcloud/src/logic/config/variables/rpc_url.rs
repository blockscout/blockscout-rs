use ethers::{prelude::*, providers::Provider};

use crate::logic::config::{ParsedVariable, ParsedVariableKey, UserVariable};
use anyhow::Context;
use url::Url;

pub struct RpcUrl;

#[async_trait::async_trait]
impl UserVariable<Url> for RpcUrl {
    async fn parse_from_value(url: Url) -> Result<Vec<ParsedVariable>, anyhow::Error> {
        let mut parsed = vec![];

        // check json rpc
        let provider =
            Provider::<Http>::try_from(url.as_str()).context("failed to parse url as http")?;

        let _ = provider
            .get_chainid()
            .await
            .map_err(|e| anyhow::anyhow!("failed to check health of `rpc_url`: {e}"))?;
        parsed.push((
            ParsedVariableKey::BackendEnv("ETHEREUM_JSONRPC_HTTP_URL".to_string()),
            serde_yaml::Value::String(url.to_string()),
        ));

        // check trace method
        match provider.trace_block(BlockNumber::Latest).await {
            Ok(_) => {
                parsed.push((
                    ParsedVariableKey::BackendEnv("ETHEREUM_JSONRPC_TRACE_URL".to_string()),
                    serde_yaml::Value::String(url.to_string()),
                ));
                parsed.push((
                    ParsedVariableKey::BackendEnv(
                        "INDEXER_DISABLE_INTERNAL_TRANSACTIONS_FETCHER".to_string(),
                    ),
                    serde_yaml::Value::String("true".to_string()),
                ));
            }
            Err(_) => {
                tracing::warn!("`rpc_url` does not support tracing, disabling trace url");
            }
        };

        // check websocket
        if let Some(ws_url) = get_any_healthy_ws_url(url.clone()).await? {
            parsed.push((
                ParsedVariableKey::BackendEnv("ETHEREUM_JSONRPC_WS_URL".to_string()),
                serde_yaml::Value::String(ws_url.to_string()),
            ));
        } else {
            tracing::warn!(
                "no valid websocket url found for `rpc_url`, skipping websocket configuration"
            );
        }
        Ok(parsed)
    }
}

async fn get_any_healthy_ws_url(url: Url) -> Result<Option<Url>, anyhow::Error> {
    let possible_urls = generate_possible_ws_urls(url.clone())?;
    for url in possible_urls {
        let provider = Provider::new(
            Ws::connect(url.clone())
                .await
                .context("failed to connect to ws")?,
        );
        match provider.get_chainid().await {
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
