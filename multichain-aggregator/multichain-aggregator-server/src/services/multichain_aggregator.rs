use crate::{
    proto::{multichain_aggregator_service_server::MultichainAggregatorService, *},
    services::utils::{parse_query, parse_query_2},
    settings::ApiSettings,
};
use actix_phoenix_channel::ChannelBroadcaster;
use api_client_framework::HttpApiClient;
use blockscout_service_launcher::database::ReadWriteRepo;
use multichain_aggregator_logic::{
    clients::dapp,
    error::{ParseError, ServiceError},
    services::{
        api_key_manager::ApiKeyManager,
        chains, import,
        search::{self, SearchContext, UniformChainSearchCache},
    },
    types,
};
use std::{collections::HashSet, sync::Arc};
use tonic::{Request, Response, Status};

pub struct MultichainAggregator {
    repo: Arc<ReadWriteRepo>,
    api_key_manager: ApiKeyManager,
    dapp_client: HttpApiClient,
    token_info_client: HttpApiClient,
    bens_client: HttpApiClient,
    api_settings: ApiSettings,
    quick_search_chains: Vec<types::ChainId>,
    bens_protocols: Option<Vec<String>>,
    domain_primary_chain_id: types::ChainId,
    marketplace_enabled_cache: chains::MarketplaceEnabledCache,
    channel_broadcaster: ChannelBroadcaster,
    uniform_chain_search_cache: Option<UniformChainSearchCache>,
}

