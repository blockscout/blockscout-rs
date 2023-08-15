use crate::{
    proto::{
        database_server::Database, BytecodeType, SearchAllSourcesRequest, SearchAllSourcesResponse,
        SearchSourcesRequest, SearchSourcesResponse, SearchSourcifySourcesRequest, Source,
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
    pub fn new_arc(
        db_client: Arc<DatabaseConnection>,
        sourcify_client: Option<sourcify::Client>,
    ) -> Self {
        Self {
            db_client,
            sourcify_client,
        }
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
        let bytecode = request.bytecode;

        let sources = self.search_sources(bytecode_type, bytecode).await?;

        Ok(tonic::Response::new(SearchSourcesResponse { sources }))
    }

    async fn search_sourcify_sources(
        &self,
        request: tonic::Request<SearchSourcifySourcesRequest>,
    ) -> Result<tonic::Response<SearchSourcesResponse>, tonic::Status> {
        let request = request.into_inner();

        let chain_id = request.chain_id;
        let contract_address = DisplayBytes::from_str(&request.contract_address)
            .map_err(|err| {
                tonic::Status::invalid_argument(format!("Invalid contract address: {err}"))
            })?
            .0;

        let sourcify_client = self
            .sourcify_client
            .as_ref()
            .ok_or(tonic::Status::unimplemented(
                "sourcify search is not enabled",
            ))?;

        let sourcify_result = sourcify_client
            .get_source_files_any(&chain_id, contract_address)
            .await
            .map_err(process_sourcify_error);

        let result = match sourcify_result {
            Ok(response) => {
                let source = SourceWrapper::try_from(response)?.into_inner();
                SearchSourcesResponse {
                    sources: vec![source],
                }
            }
            Err(None) => SearchSourcesResponse { sources: vec![] },
            Err(Some(err)) => return Err(err),
        };

        Ok(tonic::Response::new(result))
    }

    async fn search_all_sources(
        &self,
        request: tonic::Request<SearchAllSourcesRequest>,
    ) -> Result<tonic::Response<SearchAllSourcesResponse>, tonic::Status> {
        let request = request.into_inner();

        let bytecode_type = request.bytecode_type();
        let bytecode = request.bytecode;

        let eth_bytecode_db_sources = self.search_sources(bytecode_type, bytecode).await?;

        let response = SearchAllSourcesResponse {
            eth_bytecode_db_sources,
            sourcify_sources: vec![],
        };

        Ok(tonic::Response::new(response))
    }
}

impl DatabaseService {
    async fn search_sources(
        &self,
        bytecode_type: BytecodeType,
        bytecode: String,
    ) -> Result<Vec<Source>, tonic::Status> {
        let bytecode_remote = BytecodeRemote {
            bytecode_type: BytecodeTypeWrapper::from_inner(bytecode_type).try_into()?,
            data: DisplayBytes::from_str(&bytecode)
                .map_err(|err| tonic::Status::invalid_argument(format!("Invalid bytecode: {err}")))?
                .0,
        };

        let sources = search::find_contract(self.db_client.as_ref(), &bytecode_remote)
            .await
            .map_err(|err| tonic::Status::internal(err.to_string()))?;

        let sources = sources
            .into_iter()
            .map(|source| SourceWrapper::from(source).into_inner())
            .collect();

        Ok(sources)
    }
}

fn process_sourcify_error(
    error: sourcify::Error<sourcify::EmptyCustomError>,
) -> Option<tonic::Status> {
    match error {
        sourcify::Error::Reqwest(_) | sourcify::Error::ReqwestMiddleware(_) => {
            tracing::error!(target: "sourcify", "{error}");
            Some(tonic::Status::internal(
                "sending request to sourcify failed",
            ))
        }
        sourcify::Error::Sourcify(sourcify::SourcifyError::InternalServerError(_)) => {
            tracing::error!(target: "sourcify", "{error}");
            Some(tonic::Status::internal("sourcify responded with error"))
        }
        sourcify::Error::Sourcify(sourcify::SourcifyError::NotFound(_)) => {
            tracing::trace!(target: "sourcify", "{error}");
            None
        }
        sourcify::Error::Sourcify(sourcify::SourcifyError::ChainNotSupported(_)) => {
            tracing::error!(target: "sourcify", "{error}");
            None
        }
        sourcify::Error::Sourcify(sourcify::SourcifyError::BadRequest(_)) => {
            tracing::error!(target: "sourcify", "{error}");
            Some(tonic::Status::internal("sourcify responded with error"))
        }
        sourcify::Error::Sourcify(sourcify::SourcifyError::UnexpectedStatusCode { .. }) => {
            tracing::error!(target: "sourcify", "{error}");
            Some(tonic::Status::internal("sourcify responded with error"))
        }
        sourcify::Error::Sourcify(sourcify::SourcifyError::Custom(_)) => {
            // `EmptyCustomError` enum has no variants and cannot be initialized
            unreachable!()
        }
    }
}
