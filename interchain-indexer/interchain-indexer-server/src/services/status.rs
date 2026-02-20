use std::sync::Arc;

use interchain_indexer_logic::CrosschainIndexer;

use crate::{
    proto::{
        FullStatus, GetFullStatusRequest, GetStatusRequest, IndexerStatus, status_service_server::*,
    },
    services::utils::{db_datetime_to_string, sort_json_value},
};

#[derive(Default)]
pub struct StatusServiceImpl {
    pub indexers: Vec<Arc<dyn CrosschainIndexer>>,
}

impl StatusServiceImpl {
    pub fn new(indexers: Vec<Arc<dyn CrosschainIndexer>>) -> Self {
        Self { indexers }
    }
}

#[async_trait::async_trait]
impl StatusService for StatusServiceImpl {
    async fn get_full_status(
        &self,
        _request: tonic::Request<GetFullStatusRequest>,
    ) -> Result<tonic::Response<FullStatus>, tonic::Status> {
        Ok(tonic::Response::new(FullStatus {
            indexers: self.indexers.iter().map(get_indexer_status).collect(),
        }))
    }

    async fn get_status_by_indexer_name(
        &self,
        request: tonic::Request<GetStatusRequest>,
    ) -> Result<tonic::Response<IndexerStatus>, tonic::Status> {
        let inner = request.into_inner();
        let indexer = self
            .indexers
            .iter()
            .find(|i| i.name() == inner.indexer_name)
            .ok_or(tonic::Status::not_found(format!(
                "Indexer not found: {}",
                inner.indexer_name
            )))?;

        Ok(tonic::Response::new(get_indexer_status(indexer)))
    }
}

fn get_indexer_status(indexer: &Arc<dyn CrosschainIndexer>) -> IndexerStatus {
    let status = indexer.get_status();
    IndexerStatus {
        name: indexer.name(),
        description: (!indexer.description().is_empty()).then_some(indexer.description()),
        state: status.state.to_string(),
        init_timestamp: db_datetime_to_string(status.init_timestamp),
        extra_info: {
            let json = serde_json::Value::Object(status.extra_info.into_iter().collect());
            let json = sort_json_value(json);
            serde_json::from_value::<prost_wkt_types::Struct>(json).ok()
        },
    }
}
