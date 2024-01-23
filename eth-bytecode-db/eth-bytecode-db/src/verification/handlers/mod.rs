pub mod compiler_versions;
pub mod import_existing_abis;
pub mod solidity_multi_part;
pub mod solidity_standard_json;
pub mod sourcify;
pub mod sourcify_from_etherscan;
pub mod vyper_multi_part;
pub mod vyper_standard_json;

////////////////////////////////////////////////////////////////////////////////////////////

use super::{
    db,
    errors::Error,
    smart_contract_verifier,
    types::{BytecodeType, DatabaseReadySource, Source, VerificationMetadata, VerificationType},
    verifier_alliance,
};
use anyhow::Context;
use sea_orm::DatabaseConnection;

enum EthBytecodeDbAction<'a> {
    SaveOnlyAbiData {
        db_client: &'a DatabaseConnection,
        verification_metadata: Option<VerificationMetadata>,
    },
    SaveData {
        db_client: &'a DatabaseConnection,
        bytecode_type: BytecodeType,
        raw_request_bytecode: Vec<u8>,
        verification_settings: serde_json::Value,
        verification_type: VerificationType,
        verification_metadata: Option<VerificationMetadata>,
    },
}

impl<'a> EthBytecodeDbAction<'a> {
    fn db_client(&self) -> &'a DatabaseConnection {
        match self {
            EthBytecodeDbAction::SaveOnlyAbiData { db_client, .. } => db_client,
            EthBytecodeDbAction::SaveData { db_client, .. } => db_client,
        }
    }
    fn contract_address(&self) -> Option<blockscout_display_bytes::Bytes> {
        let contract_address = match self {
            EthBytecodeDbAction::SaveOnlyAbiData {
                verification_metadata:
                    Some(VerificationMetadata {
                        contract_address: Some(contract_address),
                        ..
                    }),
                ..
            } => Some(contract_address),
            EthBytecodeDbAction::SaveData {
                verification_metadata:
                    Some(VerificationMetadata {
                        contract_address: Some(contract_address),
                        ..
                    }),
                ..
            } => Some(contract_address),
            _ => None,
        };
        contract_address.map(|contract_address| {
            blockscout_display_bytes::Bytes::from(contract_address.to_vec())
        })
    }

    fn chain_id(&self) -> Option<i64> {
        match self {
            EthBytecodeDbAction::SaveOnlyAbiData {
                verification_metadata:
                    Some(VerificationMetadata {
                        chain_id: Some(chain_id),
                        ..
                    }),
                ..
            } => Some(*chain_id),
            EthBytecodeDbAction::SaveData {
                verification_metadata:
                    Some(VerificationMetadata {
                        chain_id: Some(chain_id),
                        ..
                    }),
                ..
            } => Some(*chain_id),
            _ => None,
        }
    }
}

enum VerifierAllianceDbAction<'a> {
    IgnoreDb,
    SaveIfDeploymentExists {
        db_client: &'a DatabaseConnection,
        chain_id: i64,
        contract_address: bytes::Bytes,
        transaction_hash: Option<bytes::Bytes>,
        creation_code: Option<bytes::Bytes>,
        runtime_code: Option<bytes::Bytes>,
    },
    SaveWithDeploymentData {
        db_client: &'a DatabaseConnection,
        chain_id: i64,
        contract_address: bytes::Bytes,
        transaction_hash: Option<bytes::Bytes>,
        block_number: Option<i64>,
        transaction_index: Option<i64>,
        deployer: Option<bytes::Bytes>,
        creation_code: Option<bytes::Bytes>,
        runtime_code: Option<bytes::Bytes>,
    },
}

impl<'a> VerifierAllianceDbAction<'a> {
    pub fn from_db_client_and_metadata(
        db_client: Option<&'a DatabaseConnection>,
        verification_metadata: Option<VerificationMetadata>,
        is_authorized: bool,
    ) -> Self {
        match (db_client, verification_metadata) {
            (
                Some(db_client),
                Some(VerificationMetadata {
                    chain_id: Some(chain_id),
                    contract_address: Some(contract_address),
                    transaction_hash,
                    block_number,
                    transaction_index,
                    deployer,
                    creation_code,
                    runtime_code,
                }),
            ) => {
                // Contract deployment must have at least one of creation/runtime code to exist
                if is_authorized && (creation_code.is_some() || runtime_code.is_some()) {
                    Self::SaveWithDeploymentData {
                        db_client,
                        chain_id,
                        contract_address,
                        transaction_hash,
                        block_number,
                        transaction_index,
                        deployer,
                        creation_code,
                        runtime_code,
                    }
                } else {
                    Self::SaveIfDeploymentExists {
                        db_client,
                        chain_id,
                        contract_address,
                        transaction_hash,
                        creation_code,
                        runtime_code,
                    }
                }
            }
            _ => Self::IgnoreDb,
        }
    }
}

