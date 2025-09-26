use crate::{
    proto::multichain_aggregator_service_server::MultichainAggregatorService,
    services::{
        ClusterExplorer, MULTICHAIN_CLUSTER_ID,
        macros::*,
        utils::{PageTokenExtractor, page_token_to_proto, parse_chain_ids},
    },
    settings::ApiSettings,
};
use actix_phoenix_channel::ChannelBroadcaster;
use api_client_framework::HttpApiClient;
use blockscout_service_launcher::database::ReadWriteRepo;
use multichain_aggregator_logic::{
    clients::dapp,
    error::ServiceError,
    services::{api_key_manager::ApiKeyManager, chains, cluster::Cluster, dapp_search, import},
    types::{self},
};
use multichain_aggregator_proto::blockscout::multichain_aggregator::v1::*;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct MultichainAggregator {
    repo: Arc<ReadWriteRepo>,
    api_key_manager: ApiKeyManager,
    dapp_client: HttpApiClient,
    api_settings: ApiSettings,
    marketplace_enabled_cache: chains::MarketplaceEnabledCache,
    channel_broadcaster: ChannelBroadcaster,
    cluster_explorer: Arc<ClusterExplorer>,
}

impl MultichainAggregator {
    pub fn new(
        repo: Arc<ReadWriteRepo>,
        dapp_client: HttpApiClient,
        api_settings: ApiSettings,
        marketplace_enabled_cache: chains::MarketplaceEnabledCache,
        channel_broadcaster: ChannelBroadcaster,
        cluster_explorer: Arc<ClusterExplorer>,
    ) -> Self {
        Self {
            api_key_manager: ApiKeyManager::new(repo.main_db().clone()),
            repo,
            dapp_client,
            api_settings,
            marketplace_enabled_cache,
            channel_broadcaster,
            cluster_explorer,
        }
    }

    fn normalize_page_size(&self, size: Option<u32>) -> u32 {
        size.unwrap_or(self.api_settings.default_page_size)
            .clamp(1, self.api_settings.max_page_size)
    }

    #[allow(clippy::result_large_err)]
    pub fn get_multichain_cluster(&self) -> Result<&Cluster, Status> {
        self.cluster_explorer.try_get_cluster(MULTICHAIN_CLUSTER_ID)
    }
}

#[async_trait::async_trait]
impl MultichainAggregatorService for MultichainAggregator {
    async fn batch_import(
        &self,
        request: Request<BatchImportRequest>,
    ) -> Result<Response<BatchImportResponse>, Status> {
        let inner = request.into_inner();

        let api_key = (inner.api_key.as_str(), inner.chain_id.as_str()).try_into()?;
        self.api_key_manager
            .validate_api_key(api_key)
            .await
            .map_err(ServiceError::from)?;

        let import_request: types::batch_import_request::BatchImportRequest = inner.try_into()?;

        import::batch_import(
            self.repo.main_db(),
            import_request,
            self.channel_broadcaster.clone(),
        )
        .await
        .inspect_err(|err| {
            tracing::error!(error = ?err, "failed to batch import");
        })?;

        Ok(Response::new(BatchImportResponse {
            status: "ok".to_string(),
        }))
    }

    async fn list_chains(
        &self,
        request: Request<ListChainsRequest>,
    ) -> Result<Response<ListChainsResponse>, Status> {
        let inner = request.into_inner();

        let only_active = inner.only_active.unwrap_or(false);
        let chains = chains::list_repo_chains_cached(self.repo.read_db(), only_active).await?;

        let items = chains
            .into_iter()
            .filter_map(|c| c.try_into().ok())
            .collect();

        Ok(Response::new(ListChainsResponse { items }))
    }

    async fn list_addresses(
        &self,
        request: Request<ListAddressesRequest>,
    ) -> Result<Response<ListAddressesResponse>, Status> {
        paginated_multichain_endpoint!(
            self,
            request,
            search_addresses_non_aggregated,
            ListAddressesResponse
        )
    }

