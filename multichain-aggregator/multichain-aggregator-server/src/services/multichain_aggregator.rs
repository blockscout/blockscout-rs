use crate::{
    proto::{
        multichain_aggregator_service_server::MultichainAggregatorService, BatchImportRequest,
        BatchImportResponse, ListAddressesRequest, ListAddressesResponse, Pagination,
        QuickSearchRequest, QuickSearchResponse,
    },
    settings::ApiSettings,
};
use api_client_framework::HttpApiClient;
use multichain_aggregator_logic::{
    clients::{
        dapp,
        token_info::{SearchTokenInfos, SearchTokenInfosParams},
    },
    error::ServiceError,
    repository,
    services::{api_key_manager::ApiKeyManager, chains, import, search},
    types,
};
use multichain_aggregator_proto::blockscout::multichain_aggregator::v1::{
    ListChainsRequest, ListChainsResponse, ListDappCategoriesRequest, ListDappCategoriesResponse,
    ListDappChainsRequest, ListDappChainsResponse, ListDappsRequest, ListDappsResponse,
    ListNftsRequest, ListNftsResponse, ListTokensRequest, ListTokensResponse,
    ListTransactionsRequest, ListTransactionsResponse,
};
use sea_orm::DatabaseConnection;
use std::str::FromStr;
use tonic::{Request, Response, Status};

pub struct MultichainAggregator {
    db: DatabaseConnection,
    api_key_manager: ApiKeyManager,
    dapp_client: HttpApiClient,
    token_info_client: HttpApiClient,
    api_settings: ApiSettings,
}

impl MultichainAggregator {
    pub fn new(
        db: DatabaseConnection,
        dapp_client: HttpApiClient,
        token_info_client: HttpApiClient,
        api_settings: ApiSettings,
    ) -> Self {
        Self {
            db: db.clone(),
            api_key_manager: ApiKeyManager::new(db),
            dapp_client,
            token_info_client,
            api_settings,
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

        import::batch_import(&self.db, import_request)
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
        let chains = chains::list_chains_cached(&self.db, only_active).await?;

        Ok(Response::new(ListChainsResponse {
            items: chains
                .into_iter()
                .filter_map(|c| c.try_into().ok())
                .collect(),
        }))
    }

    async fn list_addresses(
        &self,
        request: Request<ListAddressesRequest>,
    ) -> Result<Response<ListAddressesResponse>, Status> {
        let inner = request.into_inner();

        let (address, query) = match parse_query::<alloy_primitives::Address>(inner.q.clone()) {
            Ok(address) => (Some(address), None),
            Err(_) => (None, Some(inner.q)),
        };
        let chain_id = inner.chain_id.map(parse_query).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);
        let page_token = inner.page_token.map(parse_query_2).transpose()?;

        let (addresses, next_page_token) = repository::addresses::list_addresses_paginated(
            &self.db,
            address,
            query,
            chain_id,
            None,
            page_size as u64,
            page_token,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to list addresses");
            Status::internal("failed to list addresses")
        })?;

