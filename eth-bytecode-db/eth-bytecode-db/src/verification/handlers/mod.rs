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
use crate::{
    verification::{types::AllianceContractImportSuccess, verifier_alliance::CodeMatch},
    ToHex,
};
use anyhow::Context;
use sea_orm::{DatabaseConnection, TransactionTrait};
use verifier_alliance_database::{ContractDeployment, RetrieveContractDeployment};
use verifier_alliance_entity::contract_deployments;

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
        deployment_data: RetrieveContractDeployment,
    },
    SaveWithDeploymentData {
        db_client: &'a DatabaseConnection,
        deployment_data: ContractDeployment,
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
                    transaction_hash: Some(transaction_hash),
                    block_number: Some(block_number),
                    transaction_index: Some(transaction_index),
                    deployer: Some(deployer),
                    creation_code: Some(creation_code),
                    runtime_code: Some(runtime_code),
                }),
            ) if is_authorized => {
                let contract_deployment = ContractDeployment::Regular {
                    chain_id: u128::try_from(chain_id).unwrap(),
                    address: contract_address.to_vec(),
                    transaction_hash: transaction_hash.to_vec(),
                    block_number: u128::try_from(block_number).unwrap(),
                    transaction_index: u128::try_from(transaction_index).unwrap(),
                    deployer: deployer.to_vec(),
                    creation_code: creation_code.to_vec(),
                    runtime_code: runtime_code.to_vec(),
                };
                Self::SaveWithDeploymentData {
                    db_client,
                    deployment_data: contract_deployment,
                }
            }
            (
                Some(db_client),
                Some(VerificationMetadata {
                    chain_id: Some(chain_id),
                    contract_address: Some(contract_address),
                    runtime_code: Some(runtime_code),
                    ..
                }),
            ) if is_authorized => {
                let contract_deployment = ContractDeployment::Genesis {
                    chain_id: u128::try_from(chain_id).unwrap(),
                    address: contract_address.to_vec(),
                    runtime_code: runtime_code.to_vec(),
                };
                Self::SaveWithDeploymentData {
                    db_client,
                    deployment_data: contract_deployment,
                }
            }
            (
                Some(db_client),
                Some(VerificationMetadata {
                    chain_id: Some(chain_id),
                    contract_address: Some(contract_address),
                    transaction_hash,
                    runtime_code,
                    ..
                }),
            ) => {
                let chain_id = u128::try_from(chain_id).unwrap();
                let deployment_data = if let Some(transaction_hash) = transaction_hash {
                    RetrieveContractDeployment::regular(
                        chain_id,
                        contract_address.to_vec(),
                        transaction_hash.to_vec(),
                    )
                } else if let Some(runtime_code) = runtime_code {
                    RetrieveContractDeployment::genesis(
                        chain_id,
                        contract_address.to_vec(),
                        runtime_code.to_vec(),
                    )
                } else {
                    tracing::warn!(
                        chain_id=chain_id,
                        contract_address=contract_address.to_hex(),
                        "Trying to save into verifier alliance database contract without transaction hash and runtime code"
                    );
                    return Self::IgnoreDb;
                };
                Self::SaveIfDeploymentExists {
                    db_client,
                    deployment_data,
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
                deployment_data, ..
            } => Some(deployment_data.address()),
            VerifierAllianceDbAction::SaveWithDeploymentData {
                deployment_data, ..
            } => Some(deployment_data.address()),
        }
        .map(|contract_address| blockscout_display_bytes::Bytes::from(contract_address.to_vec()))
    }

    fn chain_id(&self) -> Option<i64> {
        match self {
            VerifierAllianceDbAction::IgnoreDb => None,
            VerifierAllianceDbAction::SaveIfDeploymentExists {
                deployment_data, ..
            } => Some(deployment_data.chain_id()),
            VerifierAllianceDbAction::SaveWithDeploymentData {
                deployment_data, ..
            } => Some(deployment_data.chain_id()),
        }
        .map(|chain_id| i64::try_from(chain_id).unwrap())
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

    let (creation_code_match, runtime_code_match) =
        check_code_matches(db_client, &database_source, &contract_deployment).await?;

    check_match_statuses(
        db_client,
        &contract_deployment,
        &creation_code_match,
        &runtime_code_match,
    )
    .await?;

    db::verifier_alliance_db::insert_data(
        db_client,
        database_source,
        contract_deployment,
        creation_code_match,
        runtime_code_match,
    )
    .await
    .context("Insert data into verifier alliance database")
    .map_err(|err| {
        println!("\n[ERROR]: {err:#?}\n");
        err
    })
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
) -> Result<Option<(&DatabaseConnection, contract_deployments::Model)>, anyhow::Error> {
    match action {
        VerifierAllianceDbAction::IgnoreDb => Ok(None),
        VerifierAllianceDbAction::SaveIfDeploymentExists {
            db_client,
            deployment_data,
        } => {
            let contract_deployment = verifier_alliance_database::retrieve_contract_deployment(
                db_client,
                deployment_data,
            )
            .await
            .context("retrieve contract deployment")?;
            if let Some(contract_deployment) = contract_deployment {
                return Ok(Some((db_client, contract_deployment)));
            }

            tracing::debug!("contract deployment has not been found in the database");
            Ok(None)
        }
        VerifierAllianceDbAction::SaveWithDeploymentData {
            db_client,
            deployment_data,
        } => {
            let contract_deployment = save_contract_deployment(db_client, deployment_data).await?;

            Ok(Some((db_client, contract_deployment)))
        }
    }
}

