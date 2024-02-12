mod health;
mod proxy;
mod solidity_verifier;
mod vyper_verifier;

pub use health::HealthService;
pub use proxy::ProxyService;
pub use solidity_verifier::SolidityVerifierService;
pub use vyper_verifier::VyperVerifierService;

/****************************************************/

use eth_bytecode_db_proto::{
    blockscout::eth_bytecode_db::v2 as eth_bytecode_db_proto_v2,
    http_client as eth_bytecode_db_http,
};
use proxy_verifier_proto::blockscout::proxy_verifier::v1::{
    self as proxy_verifier_proto_v1, verification_response,
};
use std::collections::BTreeMap;
use tonic::{Response, Status};

pub(crate) async fn verify<'a, Request, Verify, VerifyOutput>(
    blockscout_clients: &'a BTreeMap<String, blockscout_client::Client>,
    eth_bytecode_db_client: &'a eth_bytecode_db_proto::http_client::Client,
    contracts: Vec<proxy_verifier_proto_v1::Contract>,
    verification_request: Request,
    verification_function: Verify,
) -> Result<Response<proxy_verifier_proto_v1::VerificationResponse>, Status>
where
    Verify: Fn(
        &'a eth_bytecode_db_proto::http_client::Client,
        Vec<(&'a blockscout_client::Client, ethers_core::types::Address)>,
        Request,
    ) -> VerifyOutput,
    VerifyOutput: std::future::Future<Output = proxy_verifier_logic::VerificationResponse>,
{
    let contracts = contracts_proto_to_inner(blockscout_clients, &contracts)?;

    let response =
        verification_function(eth_bytecode_db_client, contracts, verification_request).await;

    Ok(Response::new(verification_response_inner_to_proto(
        response,
    )))
}

pub(crate) async fn list_compilers<'a, List, ListOutput>(
    eth_bytecode_db_client: &'a eth_bytecode_db_proto::http_client::Client,
    list_compiler_versions: List,
) -> Result<Response<proxy_verifier_proto_v1::ListCompilersResponse>, Status>
where
    List: Fn(
        &'a eth_bytecode_db_proto::http_client::Client,
        eth_bytecode_db_proto_v2::ListCompilerVersionsRequest,
    ) -> ListOutput,
    ListOutput: std::future::Future<
        Output = eth_bytecode_db_http::Result<
            eth_bytecode_db_proto_v2::ListCompilerVersionsResponse,
        >,
    >,
{
    let eth_bytecode_db_request = eth_bytecode_db_proto_v2::ListCompilerVersionsRequest {};
    let eth_bytecode_db_response =
        list_compiler_versions(eth_bytecode_db_client, eth_bytecode_db_request)
            .await
            .map_err(|err| {
                Status::internal(format!(
                    "request to underlying eth-bytecode-db service failed: {err:#}"
                ))
            })?;

    Ok(Response::new(
        proxy_verifier_proto_v1::ListCompilersResponse {
            compilers: eth_bytecode_db_response.compiler_versions,
        },
    ))
}

pub fn contracts_proto_to_inner<'a>(
    blockscout_clients: &'a std::collections::BTreeMap<String, blockscout_client::Client>,
    proto_contracts: &[proxy_verifier_proto_v1::Contract],
) -> Result<Vec<(&'a blockscout_client::Client, ethers_core::types::Address)>, tonic::Status> {
    use std::str::FromStr;

    let mut inner_contracts = vec![];
    for contract in proto_contracts {
        let blockscout_client = blockscout_clients.get(&contract.chain_id).ok_or_else(|| {
            tonic::Status::invalid_argument(format!(
                "chain_id={}; is not supported",
                contract.chain_id
            ))
        })?;
        let contract_address =
            ethers_core::types::Address::from_str(&contract.address).map_err(|err| {
                tonic::Status::invalid_argument(format!(
                    "chain_id={}, address={}; invalid address={err}",
                    contract.chain_id, contract.address
                ))
            })?;
        inner_contracts.push((blockscout_client, contract_address))
    }

    Ok(inner_contracts)
}

pub fn verification_response_inner_to_proto(
    response: proxy_verifier_logic::VerificationResponse,
) -> proxy_verifier_proto_v1::VerificationResponse {
    use proxy_verifier_proto_v1::verification_response::{CompilationFailure, VerificationStatus};

    let verification_status = match response {
        proxy_verifier_logic::VerificationResponse::InvalidContracts(invalid_contracts) => {
            process_invalid_contracts_response(invalid_contracts)
        }
        proxy_verifier_logic::VerificationResponse::CompilationFailed(error) => {
            VerificationStatus::CompilationFailure(CompilationFailure {
                message: error.to_string(),
            })
        }
        proxy_verifier_logic::VerificationResponse::Results(results) => {
            process_results_response(results)
        }
    };

    proxy_verifier_proto_v1::VerificationResponse {
        verification_status: Some(verification_status),
    }
}

fn process_invalid_contracts_response(
    invalid_contracts: Vec<Option<proxy_verifier_logic::Error>>,
) -> verification_response::VerificationStatus {
    use verification_response::{
        contract_validation_results::{contract_validation_result, ContractValidationResult},
        ContractValidationResults, VerificationStatus,
    };

    let items = invalid_contracts
        .into_iter()
        .map(|error| match error {
            None => ContractValidationResult {
                message: "Ok".to_string(),
                status: contract_validation_result::Status::Valid.into(),
            },
            Some(err) if err.is_invalid_contract_error() => ContractValidationResult {
                message: err.to_string(),
                status: contract_validation_result::Status::Invalid.into(),
            },
            Some(err) => ContractValidationResult {
                message: err.to_string(),
                status: contract_validation_result::Status::InternalError.into(),
            },
        })
        .collect();
    VerificationStatus::ContractValidationResults(ContractValidationResults { items })
}

fn process_results_response(
    results: Vec<Result<String, proxy_verifier_logic::Error>>,
) -> verification_response::VerificationStatus {
    use verification_response::{
        contract_validation_results::contract_validation_result,
        contract_verification_results::{contract_verification_result, ContractVerificationResult},
        ContractVerificationResults, VerificationStatus,
    };

    let items = results
        .into_iter()
        .map(|result| match result {
            Ok(url) => ContractVerificationResult {
                message: url,
                status: contract_verification_result::Status::Success.into(),
            },
            Err(err) if err.is_internal_error() => ContractVerificationResult {
                message: err.to_string(),
                status: contract_validation_result::Status::InternalError.into(),
            },
            Err(err) => ContractVerificationResult {
                message: err.to_string(),
                status: contract_verification_result::Status::Failure.into(),
            },
        })
        .collect();
    VerificationStatus::ContractVerificationResults(ContractVerificationResults { items })
}
