use crate::{
    verification::{
        smart_contract_verifier,
        types::{AllianceBatchImportResult, AllianceImportRequest},
        Client, Error,
    },
    ToHex,
};
use eth_bytecode_db_proto::{blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2, tonic};
use serde::{Deserialize, Serialize};
use smart_contract_verifier_proto::http_client::solidity_verifier_client;
use std::{collections::BTreeMap, str::FromStr};
use verifier_alliance_database::ContractDeployment;

fn convert_contracts(contracts: Vec<ContractDeployment>) -> Vec<smart_contract_verifier::Contract> {
    contracts
        .into_iter()
        .map(|v| smart_contract_verifier::Contract {
            creation_code: v.creation_code().map(|v| ToHex::to_hex(&v)),
            runtime_code: Some(v.runtime_code().to_hex()),
            metadata: Some(smart_contract_verifier::VerificationMetadata {
                chain_id: Some(format!("{}", v.chain_id())),
                contract_address: Some(v.address().to_hex()),
            }),
        })
        .collect()
}

fn verifier_alliance_contract_try_into_contract_deployment(
    value: eth_bytecode_db_v2::VerifierAllianceContract,
) -> Result<ContractDeployment, tonic::Status> {
    let str_to_bytes = |value: &str| {
        blockscout_display_bytes::decode_hex(value)
            .map_err(|err| tonic::Status::invalid_argument(err.to_string()))
    };

    let str_to_u128 = |value: &str| {
        u128::from_str(value).map_err(|err| tonic::Status::invalid_argument(err.to_string()))
    };

    let i64_to_u128 = |value: i64| {
        u128::try_from(value).map_err(|err| tonic::Status::invalid_argument(err.to_string()))
    };

    let chain_id = str_to_u128(&value.chain_id)?;
    let address = str_to_bytes(&value.contract_address)?;
    let runtime_code = str_to_bytes(&value.runtime_code)?;

    let contract_deployment = match (
        value.transaction_hash,
        value.block_number,
        value.transaction_index,
        value.deployer,
        value.creation_code,
    ) {
        (None, None, None, None, None) => ContractDeployment::Genesis {
            chain_id,
            address,
            runtime_code,
        },
        (
            Some(transaction_hash),
            Some(block_number),
            Some(transaction_index),
            Some(deployer),
            Some(creation_code),
        ) => ContractDeployment::Regular {
            chain_id,
            address,
            transaction_hash: str_to_bytes(&transaction_hash)?,
            block_number: i64_to_u128(block_number)?,
            transaction_index: i64_to_u128(transaction_index)?,
            deployer: str_to_bytes(&deployer)?,
            creation_code: str_to_bytes(&creation_code)?,
            runtime_code,
        },
        _ => {
            return Err(tonic::Status::invalid_argument(
                "the verifier alliance contract is neither genesis, nor regular one",
            ))
        }
    };

    Ok(contract_deployment)
}

/******************** Solidity Standard Json ********************/

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardJson {
    pub input: String,
}

impl From<AllianceImportRequest<StandardJson>>
    for smart_contract_verifier::BatchVerifySolidityStandardJsonRequest
{
    fn from(value: AllianceImportRequest<StandardJson>) -> Self {
        Self {
            contracts: convert_contracts(value.contracts),
            compiler_version: value.compiler_version,
            input: value.content.input,
        }
    }
}

impl TryFrom<eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityStandardJsonRequest>
    for AllianceImportRequest<StandardJson>
{
    type Error = eth_bytecode_db_proto::tonic::Status;

    fn try_from(
        value: eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityStandardJsonRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            contracts: value
                .contracts
                .into_iter()
                .map(verifier_alliance_contract_try_into_contract_deployment)
                .collect::<Result<Vec<_>, _>>()?,
            compiler_version: value.compiler_version,
            content: StandardJson { input: value.input },
        })
    }
}

pub async fn import_solidity_standard_json(
    client: Client,
    request: AllianceImportRequest<StandardJson>,
) -> Result<AllianceBatchImportResult, Error> {
    let deployment_data = request.contracts.clone();

    let verifier_request = request.into();
    let verifier_response = solidity_verifier_client::batch_verify_standard_json(
        &client.verifier_http_client,
        verifier_request,
    )
    .await?;

    let result = super::process_batch_import_response(
        client.db_client.as_ref(),
        client.alliance_db_client.as_ref().unwrap(),
        verifier_response,
        deployment_data,
    )
    .await?;

    Ok(result)
}

/******************** Solidity Multi-Part ********************/

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiPart {
    pub sources: BTreeMap<String, String>,
    pub evm_version: Option<String>,
    pub optimization_runs: Option<u32>,
    pub libraries: BTreeMap<String, String>,
}

impl From<AllianceImportRequest<MultiPart>>
    for smart_contract_verifier::BatchVerifySolidityMultiPartRequest
{
    fn from(value: AllianceImportRequest<MultiPart>) -> Self {
        Self {
            contracts: convert_contracts(value.contracts),
            compiler_version: value.compiler_version,
            sources: value.content.sources,
            evm_version: value.content.evm_version,
            optimization_runs: value.content.optimization_runs,
            libraries: value.content.libraries,
        }
    }
}

impl TryFrom<eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityMultiPartRequest>
    for AllianceImportRequest<MultiPart>
{
    type Error = eth_bytecode_db_proto::tonic::Status;

    fn try_from(
        value: eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityMultiPartRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            contracts: value
                .contracts
                .into_iter()
                .map(verifier_alliance_contract_try_into_contract_deployment)
                .collect::<Result<Vec<_>, _>>()?,
            compiler_version: value.compiler_version,
            content: MultiPart {
                sources: value.source_files,
                evm_version: value.evm_version,
                optimization_runs: value.optimization_runs,
                libraries: value.libraries,
            },
        })
    }
}

pub async fn import_solidity_multi_part(
    client: Client,
    request: AllianceImportRequest<MultiPart>,
) -> Result<AllianceBatchImportResult, Error> {
    let deployment_data = request.contracts.clone();

    let verifier_request = request.into();
    let verifier_response = solidity_verifier_client::batch_verify_multi_part(
        &client.verifier_http_client,
        verifier_request,
    )
    .await?;

    let result = super::process_batch_import_response(
        client.db_client.as_ref(),
        client.alliance_db_client.as_ref().unwrap(),
        verifier_response,
        deployment_data,
    )
    .await?;

    Ok(result)
}
