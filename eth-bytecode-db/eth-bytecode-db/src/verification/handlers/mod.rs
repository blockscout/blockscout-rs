pub mod alliance_stats;
pub mod compiler_versions;
pub mod import_existing_abis;
pub mod solidity_multi_part;
pub mod solidity_standard_json;
pub mod sourcify;
pub mod sourcify_from_etherscan;
pub mod verifier_alliance;
pub mod vyper_multi_part;
pub mod vyper_standard_json;

////////////////////////////////////////////////////////////////////////////////////////////

use super::{
    db,
    errors::Error,
    smart_contract_verifier,
    types::{BytecodeType, DatabaseReadySource, Source, VerificationMetadata, VerificationType},
    AllianceBatchImportResult, AllianceContractImportResult,
};
use crate::verification::types::AllianceContract;
use anyhow::Context;
use blockscout_display_bytes::ToHex;
use sea_orm::DatabaseConnection;
use std::str::FromStr;
use verifier_alliance_database::{
    ContractDeployment, InsertContractDeployment, RetrieveContractDeployment,
    VerifiedContractMatches,
};

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
        runtime_code: Option<bytes::Bytes>,
    },
    SaveWithDeploymentData {
        db_client: &'a DatabaseConnection,
        deployment_data: AllianceContract,
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
                // Contract deployment must have runtime code to exist (it may be empty, though)
                if is_authorized && runtime_code.is_some() {
                    Self::SaveWithDeploymentData {
                        db_client,
                        deployment_data: AllianceContract {
                            chain_id: format!("{chain_id}"),
                            contract_address,
                            transaction_hash,
                            block_number,
                            transaction_index,
                            deployer,
                            creation_code,
                            runtime_code: runtime_code.unwrap(),
                        },
                    }
                } else {
                    Self::SaveIfDeploymentExists {
                        db_client,
                        chain_id,
                        contract_address,
                        transaction_hash,
                        runtime_code,
                    }
                }
            }
            _ => Self::IgnoreDb,
        }
    }
}

impl VerifierAllianceDbAction<'_> {
    fn contract_address(&self) -> Option<blockscout_display_bytes::Bytes> {
        match self {
            VerifierAllianceDbAction::IgnoreDb => None,
            VerifierAllianceDbAction::SaveIfDeploymentExists {
                contract_address, ..
            } => Some(contract_address),
            VerifierAllianceDbAction::SaveWithDeploymentData {
                deployment_data:
                    AllianceContract {
                        contract_address, ..
                    },
                ..
            } => Some(contract_address),
        }
        .map(|contract_address| blockscout_display_bytes::Bytes::from(contract_address.to_vec()))
    }

    fn chain_id(&self) -> Option<i64> {
        match self {
            VerifierAllianceDbAction::IgnoreDb => None,
            VerifierAllianceDbAction::SaveIfDeploymentExists { chain_id, .. } => Some(*chain_id),
            VerifierAllianceDbAction::SaveWithDeploymentData {
                deployment_data: AllianceContract { chain_id, .. },
                ..
            } => Some(i64::from_str(chain_id).unwrap()),
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
    let (db_client, contract_deployment) = match retrieve_deployment_from_action(action).await? {
        None => return Ok(()),
        Some(result) => result,
    };

    let database_source = DatabaseReadySource::try_from(source)
        .context("Converting source into database ready version")?;

    let matches = check_code_matches(&database_source, &contract_deployment).await?;

    let verified_contract: verifier_alliance_database::VerifiedContract =
        verifier_alliance_database::VerifiedContract {
            contract_deployment_id: contract_deployment.id,
            compiled_contract: database_source
                .try_into()
                .context("converting database source into alliance compiled contract")?,
            matches,
        };
    verifier_alliance_database::insert_verified_contract(db_client, verified_contract)
        .await
        .context("insert data into verifier alliance database")
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

async fn retrieve_deployment_from_action(
    action: VerifierAllianceDbAction<'_>,
) -> Result<Option<(&DatabaseConnection, ContractDeployment)>, anyhow::Error> {
    match action {
        VerifierAllianceDbAction::IgnoreDb => Ok(None),
        VerifierAllianceDbAction::SaveIfDeploymentExists {
            db_client,
            chain_id,
            contract_address,
            transaction_hash,
            runtime_code,
            ..
        } => {
            let chain_id: u128 = chain_id
                .try_into()
                .context("parsing metadata: invalid chain_id")?;
            let deployment_data = match (&transaction_hash, &runtime_code) {
                (Some(transaction_hash), _) => RetrieveContractDeployment::regular(
                    chain_id,
                    contract_address.to_vec(),
                    transaction_hash.to_vec(),
                ),
                (None, Some(runtime_code)) => RetrieveContractDeployment::genesis(
                    chain_id,
                    contract_address.to_vec(),
                    runtime_code.to_vec(),
                ),
                (None, None) => {
                    tracing::warn!(
                        chain_id = chain_id,
                        contract_address = contract_address.to_hex(),
                        "trying to save contract without transaction hash and runtime code"
                    );
                    return Ok(None);
                }
            };

            let contract_deployment = verifier_alliance_database::find_contract_deployment(db_client, deployment_data).await
                .context("retrieve contract deployment")?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "contract deployment was not found: chain_id={}, address={}, transaction_hash={:?}",
                        chain_id,
                        contract_address.to_hex(),
                        transaction_hash.as_ref().map(ToHex::to_hex),
                    )
                })?;

            Ok(Some((db_client, contract_deployment)))
        }
        VerifierAllianceDbAction::SaveWithDeploymentData {
            db_client,
            deployment_data,
        } => {
            let contract_deployment = save_deployment_data(db_client, deployment_data).await?;

            Ok(Some((db_client, contract_deployment)))
        }
    }
}