        Ok(Response::new(ListAddressesResponse {
            items: addresses
                .into_iter()
                .map(|a| types::addresses::Address::try_from(a).map(|a| a.into()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(ServiceError::from)?,
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

        let (address, query) = match parse_query::<alloy_primitives::Address>(inner.q.clone()) {
            Ok(address) => (Some(address), None),
            Err(_) => (None, Some(inner.q)),
        };
        let chain_id = inner.chain_id.map(parse_query).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);
        let page_token = inner.page_token.map(parse_query_2).transpose()?;

        let (addresses, next_page_token) = repository::addresses::list_addresses_paginated(
            &self.db,
            address,
            query,
            chain_id,
            Some(vec![
                types::addresses::TokenType::Erc721,
                types::addresses::TokenType::Erc1155,
            ]),
            page_size as u64,
            page_token,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to list addresses");
            Status::internal("failed to list addresses")
        })?;

        Ok(Response::new(ListNftsResponse {
            items: addresses
                .into_iter()
                .map(|a| types::addresses::Address::try_from(a).map(|a| a.into()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(ServiceError::from)?,
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

        let hash = parse_query::<alloy_primitives::B256>(inner.q.clone())?;
        let chain_id = inner.chain_id.map(parse_query).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);
        let page_token = inner.page_token.map(parse_query).transpose()?;

        let (transactions, next_page_token) = repository::hashes::list_transactions_paginated(
            &self.db,
            hash,
            chain_id,
            page_size as u64,
            page_token,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to list transactions");
            Status::internal("failed to list transactions")
        })?;

        Ok(Response::new(ListTransactionsResponse {
            items: transactions
                .into_iter()
                .map(|t| types::hashes::Hash::try_from(t).map(|t| t.into()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(ServiceError::from)?,
            next_page_params: next_page_token.map(|c| Pagination {
                page_token: format!("{}", c),
                page_size,
            }),
        }))
    }

    async fn list_tokens(
        &self,
        request: Request<ListTokensRequest>,
    ) -> Result<Response<ListTokensResponse>, Status> {
        let inner = request.into_inner();

        let chain_id = inner.chain_id.map(parse_query).transpose()?;

        let token_info_search_endpoint = SearchTokenInfos {
            params: SearchTokenInfosParams {
                query: inner.q.to_string(),
                chain_id,
                page_size: inner.page_size,
                page_token: inner.page_token,
            },
        };

        let res = self
            .token_info_client
            .request(&token_info_search_endpoint)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to list tokens");
                Status::internal("failed to list tokens")
            })?;

        let tokens = res
            .token_infos
            .into_iter()
            .map(|t| types::token_info::Token::try_from(t).map(|t| t.into()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(ServiceError::from)?;

        Ok(Response::new(ListTokensResponse {
            items: tokens,
            next_page_params: res.next_page_params.map(|p| Pagination {
                page_token: p.page_token,
                page_size: p.page_size,
            }),
        }))
    }

    async fn quick_search(
        &self,
        request: Request<QuickSearchRequest>,
    ) -> Result<Response<QuickSearchResponse>, Status> {
        let inner = request.into_inner();

        let results = search::quick_search(
            &self.db,
            &self.dapp_client,
            &self.token_info_client,
            inner.q,
        )
        .await
        .inspect_err(|err| {
            tracing::error!(error = ?err, "failed to quick search");
        })?;

        Ok(Response::new(results.into()))
    }

    async fn list_dapps(
        &self,
        request: Request<ListDappsRequest>,
    ) -> Result<Response<ListDappsResponse>, Status> {
        let inner = request.into_inner();

        let dapps = self
            .dapp_client
            .request(&dapp::search_dapps::SearchDapps {
                params: dapp::search_dapps::SearchDappsParams {
                    title: inner.q,
                    categories: inner.categories,
                    chain_ids: inner.chain_ids,
                },
            })
            .await
            .map_err(|err| ServiceError::Internal(err.into()))?
            .into_iter()
            .filter_map(|d| {
                types::dapp::MarketplaceDapp::try_from(d)
                    .map(|d| d.into())
                    .ok()
            })
            .collect();
        Ok(Response::new(ListDappsResponse { items: dapps }))
    }

    async fn list_dapp_chains(
        &self,
        _request: Request<ListDappChainsRequest>,
    ) -> Result<Response<ListDappChainsResponse>, Status> {
        let chain_ids = self
            .dapp_client
            .request(&dapp::list_chains::ListChains {})
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to list marketplace chains");
                Status::internal("failed to list marketplace chains")
            })?;

        let chains = chains::list_chains_cached(&self.db, false).await?;

        let items = chains
            .into_iter()
            .filter(|c| chain_ids.contains(&c.id.to_string()))
            .filter_map(|c| c.try_into().ok())
            .collect::<Vec<_>>();

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
