use super::repo::ReadWriteRepo;
use crate::{
    proto::{multichain_aggregator_service_server::MultichainAggregatorService, *},
    settings::ApiSettings,
};
use api_client_framework::HttpApiClient;
use multichain_aggregator_logic::{
    clients::dapp,
    error::ServiceError,
    services::{api_key_manager::ApiKeyManager, chains, import, search},
    types,
};
use std::str::FromStr;
use tonic::{Request, Response, Status};

pub struct MultichainAggregator {
    repo: ReadWriteRepo,
    api_key_manager: ApiKeyManager,
    dapp_client: HttpApiClient,
    token_info_client: HttpApiClient,
    bens_client: HttpApiClient,
    api_settings: ApiSettings,
    quick_search_chains: Vec<types::ChainId>,
    bens_protocols: Option<Vec<String>>,
}

impl MultichainAggregator {
    pub fn new(
        repo: ReadWriteRepo,
        dapp_client: HttpApiClient,
        token_info_client: HttpApiClient,
        bens_client: HttpApiClient,
        api_settings: ApiSettings,
        quick_search_chains: Vec<types::ChainId>,
        bens_protocols: Option<Vec<String>>,
    ) -> Self {
        Self {
            api_key_manager: ApiKeyManager::new(repo.write_db().clone()),
            repo,
            dapp_client,
            token_info_client,
            bens_client,
            api_settings,
            quick_search_chains,
            bens_protocols,
        }
    }

    fn normalize_page_size(&self, size: Option<u32>) -> u32 {
        size.unwrap_or(self.api_settings.default_page_size)
            .clamp(1, self.api_settings.max_page_size)
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

        import::batch_import(self.repo.write_db(), import_request)
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
        let chains = if only_active {
            let token_info_client = &self.token_info_client;
            let dapp_client = &self.dapp_client;
            chains::list_active_chains(
                self.repo.read_db(),
                &[
                    chains::ChainSource::Repository,
                    chains::ChainSource::TokenInfo { token_info_client },
                    chains::ChainSource::Dapp { dapp_client },
                ],
            )
            .await?
        } else {
            chains::list_repo_chains_cached(self.repo.read_db(), false).await?
        };

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

        let (addresses, next_page_token) = search::search_addresses(
            self.repo.read_db(),
            &self.bens_client,
            search::AddressSearchConfig::NonTokenSearch {
                bens_protocols: self.bens_protocols.as_deref(),
                // NOTE: resolve to a primary domain. Multi-TLD resolution is not supported yet.
                bens_domain_lookup_limit: 1,
            },
            inner.q,
            chain_id.map(|v| vec![v]).unwrap_or_default(),
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

        let (addresses, next_page_token) = search::search_addresses(
            self.repo.read_db(),
            &self.bens_client,
            search::AddressSearchConfig::NFTSearch,
            inner.q,
            chain_id.map(|v| vec![v]).unwrap_or_default(),
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

        let (transactions, next_page_token) = search::search_hashes(
            self.repo.read_db(),
            inner.q,
            Some(types::hashes::HashType::Transaction),
            chain_id.map(|v| vec![v]),
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
                page_token: format!("{}", c),
                page_size,
            }),
        }))
    }

    async fn quick_search(
        &self,
        request: Request<QuickSearchRequest>,
    ) -> Result<Response<QuickSearchResponse>, Status> {
        let inner = request.into_inner();

        let results = search::quick_search(
            self.repo.read_db(),
            &self.dapp_client,
            &self.token_info_client,
            &self.bens_client,
            inner.q,
            &self.quick_search_chains,
            self.bens_protocols.as_deref(),
        )
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

        let chain_ids = inner
            .chain_ids
            .into_iter()
            .map(parse_query)
            .collect::<Result<Vec<_>, _>>()?;

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
        let items = chains::list_active_chains(
            self.repo.read_db(),
            &[chains::ChainSource::Dapp {
                dapp_client: &self.dapp_client,
            }],
        )
        .await?
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

#[inline]
fn parse_query<T: FromStr>(input: String) -> Result<T, Status>
where
    <T as FromStr>::Err: std::fmt::Display,
{
    T::from_str(&input)
        .map_err(|e| Status::invalid_argument(format!("invalid value {}: {e}", input)))
}

#[inline]
fn parse_query_2<T1: FromStr, T2: FromStr>(input: String) -> Result<(T1, T2), Status>
where
    <T1 as FromStr>::Err: std::fmt::Display,
    <T2 as FromStr>::Err: std::fmt::Display,
{
    match input.split(',').collect::<Vec<&str>>().as_slice() {
        [v1, v2] => Ok((
            parse_query::<T1>(v1.to_string())?,
            parse_query::<T2>(v2.to_string())?,
        )),
        _ => Err(Status::invalid_argument("invalid page_token format")),
    }
}
