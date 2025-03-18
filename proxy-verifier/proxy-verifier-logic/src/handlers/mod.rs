pub mod solidity_verifier_multi_part;
pub mod solidity_verifier_standard_json;

pub mod vyper_verifier_multi_part;
pub mod vyper_verifier_standard_json;

/************************************************/

use crate::{
    address_details, address_details::AddressDetails, to_hex::ToHex, Error, VerificationResponse,
    VerificationSuccess,
};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use std::future::Future;

async fn process_verification_request<'a, Request, RequestBuilder, Verify, VerifyOutput>(
    eth_bytecode_db_client: &'a eth_bytecode_db_proto::http_client::Client,
    contracts: Vec<(&blockscout_client::Client, ethers_core::types::Address)>,
    request_builder: RequestBuilder,
    verify: Verify,
) -> VerificationResponse
where
    RequestBuilder: Fn(
            ethers_core::types::Bytes,
            eth_bytecode_db_v2::BytecodeType,
            eth_bytecode_db_v2::VerificationMetadata,
        ) -> Request
        + Clone,
    Verify: Fn(&'a eth_bytecode_db_proto::http_client::Client, Request) -> VerifyOutput + Clone,
    VerifyOutput: Future<
        Output = eth_bytecode_db_proto::http_client::Result<eth_bytecode_db_v2::VerifyResponse>,
    >,
{
    let contract_details = address_details::batch_retrieve_address_details(&contracts).await;
    if let Some(response) = check_invalid_contracts(&contract_details) {
        return response;
    }

    let mut results = vec![];
    for (contract_details, (blockscout_client, contract_address)) in
        contract_details.into_iter().zip(contracts)
    {
        let result = verify_contract(
            eth_bytecode_db_client,
            contract_details,
            request_builder.clone(),
            verify.clone(),
        )
        .await;
        match result {
            Ok(match_type) => {
                let search_result = search_contract(blockscout_client, contract_address).await;
                let result = search_result.map(|url| VerificationSuccess { url, match_type });
                results.push(result)
            }
            Err(err) if err.is_compilation_failed_error() => {
                return VerificationResponse::CompilationFailed(err)
            }
            Err(err) => results.push(Err(err)),
        }
    }

    VerificationResponse::Results(results)
}

fn check_invalid_contracts(
    contract_details: &[Result<AddressDetails, Error>],
) -> Option<VerificationResponse> {
    let has_invalid_contracts = contract_details.iter().any(|details| {
        details
            .as_ref()
            .is_err_and(|err| err.is_invalid_contract_error())
    });

    if has_invalid_contracts {
        let validation_statuses = contract_details
            .iter()
            .map(|details| match details {
                Err(err) if err.is_invalid_contract_error() => Some(err.clone()),
                _ => None,
            })
            .collect();

        Some(VerificationResponse::InvalidContracts(validation_statuses))
    } else {
        None
    }
}

async fn verify_contract<'a, Request, RequestBuilder, Verify, VerifyOutput>(
    eth_bytecode_db_client: &'a eth_bytecode_db_proto::http_client::Client,
    contract_details: Result<AddressDetails, Error>,
    request_builder: RequestBuilder,
    verify: Verify,
) -> Result<eth_bytecode_db_v2::source::MatchType, Error>
where
    RequestBuilder: Fn(
        ethers_core::types::Bytes,
        eth_bytecode_db_v2::BytecodeType,
        eth_bytecode_db_v2::VerificationMetadata,
    ) -> Request,
    Verify: Fn(&'a eth_bytecode_db_proto::http_client::Client, Request) -> VerifyOutput,
    VerifyOutput: Future<
        Output = eth_bytecode_db_proto::http_client::Result<eth_bytecode_db_v2::VerifyResponse>,
    >,
{
    match contract_details {
        Ok(AddressDetails {
            chain_id,
            address,
            transaction_hash,
            block_number,
            transaction_index,
            deployer,
            creation_code,
            runtime_code,
        }) => {
            let metadata = eth_bytecode_db_v2::VerificationMetadata {
                chain_id: Some(chain_id.clone()),
                contract_address: Some(address.to_hex()),
                transaction_hash: transaction_hash.as_ref().map(ToHex::to_hex),
                block_number: block_number.map(|v| v as i64),
                transaction_index: transaction_index.map(|v| v as i64),
                deployer: deployer.as_ref().map(ToHex::to_hex),
                creation_code: creation_code.as_ref().map(ToHex::to_hex),
                runtime_code: Some(runtime_code.to_hex()),
            };

            let (bytecode, bytecode_type) = if let Some(code) = creation_code {
                (code, eth_bytecode_db_v2::BytecodeType::CreationInput)
            } else {
                (
                    runtime_code,
                    eth_bytecode_db_v2::BytecodeType::DeployedBytecode,
                )
            };
            let eth_bytecode_db_request = request_builder(bytecode, bytecode_type, metadata);

            let eth_bytecode_db_response =
                verify(eth_bytecode_db_client, eth_bytecode_db_request).await;

            process_verify_response(&chain_id, address, eth_bytecode_db_response)
        }
        Err(err) => Err(err),
    }
}

fn process_verify_response(
    chain_id: &str,
    contract_address: ethers_core::types::Address,
    response: Result<eth_bytecode_db_v2::VerifyResponse, eth_bytecode_db_proto::http_client::Error>,
) -> Result<eth_bytecode_db_v2::source::MatchType, Error> {
    match response {
        Ok(response)
            if response.status == eth_bytecode_db_v2::verify_response::Status::Success as i32 =>
        {
            response
                .source
                .map(|value| value.match_type())
                .ok_or_else(|| Error::internal("Eth-bytecode-db returned invalid response"))
        }
        Ok(response)
            if response
                .message
                .contains("No contract could be verified with provided data") =>
        {
            Err(Error::verification_failed(response.message))
        }
        Ok(response) => Err(Error::compilation_failed(response.message)),
        Err(err) => {
            tracing::error!(
                chain_id = chain_id,
                contract_address = contract_address.to_hex(),
                "eth_bytecode_db verification request failed: {err}"
            );
            Err(Error::internal(
                "Error while sending verification request to eth-bytecode-db",
            ))
        }
    }
}

async fn search_contract(
    blockscout_client: &blockscout_client::Client,
    contract_address: ethers_core::types::Address,
) -> Result<String, Error> {
    let search_result =
        blockscout_client::import::smart_contracts::get(blockscout_client, contract_address).await;

    match search_result {
        Ok(response)
            if response.message.contains("Success")
                || response.message.contains("Already verified") =>
        {
            let url =
                blockscout_client.build_url(&format!("/address/{}", contract_address.to_hex()));
            Ok(url.to_string())
        }
        Ok(response) => {
            tracing::error!(
                chain_id = blockscout_client.chain_id(),
                contract_address = contract_address.to_hex(),
                "internal error while retrieving address details: {}",
                response.message
            );
            Err(Error::internal(
                "Contract has not been imported into blockscout",
            ))
        }
        Err(err) => {
            tracing::error!(
                chain_id = blockscout_client.chain_id(),
                contract_address = contract_address.to_hex(),
                "internal error while importing contract into blockscout: {err}"
            );
            Err(Error::internal("Importing contract into blockscout failed"))
        }
    }
}