async fn save_contract_deployment(
    db_client: &DatabaseConnection,
    contract_deployment: ContractDeployment,
) -> Result<contract_deployments::Model, anyhow::Error> {
    let txn = db_client.begin().await.context("begin transaction")?;
    let model = verifier_alliance_database::insert_contract_deployment(&txn, contract_deployment)
        .await
        .context("insert contract deployment")?;
    txn.commit().await.context("commit transaction")?;
    Ok(model)
}

async fn check_code_matches(
    db_client: &DatabaseConnection,
    database_source: &DatabaseReadySource,
    contract_deployment: &contract_deployments::Model,
) -> Result<(CodeMatch, CodeMatch), anyhow::Error> {
    let (deployed_creation_code, deployed_runtime_code) =
        db::verifier_alliance_db::retrieve_contract_codes(db_client, contract_deployment)
            .await
            .context("retrieve deployment contract codes")?;

    let creation_code_match = super::verifier_alliance::verify_creation_code(
        contract_deployment,
        deployed_creation_code.code.clone(),
        database_source.raw_creation_code.clone(),
        database_source
            .creation_code_artifacts
            .clone()
            .map(|value| value.into()),
    )
    .context("verify if creation code match")?;

    let runtime_code_match = super::verifier_alliance::verify_runtime_code(
        contract_deployment,
        deployed_runtime_code.code.clone(),
        database_source.raw_runtime_code.clone(),
        database_source
            .runtime_code_artifacts
            .clone()
            .map(|value| value.into()),
    )
    .context("verify if creation code match")?;

    if !(creation_code_match.does_match || runtime_code_match.does_match) {
        return Err(anyhow::anyhow!(
            "Neither creation code nor runtime code have not matched"
        ));
    }

    Ok((creation_code_match, runtime_code_match))
}

async fn check_match_statuses(
    db_client: &DatabaseConnection,
    contract_deployment: &contract_deployments::Model,
    creation_code_match: &CodeMatch,
    runtime_code_match: &CodeMatch,
) -> Result<(), anyhow::Error> {
    let (creation_code_max_status, runtime_code_max_status) = {
        let deployment_verified_contracts =
            db::verifier_alliance_db::retrieve_deployment_verified_contracts(
                db_client,
                contract_deployment,
            )
            .await
            .context("retrieve deployment verified contracts")?;

        (
            super::verifier_alliance::calculate_max_status(&deployment_verified_contracts, true),
            super::verifier_alliance::calculate_max_status(&deployment_verified_contracts, false),
        )
    };

    let (creation_code_status, runtime_code_status) = {
        let creation_code_status = super::verifier_alliance::retrieve_code_transformation_status(
            None,
            true,
            creation_code_match.does_match,
            creation_code_match.values.as_ref(),
        );
        let runtime_code_status = super::verifier_alliance::retrieve_code_transformation_status(
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

    Ok(())
}

async fn process_batch_import_response(
    eth_bytecode_db_client: &DatabaseConnection,
    alliance_db_client: &DatabaseConnection,
    response: smart_contract_verifier::BatchVerifyResponse,
    deployment_data: Vec<ContractDeployment>,
) -> Result<AllianceBatchImportResult, Error> {
    let mut import_result = response.try_into()?;

    if let AllianceBatchImportResult::Results(results) = &mut import_result {
        for (contract_import_result, deployment_data) in results.iter_mut().zip(deployment_data) {
            if let AllianceContractImportResult::Success(success) = contract_import_result {
                let contract_address = deployment_data.address().to_hex();
                let chain_id = deployment_data.chain_id();

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
                    success,
                );

                // We may process insertion into both databases concurrently, as they are independent from one another.
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
    deployment_data: ContractDeployment,
    contract_import_success: &AllianceContractImportSuccess,
) -> Result<(), anyhow::Error> {
    let contract_deployment = save_contract_deployment(db_client, deployment_data).await?;

    let creation_code_match =
        code_match_from_match_details(contract_import_success.creation_match_details.clone());
    let runtime_code_match =
        code_match_from_match_details(contract_import_success.runtime_match_details.clone());

    check_match_statuses(
        db_client,
        &contract_deployment,
        &creation_code_match,
        &runtime_code_match,
    )
    .await?;

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

async fn process_batch_import_eth_bytecode_db(
    db_client: &DatabaseConnection,
    database_source: DatabaseReadySource,
) -> Result<i64, anyhow::Error> {
    db::eth_bytecode_db::insert_data(db_client, database_source)
        .await
        .context("Insert data into database")
}

fn code_match_from_match_details(
    match_details: Option<crate::verification::types::MatchDetails>,
) -> CodeMatch {
    let (does_match, values, transformations) = match_details
        .map(|details| (true, Some(details.values), Some(details.transformations)))
        .unwrap_or_default();

    CodeMatch {
        does_match,
        values,
        transformations,
    }
}