impl MultichainAggregator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo: Arc<ReadWriteRepo>,
        dapp_client: HttpApiClient,
        token_info_client: HttpApiClient,
        bens_client: HttpApiClient,
        api_settings: ApiSettings,
        quick_search_chains: Vec<types::ChainId>,
        bens_protocols: Option<Vec<String>>,
        domain_primary_chain_id: types::ChainId,
        marketplace_enabled_cache: chains::MarketplaceEnabledCache,
        channel_broadcaster: ChannelBroadcaster,
        uniform_chain_search_cache: Option<UniformChainSearchCache>,
    ) -> Self {
        Self {
            api_key_manager: ApiKeyManager::new(repo.main_db().clone()),
            repo,
            dapp_client,
            token_info_client,
            bens_client,
            api_settings,
            quick_search_chains,
            bens_protocols,
            domain_primary_chain_id,
            marketplace_enabled_cache,
            channel_broadcaster,
            uniform_chain_search_cache,
        }
    }

    fn normalize_page_size(&self, size: Option<u32>) -> u32 {
        size.unwrap_or(self.api_settings.default_page_size)
            .clamp(1, self.api_settings.max_page_size)
    }

    // If `chain_ids` is empty, meaning no filter is applied,
    // we default to include all active chains.
    // Otherwise, we validate that `chain_ids` only include the active ones.
    async fn validate_and_prepare_chain_ids(
        &self,
        chain_ids: Vec<types::ChainId>,
    ) -> Result<Vec<types::ChainId>, ServiceError> {
        let active_chain_ids = chains::list_repo_chains_cached(self.repo.read_db(), true)
            .await?
            .into_iter()
            .map(|c| c.id);

        let chain_ids: Vec<_> = if chain_ids.is_empty() {
            active_chain_ids.collect()
        } else {
            let active_chain_ids = active_chain_ids.collect::<HashSet<_>>();
            let unsupported_chain_ids = chain_ids
                .iter()
                .filter(|chain_id| !active_chain_ids.contains(chain_id))
                .map(|id| id.to_string())
                .collect::<Vec<_>>();

            if !unsupported_chain_ids.is_empty() {
                return Err(ParseError::Custom(format!(
                    "unsupported chain ids provided: {}",
                    unsupported_chain_ids.join(", ")
                ))
                .into());
            }

            chain_ids
        };

        Ok(chain_ids)
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
        let inner = request.into_inner();

        let chain_id = inner.chain_id.map(parse_query).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);
        let page_token = inner.page_token.map(parse_query_2).transpose()?;

        let chain_ids = self
            .validate_and_prepare_chain_ids(chain_id.map(|v| vec![v]).unwrap_or_default())
            .await?;

        let (addresses, next_page_token) = search::search_addresses(
            self.repo.read_db(),
            &self.bens_client,
            search::AddressSearchConfig::GeneralSearch {
                bens_protocols: self.bens_protocols.as_deref(),
                domain_primary_chain_id: self.domain_primary_chain_id,
                // NOTE: resolve to a primary domain. Multi-TLD resolution is not supported yet.
                bens_domain_lookup_limit: 1,
            },
            inner.q,
            chain_ids,
            page_size as u64,
            page_token,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to list addresses");
            Status::internal("failed to list addresses")
        })?;

        Ok(Response::new(ListAddressesResponse {
            items: addresses.into_iter().map(|a| a.into()).collect(),
            next_page_params: next_page_token.map(|(a, c)| Pagination {
                page_token: format!("{},{}", a.to_checksum(None), c),
                page_size,
            }),
        }))
    }

    async fn list_nfts(
        &self,
        request: Request<ListNftsRequest>,
    ) -> Result<Response<ListNftsResponse>, Status> {
        let inner = request.into_inner();

        let chain_id = inner.chain_id.map(parse_query).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);
        let page_token = inner.page_token.map(parse_query_2).transpose()?;

        let chain_ids = self
            .validate_and_prepare_chain_ids(chain_id.map(|v| vec![v]).unwrap_or_default())
            .await?;

        let (addresses, next_page_token) = search::search_addresses(
            self.repo.read_db(),
            &self.bens_client,
            search::AddressSearchConfig::NFTSearch {
                domain_primary_chain_id: self.domain_primary_chain_id,
            },
            inner.q,
            chain_ids,
            page_size as u64,
            page_token,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to list nfts");
            Status::internal("failed to list nfts")
        })?;

        Ok(Response::new(ListNftsResponse {
            items: addresses.into_iter().map(|a| a.into()).collect(),
            next_page_params: next_page_token.map(|(a, c)| Pagination {
                page_token: format!("{},{}", a.to_checksum(None), c),
                page_size,
            }),
        }))
    }

    async fn list_transactions(
        &self,
        request: Request<ListTransactionsRequest>,
    ) -> Result<Response<ListTransactionsResponse>, Status> {
        let inner = request.into_inner();

        let chain_id = inner.chain_id.map(parse_query).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);
        let page_token = inner.page_token.map(parse_query).transpose()?;

        let chain_ids = self
            .validate_and_prepare_chain_ids(chain_id.map(|v| vec![v]).unwrap_or_default())
            .await?;

        let (transactions, next_page_token) = search::search_hashes(
            self.repo.read_db(),
            inner.q,
            Some(types::hashes::HashType::Transaction),
            chain_ids,
            page_size as u64,
            page_token,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to list transactions");
            Status::internal("failed to list transactions")
        })?;

        Ok(Response::new(ListTransactionsResponse {
            items: transactions.into_iter().map(|t| t.into()).collect(),
            next_page_params: next_page_token.map(|c| Pagination {
                page_token: c.to_string(),
                page_size,
            }),
        }))
    }

    async fn list_blocks(
        &self,
        request: Request<ListBlocksRequest>,
    ) -> Result<Response<ListBlocksResponse>, Status> {
        let inner = request.into_inner();

        let chain_id = inner.chain_id.map(parse_query).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);
        let page_token = inner.page_token.map(parse_query).transpose()?;

        let chain_ids = self
            .validate_and_prepare_chain_ids(chain_id.map(|v| vec![v]).unwrap_or_default())
            .await?;

        let (blocks, next_page_token) = search::search_hashes(
            self.repo.read_db(),
            inner.q,
            Some(types::hashes::HashType::Block),
            chain_ids,
            page_size as u64,
            page_token,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to list blocks");
            Status::internal("failed to list blocks")
        })?;

        Ok(Response::new(ListBlocksResponse {
            items: blocks.into_iter().map(|t| t.into()).collect(),
            next_page_params: next_page_token.map(|c| Pagination {
                page_token: c.to_string(),
                page_size,
            }),
        }))
    }

    async fn list_block_numbers(
        &self,
        request: Request<ListBlockNumbersRequest>,
    ) -> Result<Response<ListBlockNumbersResponse>, Status> {
        let inner = request.into_inner();

        let chain_id = inner.chain_id.map(parse_query).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);
        let page_token = inner.page_token.map(parse_query).transpose()?;

        let chain_ids = self
            .validate_and_prepare_chain_ids(chain_id.map(|v| vec![v]).unwrap_or_default())
            .await?;

        let (block_numbers, next_page_token) = search::search_block_numbers(
            self.repo.read_db(),
            inner.q,
            chain_ids,
            page_size as u64,
            page_token,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to list block numbers");
            Status::internal("failed to list block numbers")
        })?;

        Ok(Response::new(ListBlockNumbersResponse {
            items: block_numbers.into_iter().map(|b| b.into()).collect(),
            next_page_params: next_page_token.map(|c| Pagination {
                page_token: c.to_string(),
                page_size,
            }),
        }))
    }

    async fn quick_search(
        &self,
        request: Request<QuickSearchRequest>,
    ) -> Result<Response<QuickSearchResponse>, Status> {
        let inner = request.into_inner();

        let context = SearchContext {
            db: Arc::clone(&self.repo),
            dapp_client: &self.dapp_client,
            token_info_client: &self.token_info_client,
            bens_client: &self.bens_client,
            bens_protocols: self.bens_protocols.as_deref(),
            domain_primary_chain_id: self.domain_primary_chain_id,
            marketplace_enabled_cache: &self.marketplace_enabled_cache,
            uniform_chain_search_cache: self.uniform_chain_search_cache.as_ref(),
        };

        let results = search::quick_search(inner.q, &self.quick_search_chains, &context)
            .await
            .inspect_err(|err| {
                tracing::error!(error = ?err, "failed to quick search");
            })?;

        Ok(Response::new(results.into()))
    }

    async fn list_tokens(
        &self,
        request: Request<ListTokensRequest>,
    ) -> Result<Response<ListTokensResponse>, Status> {
        let inner = request.into_inner();

        let chain_ids = inner
            .chain_id
            .into_iter()
            .map(parse_query)
            .collect::<Result<Vec<_>, _>>()?;
        let page_size = self.normalize_page_size(inner.page_size);

        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;

        let (tokens, next_page_token) = search::search_tokens(
            self.repo.read_db(),
            &self.token_info_client,
            inner.q.to_string(),
            chain_ids,
            page_size as u64,
            inner.page_token,
        )
        .await?;

        Ok(Response::new(ListTokensResponse {
            items: tokens.into_iter().map(|t| t.into()).collect(),
            next_page_params: next_page_token.map(|page_token| Pagination {
                page_token,
                page_size,
            }),
        }))
    }

    async fn list_dapps(
        &self,
        request: Request<ListDappsRequest>,
    ) -> Result<Response<ListDappsResponse>, Status> {
        let inner = request.into_inner();

        let chain_ids = if inner.chain_ids.is_empty() {
            chains::list_active_chains_cached(
                self.repo.read_db(),
                &[chains::ChainSource::Dapp {
                    dapp_client: &self.dapp_client,
                }],
            )
            .await?
            .into_iter()
            .map(|c| c.id)
            .collect()
        } else {
            inner
                .chain_ids
                .into_iter()
                .map(parse_query)
                .collect::<Result<Vec<_>, _>>()?
        };

        let chain_ids = self
            .marketplace_enabled_cache
            .filter_marketplace_enabled_chains(chain_ids, |id| *id)
            .await;

        if chain_ids.is_empty() {
            return Ok(Response::new(ListDappsResponse { items: vec![] }));
        }

        let dapps =
            search::search_dapps(&self.dapp_client, inner.q, inner.categories, chain_ids).await?;

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
        let inner = request.into_inner();

        let page_size = self.normalize_page_size(inner.page_size);

        let (domains, next_page_token) = search::search_domains(
            &self.bens_client,
            inner.q,
            self.bens_protocols.as_deref(),
            self.domain_primary_chain_id,
            page_size,
            inner.page_token,
        )
        .await?;

        Ok(Response::new(ListDomainsResponse {
            items: domains.into_iter().map(|d| d.into()).collect(),
            next_page_params: next_page_token.map(|page_token| Pagination {
                page_token,
                page_size,
            }),
        }))
    }
}
