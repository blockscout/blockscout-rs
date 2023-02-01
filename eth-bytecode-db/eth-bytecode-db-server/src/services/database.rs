use crate::{
    proto::{database_server::Database, SearchSourcesRequest, SearchSourcesResponse},
    types::{BytecodeTypeWrapper, SourceWrapper},
};
use amplify::Wrapper;
use async_trait::async_trait;
use blockscout_display_bytes::Bytes as DisplayBytes;
use eth_bytecode_db::search::{self, BytecodeRemote};
use sea_orm::DatabaseConnection;
use std::{str::FromStr, sync::Arc};

pub struct DatabaseService {
    pub db_client: Arc<DatabaseConnection>,
}

impl DatabaseService {
    pub fn new_arc(db_client: Arc<DatabaseConnection>) -> Self {
        Self { db_client }
    }
}

#[async_trait]
impl Database for DatabaseService {
    async fn search_sources(
        &self,
        request: tonic::Request<SearchSourcesRequest>,
    ) -> Result<tonic::Response<SearchSourcesResponse>, tonic::Status> {
        let request = request.into_inner();

        let bytecode_type = request.bytecode_type();
        let bytecode_remote = BytecodeRemote {
            bytecode_type: BytecodeTypeWrapper::from_inner(bytecode_type).try_into()?,
            data: DisplayBytes::from_str(&request.bytecode)
                .map_err(|err| {
                    tonic::Status::invalid_argument(format!("Invalid bytecode: {}", err))
                })?
                .0,
        };

        let sources = search::find_contract(self.db_client.as_ref(), &bytecode_remote)
            .await
            .map_err(|err| tonic::Status::internal(err.to_string()))?;

        let sources = sources
            .into_iter()
            .map(|source| SourceWrapper::from(source).into_inner())
            .collect();

        let response = SearchSourcesResponse { sources };
        Ok(tonic::Response::new(response))
    }
}
