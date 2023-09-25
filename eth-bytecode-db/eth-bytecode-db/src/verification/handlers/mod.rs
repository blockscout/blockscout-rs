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
    types::{
        BytecodePart, BytecodeType, DatabaseReadySource, Source, VerificationMetadata,
        VerificationType,
    },
};
use anyhow::Context;
use sea_orm::DatabaseConnection;

enum ProcessResponseAction<'a> {
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
}

impl<'a> VerifierAllianceDbAction<'a> {
    pub fn from_db_client_and_metadata(
        db_client: Option<&'a DatabaseConnection>,
        verification_metadata: Option<VerificationMetadata>,
    ) -> Self {
        if db_client.is_none() {
            return Self::IgnoreDb;
        }

        match (db_client, verification_metadata) {
            (
                Some(db_client),
                Some(VerificationMetadata {
                    chain_id: Some(chain_id),
                    contract_address: Some(contract_address),
                    transaction_hash: Some(transaction_hash),
                }),
            ) => Self::SaveIfDeploymentExists {
                db_client,
                chain_id,
                contract_address,
                transaction_hash,
            },
            _ => Self::IgnoreDb,
        }
    }
}

async fn process_verify_response(
    response: smart_contract_verifier::VerifyResponse,
    action: ProcessResponseAction<'_>,
    alliance_db_action: VerifierAllianceDbAction<'_>,
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

    let parse_local_parts = |local_parts: Vec<smart_contract_verifier::BytecodePart>,
                             bytecode_type: &str|
     -> Result<(Vec<BytecodePart>, Vec<u8>), Error> {
        let parts = local_parts
            .into_iter()
            .map(|part| {
                BytecodePart::try_from(part).map_err(|err| {
                    Error::Internal(
                        anyhow::anyhow!("error while decoding local {}: {}", bytecode_type, err,)
                            .context("verifier service connection"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let raw_input = parts
            .iter()
            .flat_map(|part| part.data().to_vec())
            .collect::<Vec<_>>();

        Ok((parts, raw_input))
    };

    let (creation_input_parts, raw_creation_input) =
        parse_local_parts(extra_data.local_creation_input_parts, "creation input")?;
    let (deployed_bytecode_parts, raw_deployed_bytecode) = parse_local_parts(
        extra_data.local_deployed_bytecode_parts,
        "deployed bytecode",
    )?;

    let source_type = source.source_type().try_into().map_err(Error::Internal)?;
    let match_type = source.match_type().into();
    let source = Source {
        file_name: source.file_name,
        contract_name: source.contract_name,
        compiler_version: source.compiler_version,
        compiler_settings: source.compiler_settings,
        source_type,
        source_files: source.source_files,
        abi: source.abi,
        constructor_arguments: source.constructor_arguments,
        match_type,
        compilation_artifacts: source.compilation_artifacts,
        creation_input_artifacts: source.creation_input_artifacts,
        deployed_bytecode_artifacts: source.deployed_bytecode_artifacts,
        raw_creation_input,
        raw_deployed_bytecode,
        creation_input_parts,
        deployed_bytecode_parts,
    };

    let process_database_insertion = || async {
        match action {
            ProcessResponseAction::IgnoreDb => {}
            ProcessResponseAction::SaveData {
                db_client,
                bytecode_type,
                raw_request_bytecode,
                verification_settings,
                verification_type,
                verification_metadata,
            } => {
                let database_source = DatabaseReadySource::try_from(source.clone())
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
    };

    let process_alliance_database_insertion = || async {
        match alliance_db_action {
            VerifierAllianceDbAction::IgnoreDb => {}
            VerifierAllianceDbAction::SaveIfDeploymentExists {
                db_client: alliance_db_client,
                chain_id,
                contract_address,
                transaction_hash,
            } => {
                let database_source = DatabaseReadySource::try_from(source.clone())
                    .context("Converting source into database ready version")?;

                let deployment_data = db::verifier_alliance_db::ContractDeploymentData {
                    chain_id,
                    contract_address: contract_address.to_vec(),
                    transaction_hash: transaction_hash.to_vec(),
                };
                db::verifier_alliance_db::insert_data(
                    alliance_db_client,
                    database_source,
                    deployment_data,
                )
                .await
                .context("Insert data into verifier alliance database")?;
            }
        }

        Ok(())
    };

    let _ = process_database_insertion()
        .await
        .map_err(|err: anyhow::Error| {
            tracing::error!("Error while inserting contract data into database: {err:#}")
        });

    let _ = process_alliance_database_insertion()
        .await
        .map_err(|err: anyhow::Error| {
            tracing::error!(
                "Error while inserting contract data into verifier alliance database: {err:#}"
            )
        });

    Ok(source)
}