impl<'a> VerifierAllianceDbAction<'a> {
    fn contract_address(&self) -> Option<blockscout_display_bytes::Bytes> {
        match self {
            VerifierAllianceDbAction::IgnoreDb => None,
            VerifierAllianceDbAction::SaveIfDeploymentExists {
                contract_address, ..
            } => Some(contract_address),
            VerifierAllianceDbAction::SaveWithDeploymentData {
                contract_address, ..
            } => Some(contract_address),
        }
        .map(|contract_address| blockscout_display_bytes::Bytes::from(contract_address.to_vec()))
    }

    fn chain_id(&self) -> Option<i64> {
        match self {
            VerifierAllianceDbAction::IgnoreDb => None,
            VerifierAllianceDbAction::SaveIfDeploymentExists { chain_id, .. } => Some(*chain_id),
            VerifierAllianceDbAction::SaveWithDeploymentData { chain_id, .. } => Some(*chain_id),
        }
    }
}

async fn process_verify_response(
    response: smart_contract_verifier::VerifyResponse,
    eth_bytecode_db_action: EthBytecodeDbAction<'_>,
    alliance_db_action: VerifierAllianceDbAction<'_>,
) -> Result<Source, Error> {
    let source = from_response_to_source(response).await?;

    let eth_bytecode_db_action_contract_address = eth_bytecode_db_action.contract_address();
    let eth_bytecode_db_action_chain_id = eth_bytecode_db_action.chain_id();

    let alliance_db_action_contract_address = alliance_db_action.contract_address();
    let alliance_db_action_chain_id = alliance_db_action.chain_id();

    let process_abi_data_future =
        process_abi_data(source.abi.clone(), eth_bytecode_db_action.db_client());

    let process_eth_bytecode_db_future =
        process_eth_bytecode_db_action(source.clone(), eth_bytecode_db_action);

    let process_alliance_db_future =
        process_verifier_alliance_db_action(source.clone(), alliance_db_action);

    // We may process insertion into both databases concurrently, as they are independent from one another.
    let (process_abi_data_result, process_eth_bytecode_db_result, process_alliance_db_result) =
        futures::future::join3(
            process_abi_data_future,
            process_eth_bytecode_db_future,
            process_alliance_db_future,
        )
        .await;
    let _ = process_abi_data_result.map_err(|err: anyhow::Error| {
        tracing::error!(
            ?eth_bytecode_db_action_contract_address,
            ?eth_bytecode_db_action_chain_id,
            "Error while inserting abi data into database: {err:#}"
        )
    });
    let _ = process_eth_bytecode_db_result.map_err(|err: anyhow::Error| {
        tracing::error!(
            ?eth_bytecode_db_action_contract_address,
            ?eth_bytecode_db_action_chain_id,
            "Error while inserting contract data into database: {err:#}"
        )
    });
    let _ = process_alliance_db_result.map_err(|err: anyhow::Error| {
        tracing::error!(
            ?alliance_db_action_contract_address,
            ?alliance_db_action_chain_id,
            "Error while inserting contract data into verifier alliance database: {err:#}"
        )
    });

    Ok(source)
}

async fn from_response_to_source(
    response: smart_contract_verifier::VerifyResponse,
) -> Result<Source, Error> {
    let (source, extra_data) = match (response.status(), response.source, response.extra_data) {
        (smart_contract_verifier::Status::Success, Some(source), Some(extra_data)) => {
            (source, extra_data)
        }
        (smart_contract_verifier::Status::Failure, _, _) => {
            return Err(Error::VerificationFailed {
                message: response.message,
            })
        }
        _ => {
            return Err(Error::Internal(
                anyhow::anyhow!("invalid status: {}", response.status)
                    .context("verifier service connection"),
            ))
        }
    };

    Source::try_from((source, extra_data)).map_err(Error::Internal)
}

