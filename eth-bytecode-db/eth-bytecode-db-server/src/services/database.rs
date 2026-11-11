// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::{
    proto::{
        database_server::Database, AllianceStats, BatchSearchEventDescriptionsRequest,
        BatchSearchEventDescriptionsResponse, BatchSearchFunctionDescriptionsRequest,
        BatchSearchFunctionDescriptionsResponse, BytecodeType, GetAllianceStatsRequest,
        SearchAllSourcesRequest, SearchAllSourcesResponse, SearchAllianceSourcesRequest,
        SearchEventDescriptionsRequest, SearchEventDescriptionsResponse,
        SearchFunctionDescriptionsRequest, SearchFunctionDescriptionsResponse,
        SearchSourcesRequest, SearchSourcesResponse, SearchSourcifySourcesRequest, Source,
        VerifyResponse,
    },
    types::{
        BytecodeTypeWrapper, EventDescriptionWrapper, FunctionDescriptionWrapper, SourceWrapper,
        VerifyResponseWrapper,
    },
};
use alloy_primitives::{Selector, B256};
use amplify::Wrapper;
use async_trait::async_trait;
use blockscout_display_bytes::Bytes as DisplayBytes;
use blockscout_service_launcher::database::ReadWriteRepo;
use eth_bytecode_db::{
    search::{self},
    verification,
    verification::sourcify_from_etherscan,
};
use std::{str::FromStr, sync::Arc};
use tracing::instrument;

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
    #[instrument(skip_all)]
    async fn search_sources(
        &self,
        request: tonic::Request<SearchSourcesRequest>,
    ) -> Result<tonic::Response<SearchSourcesResponse>, tonic::Status> {
        let request = request.into_inner();

        let bytecode_type = request.bytecode_type();
        let bytecode = request.bytecode;

        let sources = self
            .search_sources_internal(bytecode_type, &bytecode)
            .await?;

        Ok(tonic::Response::new(SearchSourcesResponse { sources }))
    }

    #[instrument(skip_all)]
    async fn search_sourcify_sources(
        &self,
        request: tonic::Request<SearchSourcifySourcesRequest>,
    ) -> Result<tonic::Response<SearchSourcesResponse>, tonic::Status> {
        let request = request.into_inner();
        super::trace_request_metadata!(
            chain_id = request.chain,
            contract_address = request.address
        );

        let chain_id = request.chain;
        let contract_address = request.address;

        let source = self
            .search_sourcify_sources_internal(&chain_id, &contract_address)
            .await?;

        Ok(tonic::Response::new(SearchSourcesResponse {
            sources: source.map_or(vec![], |source| vec![source]),
        }))
    }

    #[instrument(skip_all)]
    async fn search_alliance_sources(
        &self,
        request: tonic::Request<SearchAllianceSourcesRequest>,
    ) -> Result<tonic::Response<SearchSourcesResponse>, tonic::Status> {
        let request = request.into_inner();
        super::trace_request_metadata!(
            chain_id = request.chain,
            contract_address = request.address
        );

        match self.client.alliance_db_client.clone() {
            None => {
                tracing::trace!("Unavailable: verifier alliance is not enabled");
                Err(tonic::Status::unavailable(
                    "Verifier alliance is not enabled",
                ))
            }
            Some(alliance_db_client) => {
                let sources = self
                    .search_alliance_sources_internal(
                        alliance_db_client,
                        &request.chain,
                        &request.address,
                    )
                    .await?;
                Ok(tonic::Response::new(SearchSourcesResponse { sources }))
            }
        }
    }

    #[instrument(skip_all)]
    async fn search_all_sources(
        &self,
        request: tonic::Request<SearchAllSourcesRequest>,
    ) -> Result<tonic::Response<SearchAllSourcesResponse>, tonic::Status> {
        let request = request.into_inner();
        super::trace_request_metadata!(
            chain_id = request.chain,
            contract_address = request.address
        );

        let bytecode_type = request.bytecode_type();
        let bytecode = request.bytecode;
        let chain_id = request.chain;
        let contract_address = request.address;
        let only_local = request.only_local.unwrap_or_default();

        tracing::debug!(
            contract_address = contract_address,
            chain_id = chain_id,
            bytecode_type = ?bytecode_type,
            bytecode = bytecode,
            "search all sources request"
        );

        let search_sources_task = self.search_sources_internal(bytecode_type, &bytecode);
        let search_alliance_sources_task =
            futures::future::OptionFuture::from(self.client.alliance_db_client.clone().map(
                |alliance_db_client| {
                    self.search_alliance_sources_internal(
                        alliance_db_client,
                        &chain_id,
                        &contract_address,
                    )
                },
            ));
        let search_sourcify_sources_task = futures::future::OptionFuture::from(
            (!only_local)
                .then(|| self.search_sourcify_sources_internal(&chain_id, &contract_address)),
        );

        let (eth_bytecode_db_sources, alliance_sources, sourcify_source) = tokio::join!(
            search_sources_task,
            search_alliance_sources_task,
            search_sourcify_sources_task
        );

        let trace_error = |source: &str, err: &tonic::Status| {
            tracing::warn!(
                contract_address = contract_address,
                chain_id = chain_id,
                status =? err,
                "Looking for sources in {source} database failed"
            );
        };

        let eth_bytecode_db_sources = eth_bytecode_db_sources
            .inspect_err(|err| trace_error("eth_bytecode_db", err))
            .unwrap_or_default();
        let alliance_sources = alliance_sources
            .transpose()
            .inspect_err(|err| trace_error("alliance", err))
            .unwrap_or_default()
            .unwrap_or_default();
        let mut sourcify_source = sourcify_source
            .transpose()
            .inspect_err(|err| trace_error("sourcify", err))
            .unwrap_or_default()
            .flatten();

        // Importing contracts from etherscan may be quite expensive operation.
        // For that reason, we try to use that approach only if no other sources have been found.
        if !only_local
            && eth_bytecode_db_sources.is_empty()
            && alliance_sources.is_empty()
            && sourcify_source.is_none()
        {
            tracing::info!(
                contract_address = contract_address,
                chain_id = chain_id,
                "no sources have been found neither in eth-bytecode-db, nor in verifier-alliance, nor in sourcify.\
                Trying to verify from etherscan"
            );
            let verification_request = sourcify_from_etherscan::VerificationRequest {
                address: contract_address.clone(),
                chain: chain_id.clone(),
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
            alliance_sources,
        };

        Ok(tonic::Response::new(response))
    }

    async fn search_event_descriptions(
        &self,
        request: tonic::Request<SearchEventDescriptionsRequest>,
    ) -> Result<tonic::Response<SearchEventDescriptionsResponse>, tonic::Status> {
        let request = request.into_inner();
        let selector = B256::from_str(&request.selector).map_err(|err| {
            tonic::Status::invalid_argument(format!("selector is not valid: {err}"))
        })?;

        let event_descriptions =
            search::find_event_descriptions(self.client.db_client.as_ref(), vec![selector])
                .await
                .remove(0)
                .map_err(|err| tonic::Status::internal(err.to_string()))?;

        Ok(tonic::Response::new(event_descriptions_to_search_response(
            event_descriptions,
        )))
    }

    async fn batch_search_event_descriptions(
        &self,
        request: tonic::Request<BatchSearchEventDescriptionsRequest>,
    ) -> Result<tonic::Response<BatchSearchEventDescriptionsResponse>, tonic::Status> {
        const BATCH_LIMIT: usize = 100;

        let request = request.into_inner();
        let selectors = request
            .selectors
            .into_iter()
            .take(BATCH_LIMIT)
            .map(|selector| {
                B256::from_str(&selector).map_err(|err| {
                    tonic::Status::invalid_argument(format!("selector is not valid: {err}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let responses: Vec<_> =
            search::find_event_descriptions(self.client.db_client.as_ref(), selectors)
                .await
                .into_iter()
                .map(|event_descriptions| event_descriptions.unwrap_or_default())
                .map(event_descriptions_to_search_response)
                .collect();

        Ok(tonic::Response::new(BatchSearchEventDescriptionsResponse {
            responses,
        }))
    }

    async fn search_function_descriptions(
        &self,
        request: tonic::Request<SearchFunctionDescriptionsRequest>,
    ) -> Result<tonic::Response<SearchFunctionDescriptionsResponse>, tonic::Status> {
        let request = request.into_inner();
        let selector = parse_function_selector(&request.selector)?;

        let function_descriptions =
            search::find_function_descriptions(self.client.db_client.as_ref(), vec![selector])
                .await
                .map_err(|err| tonic::Status::internal(err.to_string()))?
                .pop()
                .unwrap_or_default();

        Ok(tonic::Response::new(
            function_descriptions_to_search_response(function_descriptions),
        ))
    }

    async fn batch_search_function_descriptions(
        &self,
        request: tonic::Request<BatchSearchFunctionDescriptionsRequest>,
    ) -> Result<tonic::Response<BatchSearchFunctionDescriptionsResponse>, tonic::Status> {
        const BATCH_LIMIT: usize = 100;

        let request = request.into_inner();
        if request.selectors.len() > BATCH_LIMIT {
            return Err(tonic::Status::invalid_argument(format!(
                "too many selectors: {} requested, at most {BATCH_LIMIT} allowed",
                request.selectors.len()
            )));
        }
        let selectors = request
            .selectors
            .iter()
            .map(|selector| parse_function_selector(selector))
            .collect::<Result<Vec<_>, _>>()?;

        let responses: Vec<_> =
            search::find_function_descriptions(self.client.db_client.as_ref(), selectors)
                .await
                .map_err(|err| tonic::Status::internal(err.to_string()))?
                .into_iter()
                .map(function_descriptions_to_search_response)
                .collect();

        Ok(tonic::Response::new(
            BatchSearchFunctionDescriptionsResponse { responses },
        ))
    }

    async fn get_alliance_stats(
        &self,
        _request: tonic::Request<GetAllianceStatsRequest>,
    ) -> Result<tonic::Response<AllianceStats>, tonic::Status> {
        let result = verification::alliance_stats::stats(self.client.clone())
            .await
            .map_err(|err| {
                tonic::Status::internal(format!(
                    "Error while retrieving verifier alliance stats: {err}"
                ))
            })?
            .map(|stats| AllianceStats {
                total_contracts: stats.total_contracts,
                contracts_per_provider: stats.contracts_per_provider,
            })
            .unwrap_or_default();

        Ok(tonic::Response::new(result))
    }
}

impl DatabaseService {
    async fn search_sources_internal(
        &self,
        bytecode_type: BytecodeType,
        bytecode: &str,
    ) -> Result<Vec<Source>, tonic::Status> {
        let code_type = BytecodeTypeWrapper::from_inner(bytecode_type).try_into()?;
        let code = DisplayBytes::from_str(bytecode)
            .map_err(|err| tonic::Status::invalid_argument(format!("Invalid bytecode: {err}")))?
            .0;

        let mut matches =
            search::eth_bytecode_db_find_contract(self.client.db_client.as_ref(), code_type, code)
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

    async fn search_sourcify_sources_internal(
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
            .get_contract_v2(chain_id, contract_address)
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

    async fn search_alliance_sources_internal(
        &self,
        alliance_db_client: Arc<ReadWriteRepo>,
        chain_id: &str,
        contract_address: &str,
    ) -> Result<Vec<Source>, tonic::Status> {
        let chain_id = i64::from_str(chain_id)
            .map_err(|err| tonic::Status::invalid_argument(format!("Invalid chain id: {err}")))?;
        let contract_address = DisplayBytes::from_str(contract_address)
            .map_err(|err| {
                tonic::Status::invalid_argument(format!("Invalid contract address: {err}"))
            })?
            .0;

        let sources = search::alliance_db_find_contract(
            alliance_db_client.read_db(),
            chain_id,
            contract_address.to_vec(),
        )
        .await
        .map_err(|err| {
            tracing::error!("error while retrieving alliance sources: {err:#?}");
            tonic::Status::internal(err.to_string())
        })?
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
        // A plain contract lookup never runs a verification job, so this
        // terminal-outcome variant is not expected here; treat it defensively as
        // a server-side error.
        sourcify::Error::Sourcify(sourcify::SourcifyError::VerificationFailure(_)) => {
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

fn event_descriptions_to_search_response(
    event_descriptions: Vec<eth_bytecode_db::search::EventDescription>,
) -> SearchEventDescriptionsResponse {
    SearchEventDescriptionsResponse {
        event_descriptions: event_descriptions
            .into_iter()
            .map(|event| EventDescriptionWrapper::from(event).into())
            .collect(),
    }
}

fn function_descriptions_to_search_response(
    function_descriptions: Vec<eth_bytecode_db::search::FunctionDescription>,
) -> SearchFunctionDescriptionsResponse {
    SearchFunctionDescriptionsResponse {
        function_descriptions: function_descriptions
            .into_iter()
            .map(|function| FunctionDescriptionWrapper::from(function).into())
            .collect(),
    }
}

// todo: remove once https://github.com/hyperium/tonic/pull/2282 is released
#[allow(clippy::result_large_err)]
fn parse_function_selector(selector: &str) -> Result<Selector, tonic::Status> {
    Selector::from_str(selector)
        .map_err(|err| tonic::Status::invalid_argument(format!("selector is not valid: {err}")))
}
