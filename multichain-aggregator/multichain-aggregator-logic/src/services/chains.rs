use crate::{
    clients::{blockscout, dapp},
    error::ServiceError,
    repository,
    services::jobs::create_repeated_job,
    types::{ChainId, chains::Chain},
};
use api_client_framework::HttpApiClient;
use blockscout_chains::BlockscoutChainsClient;
use cached::proc_macro::{cached, once};
use futures::{StreamExt, stream};
use sea_orm::DatabaseConnection;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::{sync::RwLock, time::Duration};
use tokio_cron_scheduler::{Job, JobSchedulerError};
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
    let chains = repository::chains::list_by_active_api_keys(db, with_active_api_keys)
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
    Dapp { dapp_client: &'a HttpApiClient },
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

#[derive(Clone, Default)]
pub struct MarketplaceEnabledCache(Arc<RwLock<HashMap<ChainId, bool>>>);

impl MarketplaceEnabledCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn updater_job(
        self,
        db: DatabaseConnection,
        dapp_client: HttpApiClient,
        interval: Duration,
        concurrency: usize,
    ) -> Result<Job, JobSchedulerError> {
        create_repeated_job("marketplace enabled cache", interval, move || {
            // NOTE: these clones are cheap as each struct stores only Arc references
            let this = self.clone();
            let db = db.clone();
            let dapp_client = dapp_client.clone();

            async move { this.update(&db, &dapp_client, concurrency).await }
        })
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        dapp_client: &HttpApiClient,
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

        *self.0.write().await = new_cache;

        Ok(())
    }

    pub async fn filter_marketplace_enabled_chains<T>(
        &self,
        items: impl IntoIterator<Item = T>,
        get_chain_id: impl Fn(&T) -> ChainId,
    ) -> Vec<T> {
        let cache = self.0.read().await;
        items
            .into_iter()
            .filter_map(|c| {
                let is_enabled = *cache.get(&get_chain_id(&c)).unwrap_or(&false);
                if is_enabled { Some(c) } else { None }
            })
            .collect::<Vec<_>>()
    }
}

async fn fetch_marketplace_enabled(explorer_url: &Url) -> Result<bool, ServiceError> {
    let client = blockscout::new_client(explorer_url.clone())?;
    let response = client
        .request(&blockscout::node_api_config::NodeApiConfig {})
        .await?;

    let is_enabled = response.envs.next_public_marketplace_enabled.as_deref() == Some("true");
    Ok(is_enabled)
}
