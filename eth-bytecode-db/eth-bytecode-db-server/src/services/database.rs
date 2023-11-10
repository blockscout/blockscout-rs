use crate::{
    proto::{
        database_server::Database, BytecodeType, SearchAllSourcesRequest, SearchAllSourcesResponse,
        SearchSourcesRequest, SearchSourcesResponse, SearchSourcifySourcesRequest, Source,
        VerifyResponse,
    },
    types::{BytecodeTypeWrapper, SourceWrapper, VerifyResponseWrapper},
};
use amplify::Wrapper;
use async_trait::async_trait;
use blockscout_display_bytes::Bytes as DisplayBytes;
use eth_bytecode_db::{
    search::{self, BytecodeRemote},
    verification,
    verification::sourcify_from_etherscan,
};
use std::str::FromStr;

pub struct DatabaseService {
    pub client: verification::Client,
    pub sourcify_client: sourcify::Client,
}

impl DatabaseService {
    pub fn new_arc(client: verification::Client, sourcify_client: sourcify::Client) -> Self {
        Self {
            client,
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

        let sources = self.search_sources(bytecode_type, &bytecode).await?;

        Ok(tonic::Response::new(SearchSourcesResponse { sources }))
    }

    async fn search_sourcify_sources(
        &self,
        request: tonic::Request<SearchSourcifySourcesRequest>,
    ) -> Result<tonic::Response<SearchSourcesResponse>, tonic::Status> {
        let request = request.into_inner();

        let chain_id = request.chain;
        let contract_address = request.address;

        let source = self
            .search_sourcify_sources(&chain_id, &contract_address)
            .await?;

        Ok(tonic::Response::new(SearchSourcesResponse {
            sources: source.map_or(vec![], |source| vec![source]),
        }))
    }

    async fn search_all_sources(
        &self,
        request: tonic::Request<SearchAllSourcesRequest>,
    ) -> Result<tonic::Response<SearchAllSourcesResponse>, tonic::Status> {
        let request = request.into_inner();

        let bytecode_type = request.bytecode_type();
        let bytecode = request.bytecode;
        let chain_id = request.chain;
        let contract_address = request.address;

        tracing::debug!(
            contract_address = contract_address,
            chain_id = chain_id,
            bytecode_type = ?bytecode_type,
            bytecode = bytecode,
            "search all sources request"
        );

        let search_sources_task = self.search_sources(bytecode_type, &bytecode);
        let search_sourcify_sources_task =
            self.search_sourcify_sources(&chain_id, &contract_address);

        let (eth_bytecode_db_sources, sourcify_source) =
            tokio::join!(search_sources_task, search_sourcify_sources_task);
        let eth_bytecode_db_sources = eth_bytecode_db_sources?;
        let mut sourcify_source = sourcify_source?;

        // Importing contracts from etherscan may be quite expensive operation.
        // For that reason, we try to use that approach only if no other sources have been found.
        if eth_bytecode_db_sources.is_empty() && sourcify_source.is_none() {
            tracing::info!(
                contract_address = contract_address,
                chain_id = chain_id,
                "no sources have been found neither in eth-bytecode-db nor in sourcify.\
                Trying to verify from etherscan"
            );
            let verification_request = sourcify_from_etherscan::VerificationRequest {
                address: contract_address,
                chain: chain_id,
            };
            let result =
                sourcify_from_etherscan::verify(self.client.clone(), verification_request).await;

            if let Ok(source) = result {
                let response: VerifyResponse = VerifyResponseWrapper::ok(source).into();
                sourcify_source = response.source;
            }
        }

        let response = SearchAllSourcesResponse {
            eth_bytecode_db_sources,
            sourcify_sources: sourcify_source.map_or(vec![], |source| vec![source]),
        };

        Ok(tonic::Response::new(response))
    }
}

impl DatabaseService {
    async fn search_sources(
        &self,
        bytecode_type: BytecodeType,
        bytecode: &str,
    ) -> Result<Vec<Source>, tonic::Status> {
        let bytecode_remote = BytecodeRemote {
            bytecode_type: BytecodeTypeWrapper::from_inner(bytecode_type).try_into()?,
            data: DisplayBytes::from_str(bytecode)
                .map_err(|err| tonic::Status::invalid_argument(format!("Invalid bytecode: {err}")))?
                .0,
        };

        let mut matches = search::find_contract(self.client.db_client.as_ref(), &bytecode_remote)
            .await
            .map_err(|err| tonic::Status::internal(err.to_string()))?;
        matches.sort_by_key(|m| m.updated_at);

        let sources = matches
            .into_iter()
            .rev()
            .map(|source| SourceWrapper::from(source).into_inner())
            .collect();

        Ok(sources)
    }

    async fn search_sourcify_sources(
        &self,
        chain_id: &str,
        contract_address: &str,
    ) -> Result<Option<Source>, tonic::Status> {
        let contract_address = DisplayBytes::from_str(contract_address)
            .map_err(|err| {
                tonic::Status::invalid_argument(format!("Invalid contract address: {err}"))
            })?
            .0;

        let sourcify_result = self
            .sourcify_client
            .get_source_files_any(chain_id, contract_address)
            .await
            .map_err(process_sourcify_error);

        let result = match sourcify_result {
            Ok(response) => {
                let source = SourceWrapper::try_from(response)?.into_inner();
                Some(source)
            }
            Err(None) => None,
            Err(Some(err)) => return Err(err),
        };

        Ok(result)
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
        sourcify::Error::Sourcify(sourcify::SourcifyError::BadGateway(_)) => {
            tracing::error!(target: "sourcify", "{error}");
            Some(tonic::Status::internal("sourcify responded with error"))
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