    async fn list_nfts(
        &self,
        request: Request<ListNftsRequest>,
    ) -> Result<Response<ListNftsResponse>, Status> {
        paginated_multichain_endpoint!(self, request, search_nfts_cached, ListNftsResponse)
    }

    async fn list_transactions(
        &self,
        request: Request<ListTransactionsRequest>,
    ) -> Result<Response<ListTransactionsResponse>, Status> {
        paginated_multichain_endpoint!(self, request, search_transactions, ListTransactionsResponse)
    }

    async fn list_blocks(
        &self,
        request: Request<ListBlocksRequest>,
    ) -> Result<Response<ListBlocksResponse>, Status> {
        paginated_multichain_endpoint!(self, request, search_blocks, ListBlocksResponse)
    }

    async fn list_block_numbers(
        &self,
        request: Request<ListBlockNumbersRequest>,
    ) -> Result<Response<ListBlockNumbersResponse>, Status> {
        paginated_multichain_endpoint!(
            self,
            request,
            search_block_numbers,
            ListBlockNumbersResponse
        )
    }

    async fn quick_search(
        &self,
        request: Request<QuickSearchRequest>,
    ) -> Result<Response<QuickSearchResponse>, Status> {
        let inner = request.into_inner();

        let cluster = self.get_multichain_cluster()?;
        let res = cluster.quick_search(inner.q, false).await?;

        Ok(Response::new(res.try_into().unwrap()))
    }

    async fn list_tokens(
        &self,
        request: Request<ListTokensRequest>,
    ) -> Result<Response<ListTokensResponse>, Status> {
        paginated_multichain_endpoint!(self, request, search_token_infos_cached, ListTokensResponse)
    }

    async fn list_dapps(
        &self,
        request: Request<ListDappsRequest>,
    ) -> Result<Response<ListDappsResponse>, Status> {
        let inner = request.into_inner();

        let cluster = self.get_multichain_cluster()?;

        let dapps = if inner.chain_ids.is_empty() {
            let chain_ids = chains::list_active_chains_cached(
                self.repo.read_db(),
                &[chains::ChainSource::Dapp {
                    dapp_client: &self.dapp_client,
                }],
            )
            .await?
            .into_iter()
            .map(|c| c.id)
            .collect();

            dapp_search::search_dapps(
                &self.dapp_client,
                inner.q,
                inner.categories,
                chain_ids,
                &self.marketplace_enabled_cache,
            )
            .await?
        } else {
            let chain_ids = parse_chain_ids(inner.chain_ids)?;
            cluster
                .search_dapps(inner.q, chain_ids, inner.categories)
                .await?
        };

        Ok(Response::new(ListDappsResponse {
            items: dapps.into_iter().map(|d| d.into()).collect(),
        }))
    }

    async fn list_dapp_chains(
        &self,
        _request: Request<ListDappChainsRequest>,
    ) -> Result<Response<ListDappChainsResponse>, Status> {
        let items = chains::list_active_chains_cached(
            self.repo.read_db(),
            &[chains::ChainSource::Dapp {
                dapp_client: &self.dapp_client,
            }],
        )
        .await?;

        let items = self
            .marketplace_enabled_cache
            .filter_marketplace_enabled_chains(items, |c| c.id)
            .await
            .into_iter()
            .filter_map(|c| c.try_into().ok())
            .collect();

        Ok(Response::new(ListDappChainsResponse { items }))
    }

    async fn list_dapp_categories(
        &self,
        _request: Request<ListDappCategoriesRequest>,
    ) -> Result<Response<ListDappCategoriesResponse>, Status> {
        let items = self
            .dapp_client
            .request(&dapp::list_categories::ListCategories {})
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to list marketplace categories");
                Status::internal("failed to list marketplace categories")
            })?;
        Ok(Response::new(ListDappCategoriesResponse { items }))
    }

    async fn list_domains(
        &self,
        request: Request<ListDomainsRequest>,
    ) -> Result<Response<ListDomainsResponse>, Status> {
        paginated_multichain_endpoint!(self, request, search_domains_cached, ListDomainsResponse)
    }
}
