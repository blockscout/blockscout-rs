use crate::{
    proto::{cluster_explorer_service_server::ClusterExplorerService, *},
    services::utils::{PageTokenExtractor, page_token_to_proto, parse_map_result, parse_query},
    settings::ApiSettings,
};
use itertools::Itertools;
use multichain_aggregator_logic::{
    error::ServiceError, services::cluster::Cluster,
    types::addresses::proto_token_type_to_db_token_type,
};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use tonic::{Request, Response, Status};

pub struct ClusterExplorer {
    db: DatabaseConnection,
    clusters: HashMap<String, Cluster>,
    api_settings: ApiSettings,
}

impl ClusterExplorer {
    pub fn new(
        db: DatabaseConnection,
        clusters: HashMap<String, Cluster>,
        api_settings: ApiSettings,
    ) -> Self {
        Self {
            db,
            clusters,
            api_settings,
        }
    }

    #[allow(clippy::result_large_err)]
    fn try_get_cluster(&self, name: &str) -> Result<&Cluster, Status> {
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
        let chains = cluster.list_chains(&self.db).await?;

        let items = chains
            .into_iter()
            .filter_map(|c| c.try_into().ok())
            .collect();

        Ok(Response::new(ListClusterChainsResponse { items }))
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
                &self.db,
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
        let count = cluster.count_interop_messages(&self.db, chain_id).await?;

        Ok(Response::new(CountInteropMessagesResponse { count }))
    }

    async fn get_address(
        &self,
        request: Request<GetAddressRequest>,
    ) -> Result<Response<GetAddressResponse>, Status> {
        let inner = request.into_inner();

        let cluster = self.try_get_cluster(&inner.cluster_id)?;
        let address = parse_query(inner.address_hash)?;

        let address_info = cluster.get_address_info(&self.db, address).await?;

        Ok(Response::new(
            GetAddressResponse::try_from(address_info).map_err(ServiceError::from)?,
        ))
    }

    async fn list_address_tokens(
        &self,
        request: Request<ListAddressTokensRequest>,
    ) -> Result<Response<ListAddressTokensResponse>, Status> {
        let inner = request.into_inner();

        let cluster = self.try_get_cluster(&inner.cluster_id)?;
        let token_types = parse_map_result(&inner.r#type.unwrap_or_default(), |v| {
            let val = serde_json::Value::String(v.to_string());
            serde_json::from_value(val)
        })?
        .into_iter()
        .unique()
        .filter_map(proto_token_type_to_db_token_type)
        .collect();
        let address = parse_query(inner.address_hash)?;

        let page_size = self.normalize_page_size(inner.page_size);
        let page_token = inner.page_token.extract_page_token()?;

        let (tokens, next_page_token) = cluster
            .list_address_tokens(&self.db, address, token_types, page_size as u64, page_token)
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
}
