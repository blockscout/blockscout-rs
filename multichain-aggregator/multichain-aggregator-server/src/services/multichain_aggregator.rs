use crate::{
    proto::{
        multichain_aggregator_service_server::MultichainAggregatorService, BatchImportRequest,
        BatchImportResponse, ListAddressesRequest, ListAddressesResponse, Pagination,
        QuickSearchRequest, QuickSearchResponse,
    },
    settings::ServiceSettings,
};
use multichain_aggregator_logic::{
    self as logic, api_key_manager::ApiKeyManager, error::ServiceError, Chain,
};
use sea_orm::DatabaseConnection;
use std::str::FromStr;
use tonic::{Request, Response, Status};

const DEFAULT_PAGE_SIZE: u32 = 50;

pub struct MultichainAggregator {
    db: DatabaseConnection,
    api_key_manager: ApiKeyManager,
    // Cached chains
    chains: Vec<Chain>,

    settings: ServiceSettings,
}

impl MultichainAggregator {
    pub fn new(db: DatabaseConnection, chains: Vec<Chain>, settings: ServiceSettings) -> Self {
        Self {
            db: db.clone(),
            api_key_manager: ApiKeyManager::new(db),
            chains,
            settings,
        }
    }

    fn normalize_page_size(&self, size: Option<u32>) -> u32 {
        size.unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, self.settings.max_page_size)
    }
}

#[async_trait::async_trait]
impl MultichainAggregatorService for MultichainAggregator {
    async fn batch_import(
        &self,
        request: Request<BatchImportRequest>,
    ) -> Result<Response<BatchImportResponse>, Status> {
        let inner = request.into_inner();

        let api_key = (inner.api_key.as_str(), inner.chain_id.as_str())
            .try_into()
            .map_err(ServiceError::from)?;
        self.api_key_manager
            .validate_api_key(api_key)
            .await
            .map_err(ServiceError::from)?;

        let import_request: logic::BatchImportRequest = inner.try_into()?;

        logic::batch_import(&self.db, import_request)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to batch import");
                Status::internal("failed to batch import")
            })?;

        Ok(Response::new(BatchImportResponse {
            status: "ok".to_string(),
        }))
    }

    async fn list_addresses(
        &self,
        request: Request<ListAddressesRequest>,
    ) -> Result<Response<ListAddressesResponse>, Status> {
        let inner = request.into_inner();

        let page_token: Option<(logic::ChainId, alloy_primitives::Address)> =
            inner.page_token.map(parse_query_2).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);

        let (addresses, next_page_token) = logic::repository::addresses::search_by_query_paginated(
            &self.db,
            &inner.q,
            page_token,
            page_size as u64,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to list addresses");
            Status::internal("failed to list addresses")
        })?;

        Ok(Response::new(ListAddressesResponse {
            addresses: addresses.into_iter().map(|a| a.into()).collect(),
            pagination: next_page_token.map(|(c, a)| Pagination {
                page_token: format!("{},{}", c, a.to_checksum(None)),
                page_size,
            }),
        }))
    }

    async fn quick_search(
        &self,
        request: Request<QuickSearchRequest>,
    ) -> Result<Response<QuickSearchResponse>, Status> {
        let inner = request.into_inner();

        let results = logic::search::quick_search(&self.db, inner.q, &self.chains)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to quick search");
                Status::internal("failed to quick search")
            })?;

        Ok(Response::new(results.into()))
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
