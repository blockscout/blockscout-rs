use crate::{
    clients::blockscout,
    error::ServiceError,
    repository,
    services::{
        chains::{ChainSource, list_active_chains_cached},
        jobs::create_repeated_job,
    },
    types::{ChainId, tokens::TokenUpdate},
};
use api_client_framework::Endpoint;
use blockscout_service_launcher::database::ReadWriteRepo;
use futures::{StreamExt, stream};
use std::sync::Arc;
use tokio::time::Duration;
use tokio_cron_scheduler::{Job, JobSchedulerError};
use url::Url;

#[derive(Clone)]
pub struct NativeCoinUpdater {
    repo: Arc<ReadWriteRepo>,
}

impl NativeCoinUpdater {
    pub fn new(repo: Arc<ReadWriteRepo>) -> Self {
        Self { repo }
    }

    /// Creates a job that updates native coin metadata
    /// from the node API config for all active chains.
    pub fn metadata_job(
        self,
        interval: Duration,
        concurrency: usize,
    ) -> Result<Job, JobSchedulerError> {
        create_repeated_job("native coin metadata", interval, move || {
            let this = self.clone();
            async move {
                let endpoint = blockscout::node_api_config::NodeApiConfig {};
                this.fetch_and_save_updates(&endpoint, concurrency).await
            }
        })
    }

    /// Creates a job that updates native coin prices
    /// from the stats API for all active chains.
    pub fn price_job(
        self,
        interval: Duration,
        concurrency: usize,
    ) -> Result<Job, JobSchedulerError> {
        create_repeated_job("native coin prices", interval, move || {
            let this = self.clone();
            async move {
                let endpoint = blockscout::stats::Stats {};
                this.fetch_and_save_updates(&endpoint, concurrency).await
            }
        })
    }

    /// Initializes native coin data for all active chains.
    pub async fn initialize_all_native_coins(
        &self,
        concurrency: usize,
    ) -> Result<(), ServiceError> {
        tracing::info!("Initializing native coins for all chains");

        self.fetch_and_save_updates(&blockscout::node_api_config::NodeApiConfig {}, concurrency)
            .await?;
        self.fetch_and_save_updates(&blockscout::stats::Stats {}, concurrency)
            .await?;

        tracing::info!("Native coin initialization completed");
        Ok(())
    }

    pub async fn fetch_and_save_updates<T>(
        &self,
        endpoint: &T,
        concurrency: usize,
    ) -> Result<(), ServiceError>
    where
        T: Endpoint + Sync,
        (ChainId, T::Response): TryInto<TokenUpdate>,
        <(ChainId, T::Response) as TryInto<TokenUpdate>>::Error: Into<ServiceError>,
    {
        let chains =
            list_active_chains_cached(self.repo.read_db(), &[ChainSource::Repository]).await?;

        let jobs = chains.into_iter().filter_map(|c| {
            let chain_id = c.id;
            let url: Url = c.explorer_url?.parse().ok()?;
            Some(async move { fetch_update(chain_id, &url, endpoint).await.ok() })
        });

        let updates: Vec<TokenUpdate> = stream::iter(jobs)
            .buffer_unordered(concurrency)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect();

        if !updates.is_empty() {
            repository::tokens::upsert_many(self.repo.main_db(), updates).await?;
        }

        Ok(())
    }
}

#[tracing::instrument(
    skip(endpoint),
    fields(path = %endpoint.path()),
)]
async fn fetch_update<T>(
    chain_id: ChainId,
    url: &Url,
    endpoint: &T,
) -> Result<TokenUpdate, ServiceError>
where
    T: Endpoint,
    (ChainId, T::Response): TryInto<TokenUpdate>,
    <(ChainId, T::Response) as TryInto<TokenUpdate>>::Error: Into<ServiceError>,
{
    let client = blockscout::new_client(url.clone())?;
    let response = client.request(endpoint).await.inspect_err(|err| {
        tracing::warn!(
            err = ?err,
            "failed to fetch native coin update",
        );
    })?;

    (chain_id, response).try_into().map_err(Into::into)
}