async fn save_deployment_data(
    db_client: &DatabaseConnection,
    deployment_data: AllianceContract,
) -> Result<ContractDeployment, anyhow::Error> {
    let chain_id = u128::from_str(&deployment_data.chain_id)
        .context("parsing contract metadata: invalid chain_id")?;
    let insert_data = match deployment_data {
        AllianceContract {
            creation_code: Some(creation_code),
            block_number: Some(block_number),
            transaction_hash: Some(transaction_hash),
            transaction_index: Some(transaction_index),
            deployer: Some(deployer),
            ..
        } => InsertContractDeployment::Regular {
            chain_id,
            address: deployment_data.contract_address.to_vec(),
            transaction_hash: transaction_hash.to_vec(),
            block_number: block_number
                .try_into()
                .context("parsing contract metadata: invalid block_number")?,
            transaction_index: transaction_index
                .try_into()
                .context("parsing contract metadata: invalid transaction_index")?,
            deployer: deployer.to_vec(),
            creation_code: creation_code.to_vec(),
            runtime_code: deployment_data.runtime_code.to_vec(),
        },
        AllianceContract {
            creation_code: None,
            block_number: None,
            transaction_hash: None,
            transaction_index: None,
            deployer: None,
            ..
        } => InsertContractDeployment::Genesis {
            chain_id,
            address: deployment_data.contract_address.to_vec(),
            runtime_code: deployment_data.runtime_code.to_vec(),
        },
        _ => {
            anyhow::bail!(
                "parsing contract metadata: contract metadata does not correspond neither to genesis nor regular contract: creation_code_exists={}, block_number_exists={}, transaction_hash_exists={}, transaction_index_exists={}, deployer_exists={}",
                deployment_data.creation_code.is_some(),
                deployment_data.block_number.is_some(),
                deployment_data.transaction_hash.is_some(),
                deployment_data.transaction_index.is_some(),
                deployment_data.deployer.is_some(),
            )
        }
    };

    verifier_alliance_database::insert_contract_deployment(db_client, insert_data)
        .await
        .context("insert contract deployment into verifier alliance database")
}

async fn check_code_matches(
    database_source: &DatabaseReadySource,
    contract_deployment: &ContractDeployment,
) -> Result<VerifiedContractMatches, anyhow::Error> {
    let compilation_artifacts = parse_artifacts_value(
        &database_source.compilation_artifacts,
        "compilation_artifacts",
    )?;
    let creation_code_artifacts = parse_artifacts_value(
        &database_source.creation_code_artifacts,
        "creation_code_artifacts",
    )?;
    let runtime_code_artifacts = parse_artifacts_value(
        &database_source.runtime_code_artifacts,
        "runtime_code_artifacts",
    )?;

    let creation_code_match = match &contract_deployment.creation_code {
        None => None,
        Some(code) => verification_common::verifier_alliance::verify_creation_code(
            code,
            database_source.raw_creation_code.clone(),
            &creation_code_artifacts,
            &compilation_artifacts,
        )
        .context("verify if creation code match")?,
    };
    let runtime_code_match = verification_common::verifier_alliance::verify_runtime_code(
        &contract_deployment.runtime_code,
        database_source.raw_runtime_code.clone(),
        &runtime_code_artifacts,
    )
    .context("verify if runtime code match")?;

    match (creation_code_match, runtime_code_match) {
        (Some(creation_code_match), Some(runtime_code_match)) => {
            Ok(VerifiedContractMatches::Complete {
                creation_match: creation_code_match,
                runtime_match: runtime_code_match,
            })
        }
        (Some(creation_code_match), None) => Ok(VerifiedContractMatches::OnlyCreation {
            creation_match: creation_code_match,
        }),
        (None, Some(runtime_code_match)) => Ok(VerifiedContractMatches::OnlyRuntime {
            runtime_match: runtime_code_match,
        }),
        (None, None) => Err(anyhow::anyhow!(
            "Neither creation code nor runtime code have not matched"
        )),
    }
}

