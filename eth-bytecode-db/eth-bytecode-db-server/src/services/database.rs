use crate::proto::{database_server::Database, SearchSourcesRequest, SearchSourcesResponse};
use async_trait::async_trait;

#[derive(Default)]
pub struct DatabaseService {}

#[async_trait]
impl Database for DatabaseService {
    async fn search_sources(
        &self,
        _request: tonic::Request<SearchSourcesRequest>,
    ) -> Result<tonic::Response<SearchSourcesResponse>, tonic::Status> {
        todo!()
    }
}
