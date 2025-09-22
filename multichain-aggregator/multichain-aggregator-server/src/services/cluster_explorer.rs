use crate::{
    proto::cluster_explorer_service_server::ClusterExplorerService,
    services::{macros::*, utils::*},
    settings::ApiSettings,
};
use itertools::Itertools;
use multichain_aggregator_logic::{
    error::ServiceError,
    services::cluster::Cluster,
    types::{addresses::proto_token_type_to_db_token_type, tokens::TokenType},
};
use multichain_aggregator_proto::blockscout::cluster_explorer::v1::*;
use std::collections::HashMap;
use tonic::{Request, Response, Status};

pub struct ClusterExplorer {
    clusters: HashMap<String, Cluster>,
    api_settings: ApiSettings,
}

impl ClusterExplorer {
    pub fn new(clusters: HashMap<String, Cluster>, api_settings: ApiSettings) -> Self {
        Self {
            clusters,
            api_settings,
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn try_get_cluster(&self, name: &str) -> Result<&Cluster, Status> {
        self.clusters
            .get(name)
            .ok_or(Status::not_found(format!("cluster not found: {name}")))
    }

    fn normalize_page_size(&self, size: Option<u32>) -> u32 {
        size.unwrap_or(self.api_settings.default_page_size)
            .clamp(1, self.api_settings.max_page_size)
    }
}

#[async_trait::async_trait]
impl ClusterExplorerService for ClusterExplorer {
    async fn list_cluster_chains(
        &self,
        request: Request<ListClusterChainsRequest>,
    ) -> Result<Response<ListClusterChainsResponse>, Status> {
        let inner = request.into_inner();

        let cluster = self.try_get_cluster(&inner.cluster_id)?;
        let chains = cluster.list_chains().await?;

        let items = chains
            .into_iter()
            .filter_map(|c| c.try_into().ok())
            .collect();

        Ok(Response::new(ListClusterChainsResponse { items }))
    }

    async fn get_interop_message(
        &self,
        request: Request<GetInteropMessageRequest>,
    ) -> Result<Response<GetInteropMessageResponse>, Status> {
        let inner = request.into_inner();

        let init_chain_id = parse_query(inner.init_chain_id)?;
        let nonce = inner.nonce;

        let cluster = self.try_get_cluster(&inner.cluster_id)?;
        let message = cluster
            .get_interop_message(init_chain_id, nonce)
            .await?
            .into();

        Ok(Response::new(GetInteropMessageResponse {
            message: Some(message),
        }))
    }

    async fn list_interop_messages(
        &self,
        request: Request<ListInteropMessagesRequest>,
    ) -> Result<Response<ListInteropMessagesResponse>, Status> {
        let inner = request.into_inner();

        let init_chain_id = inner.init_chain_id.map(parse_query).transpose()?;
        let relay_chain_id = inner.relay_chain_id.map(parse_query).transpose()?;
        let address = inner.address.map(parse_query).transpose()?;
        let direction = inner.direction.map(parse_query).transpose()?;

        let page_size = self.normalize_page_size(inner.page_size);
        let page_token = inner.page_token.extract_page_token()?;

        let cluster = self.try_get_cluster(&inner.cluster_id)?;
        let (interop_messages, next_page_token) = cluster
            .list_interop_messages(
                init_chain_id,
                relay_chain_id,
                address,
                direction,
                inner.nonce,
                page_size as u64,
                page_token,
            )
            .await?;

        Ok(Response::new(ListInteropMessagesResponse {
            items: interop_messages.into_iter().map(|i| i.into()).collect(),
            next_page_params: page_token_to_proto(next_page_token, page_size),
        }))
    }

    async fn count_interop_messages(
        &self,
        request: Request<CountInteropMessagesRequest>,
    ) -> Result<Response<CountInteropMessagesResponse>, Status> {
        let inner = request.into_inner();

        let chain_id = parse_query(inner.chain_id)?;

        let cluster = self.try_get_cluster(&inner.cluster_id)?;
        let count = cluster.count_interop_messages(chain_id).await?;

        Ok(Response::new(CountInteropMessagesResponse { count }))
    }

    async fn get_address(
        &self,
        request: Request<GetAddressRequest>,
    ) -> Result<Response<GetAddressResponse>, Status> {
        let inner = request.into_inner();

        let cluster = self.try_get_cluster(&inner.cluster_id)?;
        let address = parse_query(inner.address_hash)?;

        let address_info = cluster.get_address_info_aggregated(address).await?;

        Ok(Response::new(address_info.into()))
    }

    async fn list_address_tokens(
        &self,
        request: Request<ListAddressTokensRequest>,
    ) -> Result<Response<ListAddressTokensResponse>, Status> {
        let inner = request.into_inner();

        let cluster = self.try_get_cluster(&inner.cluster_id)?;
        let token_types = parse_token_types(inner.r#type)?;
        let address = parse_query(inner.address_hash)?;
        let chain_ids = parse_chain_ids(inner.chain_id)?;
        let page_size = self.normalize_page_size(inner.page_size);
        let page_token = inner.page_token.extract_page_token()?;

        let (tokens, next_page_token) = cluster
            .list_address_tokens(
                address,
                token_types,
                chain_ids,
                page_size as u64,
                page_token,
            )
            .await?;

        let items = tokens
            .into_iter()
            .map(|t| t.try_into().map_err(ServiceError::from))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Response::new(ListAddressTokensResponse {
            items,
            next_page_params: page_token_to_proto(next_page_token, page_size),
        }))
    }

    async fn list_cluster_tokens(
        &self,
        request: Request<ListClusterTokensRequest>,
    ) -> Result<Response<ListClusterTokensResponse>, Status> {
        let inner = request.into_inner();

        let cluster = self.try_get_cluster(&inner.cluster_id)?;
        let token_types = parse_token_types(inner.r#type)?;
        let chain_ids = parse_chain_ids(inner.chain_id)?;
        let page_size = self.normalize_page_size(inner.page_size);
        let page_token = inner.page_token.extract_page_token()?;

        let (tokens, next_page_token) = cluster
            .list_cluster_tokens(token_types, chain_ids, page_size as u64, page_token)
            .await?;

        Ok(Response::new(ListClusterTokensResponse {
            items: tokens
                .into_iter()
                .map(|t| t.try_into().map_err(ServiceError::from))
                .collect::<Result<Vec<_>, _>>()?,
            next_page_params: page_token_to_proto(next_page_token, page_size),
        }))
    }

    async fn get_aggregated_token(
        &self,
        request: Request<GetAggregatedTokenRequest>,
    ) -> Result<Response<GetAggregatedTokenResponse>, Status> {
        let inner = request.into_inner();

        let cluster = self.try_get_cluster(&inner.cluster_id)?;
        let address = parse_query(inner.address_hash)?;
        let chain_id = parse_query(inner.chain_id)?;

        let token = cluster.get_aggregated_token(address, chain_id).await?;

        Ok(Response::new(GetAggregatedTokenResponse {
            token: token
                .map(|t| t.try_into().map_err(ServiceError::from))
                .transpose()?,
        }))
    }

    async fn list_token_holders(
        &self,
        request: Request<ListTokenHoldersRequest>,
    ) -> Result<Response<ListTokenHoldersResponse>, Status> {
        let inner = request.into_inner();

        let cluster = self.try_get_cluster(&inner.cluster_id)?;
        let address = parse_query(inner.address_hash)?;
        let chain_id = parse_query(inner.chain_id)?;
        let page_size = self.normalize_page_size(inner.page_size);
        let page_token = inner.page_token.extract_page_token()?;

        let (holders, next_page_token) = cluster
            .list_token_holders(address, chain_id, page_size as u64, page_token)
            .await?;

        Ok(Response::new(ListTokenHoldersResponse {
            items: holders.into_iter().map(|h| h.into()).collect(),
            next_page_params: page_token_to_proto(next_page_token, page_size),
        }))
    }

    async fn search_addresses(
        &self,
        request: Request<SearchByQueryRequest>,
    ) -> Result<Response<SearchAddressesResponse>, Status> {
        paginated_list_by_query_endpoint!(
            self,
            request,
            search_addresses_aggregated,
            SearchAddressesResponse
        )
    }

    async fn search_nfts(
        &self,
        request: Request<SearchByQueryRequest>,
    ) -> Result<Response<SearchNftsResponse>, Status> {
        paginated_list_by_query_endpoint!(self, request, search_nfts, SearchNftsResponse)
    }

    async fn search_transactions(
        &self,
        request: Request<SearchByQueryRequest>,
    ) -> Result<Response<SearchTransactionsResponse>, Status> {
        paginated_list_by_query_endpoint!(
            self,
            request,
            search_transactions,
            SearchTransactionsResponse
        )
    }

    async fn search_blocks(
        &self,
        request: Request<SearchByQueryRequest>,
    ) -> Result<Response<SearchBlocksResponse>, Status> {
        paginated_list_by_query_endpoint!(self, request, search_blocks, SearchBlocksResponse)
    }

    async fn search_block_numbers(
        &self,
        request: Request<SearchByQueryRequest>,
    ) -> Result<Response<SearchBlockNumbersResponse>, Status> {
        paginated_list_by_query_endpoint!(
            self,
            request,
            search_block_numbers,
            SearchBlockNumbersResponse
        )
    }

    async fn search_tokens(
        &self,
        request: Request<SearchByQueryRequest>,
    ) -> Result<Response<SearchTokensResponse>, Status> {
        paginated_list_by_query_endpoint!(self, request, search_tokens, SearchTokensResponse)
    }

    async fn search_domains(
        &self,
        request: Request<SearchByQueryRequest>,
    ) -> Result<Response<SearchDomainsResponse>, Status> {
        paginated_list_by_query_endpoint!(
            self,
            request,
            search_domains_cached,
            SearchDomainsResponse
        )
    }

    async fn search_dapps(
        &self,
        request: Request<SearchByQueryRequest>,
    ) -> Result<Response<SearchDappsResponse>, Status> {
        paginated_list_by_query_endpoint!(
            self,
            request,
            search_dapps_paginated,
            SearchDappsResponse
        )
    }

    async fn quick_search(
        &self,
        request: Request<ClusterQuickSearchRequest>,
    ) -> Result<Response<ClusterQuickSearchResponse>, Status> {
        let inner = request.into_inner();

        let cluster = self.try_get_cluster(&inner.cluster_id)?;
        let result = cluster.quick_search(inner.q, true).await?;

        Ok(Response::new(
            result.try_into().map_err(ServiceError::from)?,
        ))
    }
}

#[allow(clippy::result_large_err)]
fn parse_token_types(types: Option<String>) -> Result<Vec<TokenType>, Status> {
    let types = if let Some(types) = types {
        parse_map_result(&types, |v| {
            let val = serde_json::Value::String(v.to_string());
            serde_json::from_value(val)
        })?
        .into_iter()
        .unique()
        .filter_map(proto_token_type_to_db_token_type)
        .collect()
    } else {
        vec![]
    };

    Ok(types)
}