fn parse_artifacts_value<T: for<'de> serde::Deserialize<'de>>(
    value: &Option<serde_json::Value>,
    label: &'static str,
) -> Result<T, anyhow::Error> {
    let value = value
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("{label} are missing"))?;
    Ok(serde::Deserialize::deserialize(value)?)
}

async fn process_batch_import_response(
    eth_bytecode_db_client: &DatabaseConnection,
    alliance_db_client: &DatabaseConnection,
    response: smart_contract_verifier::BatchVerifyResponse,
    deployment_data: Vec<AllianceContract>,
) -> Result<AllianceBatchImportResult, Error> {
    let mut import_result = response.try_into()?;

    if let AllianceBatchImportResult::Results(results) = &mut import_result {
        for (contract_import_result, deployment_data) in results.iter_mut().zip(deployment_data) {
            if let AllianceContractImportResult::Success(success) = contract_import_result {
                let contract_address = deployment_data.contract_address.to_hex();
                let chain_id = deployment_data.chain_id.clone();

                let database_source = DatabaseReadySource::try_from(success.clone())
                    .context(
                        "Converting alliance contract import success into database ready version",
                    )
                    .map_err(Error::Internal)?;

                let process_abi_data_future = process_abi_data(
                    database_source.abi.clone().map(|v| v.to_string()),
                    eth_bytecode_db_client,
                );

                let process_eth_bytecode_db_future = process_batch_import_eth_bytecode_db(
                    eth_bytecode_db_client,
                    database_source.clone(),
                );

                let process_alliance_db_future = process_batch_import_verifier_alliance(
                    alliance_db_client,
                    database_source.clone(),
                    deployment_data,
                );

                // We may process insertion into both databases concurrently, as they are independent of one another.
                let (
                    process_abi_data_result,
                    process_eth_bytecode_db_result,
                    process_alliance_db_result,
                ) = futures::future::join3(
                    process_abi_data_future,
                    process_eth_bytecode_db_future,
                    process_alliance_db_future,
                )
                .await;

                let _ = process_abi_data_result.map_err(|err: anyhow::Error| {
                    tracing::error!(
                        contract_address,
                        chain_id,
                        "Error while inserting abi data into database: {err:#}"
                    )
                });
                let _ = process_eth_bytecode_db_result.map_err(|err: anyhow::Error| {
                    tracing::error!(
                        contract_address,
                        chain_id,
                        "Error while inserting contract data into database: {err:#}"
                    )
                });

                if let Err(err) = process_alliance_db_result {
                    tracing::error!(
                        contract_address,
                        chain_id,
                        "Error while inserting contract data into verifier alliance database: {err:#}"
                    );

                    *contract_import_result = AllianceContractImportResult::ImportFailure(
                        err.context("verifier alliance database").to_string(),
                    )
                }
            }
        }
    }

    Ok(import_result)
}

async fn process_batch_import_verifier_alliance(
    db_client: &DatabaseConnection,
    database_source: DatabaseReadySource,
    deployment_data: AllianceContract,
) -> Result<(), anyhow::Error> {
    let contract_deployment = save_deployment_data(db_client, deployment_data).await?;

    let matches = check_code_matches(&database_source, &contract_deployment).await?;

    let verified_contract: verifier_alliance_database::VerifiedContract =
        verifier_alliance_database::VerifiedContract {
            contract_deployment_id: contract_deployment.id,
            compiled_contract: database_source
                .try_into()
                .context("converting database source into alliance compiled contract")?,
            matches,
        };
    verifier_alliance_database::insert_verified_contract(db_client, verified_contract)
        .await
        .context("insert data into verifier alliance database")
}

async fn process_batch_import_eth_bytecode_db(
    db_client: &DatabaseConnection,
    database_source: DatabaseReadySource,
) -> Result<i64, anyhow::Error> {
    db::eth_bytecode_db::insert_data(db_client, database_source)
        .await
        .context("Insert data into database")
}
