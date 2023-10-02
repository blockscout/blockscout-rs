pub mod compiler_versions;
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
};
use anyhow::Context;
use sea_orm::DatabaseConnection;

enum EthBytecodeDbAction<'a> {
    IgnoreDb,
    SaveData {
        db_client: &'a DatabaseConnection,
        bytecode_type: BytecodeType,
        raw_request_bytecode: Vec<u8>,
        verification_settings: serde_json::Value,
        verification_type: VerificationType,
        verification_metadata: Option<VerificationMetadata>,
    },
}

enum VerifierAllianceDbAction<'a> {
    IgnoreDb,
    SaveIfDeploymentExists {
        db_client: &'a DatabaseConnection,
        chain_id: i64,
        contract_address: bytes::Bytes,
        transaction_hash: bytes::Bytes,
    },
    SaveWithDeploymentData {
        db_client: &'a DatabaseConnection,
        chain_id: i64,
        contract_address: bytes::Bytes,
        transaction_hash: bytes::Bytes,
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
                    transaction_hash: Some(transaction_hash),
                    block_number,
                    transaction_index,
                    deployer,
                    creation_code,
                    runtime_code,
                }),
            ) => {
                if is_authorized {
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
                    }
                }
            }
            _ => Self::IgnoreDb,
        }
    }
}

async fn process_verify_response(
    response: smart_contract_verifier::VerifyResponse,
    eth_bytecode_db_action: EthBytecodeDbAction<'_>,
    alliance_db_action: VerifierAllianceDbAction<'_>,
) -> Result<Source, Error> {
    let source = from_response_to_source(response).await?;

    let process_eth_bytecode_db_future =
        process_eth_bytecode_db_action(source.clone(), eth_bytecode_db_action);

    let process_alliance_db_future =
        process_verifier_alliance_db_action(source.clone(), alliance_db_action);

    // We may process insertion into both databases concurrently, as they are independent from one another.
    let (process_eth_bytecode_db_result, process_alliance_db_result) =
        futures::future::join(process_eth_bytecode_db_future, process_alliance_db_future).await;
    let _ = process_eth_bytecode_db_result.map_err(|err: anyhow::Error| {
        tracing::error!("Error while inserting contract data into database: {err:#}")
    });
    let _ = process_alliance_db_result.map_err(|err: anyhow::Error| {
        tracing::error!(
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
        EthBytecodeDbAction::IgnoreDb => {}
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
    match action {
        VerifierAllianceDbAction::IgnoreDb => {}
        VerifierAllianceDbAction::SaveIfDeploymentExists {
            db_client,
            chain_id,
            contract_address,
            transaction_hash,
        } => {
            let database_source = DatabaseReadySource::try_from(source)
                .context("Converting source into database ready version")?;

            let deployment_data = db::verifier_alliance_db::ContractDeploymentData {
                chain_id,
                contract_address: contract_address.to_vec(),
                transaction_hash: transaction_hash.to_vec(),
                ..Default::default()
            };
            db::verifier_alliance_db::insert_data(db_client, database_source, deployment_data)
                .await
                .context("Insert data into verifier alliance database")?;
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
            let database_source = DatabaseReadySource::try_from(source)
                .context("Converting source into database ready version")?;

            let deployment_data = db::verifier_alliance_db::ContractDeploymentData {
                chain_id,
                contract_address: contract_address.to_vec(),
                transaction_hash: transaction_hash.to_vec(),
                block_number,
                transaction_index,
                deployer: deployer.map(|deployer| deployer.to_vec()),
                creation_code: creation_code
                    .as_ref()
                    .map(|creation_code| creation_code.to_vec()),
                runtime_code: runtime_code
                    .as_ref()
                    .map(|runtime_code| runtime_code.to_vec()),
            };

            // At least one of creation and runtime code should exist to add the contract into the database.
            if creation_code.is_some() || runtime_code.is_some() {
                db::verifier_alliance_db::insert_deployment_data(
                    db_client,
                    deployment_data.clone(),
                )
                .await
                .context("Insert deployment data into verifier alliance database")?;
            }
            db::verifier_alliance_db::insert_data(db_client, database_source, deployment_data)
                .await
                .context("Insert data into verifier alliance database")?;
        }
    }

    Ok(())
}
