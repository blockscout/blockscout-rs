use crate::{
    clients::{dapp, token_info},
    error::ServiceError,
    repository,
    types::{chains::Chain, ChainId},
};
use api_client_framework::HttpApiClient;
use blockscout_chains::BlockscoutChainsClient;
use cached::proc_macro::{cached, once};
use futures::{stream, StreamExt};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::{
    sync::RwLock,
    time::{interval, Duration, Instant},
};
use url::Url;

#[cached(
    key = "bool",
    convert = r#"{ with_active_api_keys }"#,
    time = 600, // 10 minutes
    result = true
)]
pub async fn list_repo_chains_cached(
    db: &DatabaseConnection,
    with_active_api_keys: bool,
) -> Result<Vec<Chain>, ServiceError> {
    let chains = repository::chains::list_chains(db, with_active_api_keys)
        .await?
        .into_iter()
        .map(|c| c.into())
        .collect();
    Ok(chains)
}

#[once(
    time = 600, // 10 minutes
    result = true
)]
async fn list_token_info_chains_cached(
    token_info_client: &HttpApiClient,
) -> Result<Vec<ChainId>, ServiceError> {
    let chain_ids = token_info_client
        .request(&token_info::list_chains::ListChains {})
        .await?
        .chains
        .into_iter()
        .filter_map(|id| ChainId::try_from(id).ok())
        .collect();

    Ok(chain_ids)
}

#[once(
    time = 600, // 10 minutes
    result = true
)]
async fn list_dapp_chains_cached(
    dapp_client: &HttpApiClient,
) -> Result<Vec<ChainId>, ServiceError> {
    let chain_ids = dapp_client
        .request(&dapp::list_chains::ListChains {})
        .await?
        .into_iter()
        .filter_map(|id| id.parse::<ChainId>().ok())
        .collect();

    Ok(chain_ids)
}

pub enum ChainSource<'a> {
    Repository,
    TokenInfo {
        token_info_client: &'a HttpApiClient,
    },
    Dapp {
        dapp_client: &'a HttpApiClient,
    },
}

pub async fn list_active_chains_cached(
    db: &DatabaseConnection,
    sources: &[ChainSource<'_>],
) -> Result<Vec<Chain>, ServiceError> {
    let mut chain_ids = HashSet::new();

    for source in sources {
        match source {
            ChainSource::Repository => {
                let active_repo_chain_ids = list_repo_chains_cached(db, true)
                    .await?
                    .into_iter()
                    .map(|c| c.id);
                chain_ids.extend(active_repo_chain_ids);
            }
            ChainSource::TokenInfo { token_info_client } => {
                let token_info_chain_ids = list_token_info_chains_cached(token_info_client).await?;
                chain_ids.extend(token_info_chain_ids);
            }
            ChainSource::Dapp { dapp_client } => {
                let dapp_chain_ids = list_dapp_chains_cached(dapp_client).await?;
                chain_ids.extend(dapp_chain_ids);
            }
        }
    }

    let repo_chains = list_repo_chains_cached(db, false).await?;

    let items = repo_chains
        .into_iter()
        .filter(|c| chain_ids.contains(&c.id))
        .collect::<Vec<_>>();

    Ok(items)
}

pub async fn fetch_and_upsert_blockscout_chains(
    db: &DatabaseConnection,
) -> Result<(), ServiceError> {
    let blockscout_chains = BlockscoutChainsClient::builder()
        .with_max_retries(0)
        .build()
        .fetch_all()
        .await
        .map_err(|e| anyhow::anyhow!("failed to fetch blockscout chains: {:?}", e))?
        .into_iter()
        .filter_map(|(id, chain)| {
            let id = id.parse::<i64>().ok()?;
            Some((id, chain).into())
        })
        .collect::<Vec<_>>();
    repository::chains::upsert_many(db, blockscout_chains).await?;
    Ok(())
}

pub type MarketplaceEnabledCache = Arc<RwLock<HashMap<ChainId, bool>>>;

pub fn start_marketplace_enabled_cache_updater(
    db: DatabaseConnection,
    dapp_client: HttpApiClient,
    cache: MarketplaceEnabledCache,
    update_interval: Duration,
    concurrency: usize,
) {
    let mut interval = interval(update_interval);

    tokio::spawn(async move {
        loop {
            interval.tick().await;
            let now = Instant::now();
            if let Err(err) =
                update_marketplace_enabled_cache(&db, &dapp_client, &cache, concurrency).await
            {
                tracing::error!(err = ?err, "failed to update marketplace enabled cache");
            }
            let elapsed = now.elapsed();
            tracing::info!(
                elapsed_secs = elapsed.as_secs_f32(),
                "marketplace enabled cache updated"
            );
        }
    });
}

async fn update_marketplace_enabled_cache(
    db: &DatabaseConnection,
    dapp_client: &HttpApiClient,
    cache: &MarketplaceEnabledCache,
    concurrency: usize,
) -> Result<(), ServiceError> {
    // Get chains that have at least one active dapp
    let chains = list_active_chains_cached(db, &[ChainSource::Dapp { dapp_client }]).await?;

    let res = chains.into_iter().map(|c| async move {
        let explorer_url = c.explorer_url?;
        let url = Url::parse(&explorer_url)
            .inspect_err(|err| {
                tracing::warn!(
                    explorer_url = explorer_url,
                    chain_id = c.id,
                    err = ?err,
                    "failed to parse explorer url",
                )
            })
            .ok()?;
        fetch_marketplace_enabled(&url)
            .await
            .inspect_err(|err| {
                tracing::warn!(
                    explorer_url = explorer_url,
                    chain_id = c.id,
                    err = ?err,
                    "failed to fetch chain marketplace info",
                );
            })
            .ok()
            .map(|is_enabled| (c.id, is_enabled))
    });

    // Limit the number of concurrent requests to prevent congestion
    let new_cache = stream::iter(res)
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flatten()
        .collect::<HashMap<_, _>>();

    *cache.write().await = new_cache;

    Ok(())
}

async fn fetch_marketplace_enabled(explorer_url: &Url) -> Result<bool, ServiceError> {
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    struct Envs {
        next_public_marketplace_enabled: String,
    }

    #[derive(Debug, Deserialize)]
    struct NodeApiConfig {
        envs: Envs,
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("client should be valid");

    let url = explorer_url
        .join("/node-api/config")
        .map_err(|e| ServiceError::Convert(e.into()))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("failed to fetch node-api config: {:?}", e))?
        .json::<NodeApiConfig>()
        .await
        .map_err(|e| anyhow::anyhow!("failed to parse node-api config: {:?}", e))?;

    let is_enabled = response.envs.next_public_marketplace_enabled == "true";
    Ok(is_enabled)
}