async fn process_eth_bytecode_db_action(
    source: Source,
    action: EthBytecodeDbAction<'_>,
) -> Result<(), anyhow::Error> {
    match action {
        EthBytecodeDbAction::SaveOnlyAbiData { .. } => {}
        EthBytecodeDbAction::SaveData {
            db_client,
            bytecode_type,
            raw_request_bytecode,
            verification_settings,
            verification_type,
            verification_metadata,
        } => {
            let database_source = DatabaseReadySource::try_from(source)
                .context("Converting source into database ready version")?;
            let source_id = db::eth_bytecode_db::insert_data(db_client, database_source)
                .await
                .context("Insert data into database")?;

            // For historical data we just log any errors but do not propagate them further
            db::eth_bytecode_db::insert_verified_contract_data(
                db_client,
                source_id,
                raw_request_bytecode,
                bytecode_type,
                verification_settings,
                verification_type,
                verification_metadata.clone(),
            )
            .await
            .context("Insert verified contract data")?;
        }
    };

    Ok(())
}

async fn process_verifier_alliance_db_action(
    source: Source,
    action: VerifierAllianceDbAction<'_>,
) -> Result<(), anyhow::Error> {
    let derive_transaction_hash =
        |transaction_hash: Option<bytes::Bytes>,
         creation_code: Option<bytes::Bytes>,
         runtime_code: Option<bytes::Bytes>| {
            match transaction_hash {
                Some(hash) => Some(hash.to_vec()),
                None if creation_code.is_some() || runtime_code.is_some() => {
                    let combined_hash: Vec<_> = creation_code
                        .unwrap_or_default()
                        .into_iter()
                        .chain(runtime_code.unwrap_or_default())
                        .collect();
                    Some(keccak_hash::keccak(combined_hash).0.to_vec())
                }
                None => None,
            }
        };

    let (db_client, contract_deployment) = match action {
        VerifierAllianceDbAction::IgnoreDb => return Ok(()),
        VerifierAllianceDbAction::SaveIfDeploymentExists {
            db_client,
            chain_id,
            contract_address,
            transaction_hash,
            creation_code,
            runtime_code,
        } => {
            let transaction_hash = match derive_transaction_hash(
                transaction_hash,
                creation_code,
                runtime_code,
            ) {
                Some(hash) => hash,
                // If no transaction hash can be derived,
                // consider it like no active deployment exists.
                None => {
                    tracing::warn!(
                        chain_id=chain_id,
                        contract_address=blockscout_display_bytes::Bytes::from(contract_address.to_vec()).to_string(),
                        "Trying to save contract without transaction hash and creation and runtime codes"
                    );
                    return Ok(());
                }
            };
            let deployment_data = db::verifier_alliance_db::ContractDeploymentData {
                chain_id,
                contract_address: contract_address.to_vec(),
                transaction_hash,
                ..Default::default()
            };

            let contract_deployment = db::verifier_alliance_db::retrieve_contract_deployment(db_client, &deployment_data)
                .await
                .context("retrieve contract contract_deployment")?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "contract deployment was not found: chain_id={}, address={}, transaction_hash={}",
                        deployment_data.chain_id,
                        format!("0x{}", hex::encode(&deployment_data.contract_address)),
                        format!("0x{}", hex::encode(&deployment_data.transaction_hash)),
                    )
                })?;

            (db_client, contract_deployment)
        }
        VerifierAllianceDbAction::SaveWithDeploymentData {
            db_client,
            chain_id,
            contract_address,
            transaction_hash,
            block_number,
            transaction_index,
            deployer,
            creation_code,
            runtime_code,
        } => {
            // At least one of creation and runtime code should exist to add the contract into the database.
            let transaction_hash = derive_transaction_hash(
                transaction_hash.clone(),
                creation_code.clone(),
                runtime_code.clone(),
            )
            .ok_or_else(|| anyhow::anyhow!("Both creation and runtime codes are nulls"))?;

            let deployment_data = db::verifier_alliance_db::ContractDeploymentData {
                chain_id,
                contract_address: contract_address.to_vec(),
                transaction_hash,
                block_number,
                transaction_index,
                deployer: deployer.map(|deployer| deployer.to_vec()),
                creation_code: creation_code.map(|code| code.to_vec()),
                runtime_code: runtime_code.map(|code| code.to_vec()),
            };

            let contract_deployment = db::verifier_alliance_db::insert_deployment_data(
                db_client,
                deployment_data.clone(),
            )
            .await
            .context("Insert deployment data into verifier alliance database")?;

            (db_client, contract_deployment)
        }
    };

    let database_source = DatabaseReadySource::try_from(source)
        .context("Converting source into database ready version")?;

    let (deployed_creation_code, deployed_runtime_code) =
        db::verifier_alliance_db::retrieve_contract_codes(db_client, &contract_deployment)
            .await
            .context("retrieve deployment contract codes")?;

    let creation_code_match = verifier_alliance::verify_creation_code(
        &contract_deployment,
        deployed_creation_code.code.clone(),
        database_source.raw_creation_code.clone(),
        database_source.creation_code_artifacts.clone(),
    )
    .context("verify if creation code match")?;

    let runtime_code_match = verifier_alliance::verify_runtime_code(
        &contract_deployment,
        deployed_runtime_code.code.clone(),
        database_source.raw_runtime_code.clone(),
        database_source.runtime_code_artifacts.clone(),
    )
    .context("verify if creation code match")?;

    if !(creation_code_match.does_match || runtime_code_match.does_match) {
        return Err(anyhow::anyhow!(
            "Neither creation code nor runtime code have not matched"
        ));
    }

    let (creation_code_max_status, runtime_code_max_status) = {
        let deployment_verified_contracts =
            db::verifier_alliance_db::retrieve_deployment_verified_contracts(
                db_client,
                &contract_deployment,
            )
            .await
            .context("retrieve deployment verified contracts")?;

        let calculate_max_status = |is_creation_code: bool| {
            deployment_verified_contracts
                .iter()
                .map(|verified_contract| {
                    let (does_match, values) = if is_creation_code {
                        (
                            verified_contract.creation_match,
                            verified_contract.creation_values.as_ref(),
                        )
                    } else {
                        (
                            verified_contract.runtime_match,
                            verified_contract.runtime_values.as_ref(),
                        )
                    };

                    verifier_alliance::retrieve_code_transformation_status(
                        Some(verified_contract.id),
                        is_creation_code,
                        does_match,
                        values,
                    )
                })
                .max()
                .unwrap_or(verifier_alliance::TransformationStatus::NoMatch)
        };

        (calculate_max_status(true), calculate_max_status(false))
    };

    let (creation_code_status, runtime_code_status) = {
        let creation_code_status = verifier_alliance::retrieve_code_transformation_status(
            None,
            true,
            creation_code_match.does_match,
            creation_code_match.values.as_ref(),
        );
        let runtime_code_status = verifier_alliance::retrieve_code_transformation_status(
            None,
            false,
            runtime_code_match.does_match,
            runtime_code_match.values.as_ref(),
        );
        (creation_code_status, runtime_code_status)
    };

    if creation_code_max_status >= creation_code_status
        && runtime_code_max_status >= runtime_code_status
    {
        return Err(anyhow::anyhow!(
            "New verified contract is not better than existing for the given contract deployment"
        ));
    }

    db::verifier_alliance_db::insert_data(
        db_client,
        database_source,
        contract_deployment,
        creation_code_match,
        runtime_code_match,
    )
    .await
    .context("Insert data into verifier alliance database")
}

async fn process_abi_data(
    abi: Option<String>,
    db_client: &DatabaseConnection,
) -> Result<(), anyhow::Error> {
    if abi.is_none() {
        return Ok(());
    }

    // We use `alloy_json_abi::JsonAbi` and not `ethabi::Contract` because
    // `ethabi::Contract` lose the notion of internal type during deserialization
    let abi = alloy_json_abi::JsonAbi::from_json_str(&abi.unwrap()).context("Parse abi")?;

    let events = abi
        .events
        .into_values()
        .flatten()
        .filter(|event| !event.anonymous)
        .collect();
    db::eth_bytecode_db::insert_event_descriptions(db_client, events)
        .await
        .context("Insert event descriptions into database")?;

    Ok(())
}
