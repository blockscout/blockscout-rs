use crate::{
    proto::{
        database_server::Database, SearchSourcesRequest, SearchSourcesResponse,
        VerificationMetadata,
    },
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
    pub sourcify_client: Option<sourcify::Client>,
}

impl DatabaseService {
    pub fn new_arc(db_client: Arc<DatabaseConnection>) -> Self {
        Self {
            db_client,
            sourcify_client: None,
        }
    }

    pub fn with_sourcify_client(mut self, sourcify_client: sourcify::Client) -> Self {
        self.sourcify_client = Some(sourcify_client);
        self
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
                .map_err(|err| tonic::Status::invalid_argument(format!("Invalid bytecode: {err}")))?
                .0,
        };

        let sources = search::find_contract(self.db_client.as_ref(), &bytecode_remote)
            .await
            .map_err(|err| tonic::Status::internal(err.to_string()))?;

        match (self.sourcify_client.as_ref(), request.metadata) {
            (
                Some(sourcify_client),
                Some(VerificationMetadata {
                    chain_id: Some(chain_id),
                    contract_address: Some(contract_address),
                }),
            ) => {
                let contract_address = DisplayBytes::from_str(&contract_address)
                    .map_err(|err| tonic::Status::invalid_argument(format!("Invalid contract address in verification metadata: {err}")))?
                    .0;
                let sourcify_result = sourcify_client.get_source_files_any(&chain_id, contract_address).await.map_err(|err| match err {

                });
            }
            _ => todo!(),
        };

        let sources = sources
            .into_iter()
            .map(|source| SourceWrapper::from(source).into_inner())
            .collect();

        let response = SearchSourcesResponse { sources };
        Ok(tonic::Response::new(response))
    }
}
