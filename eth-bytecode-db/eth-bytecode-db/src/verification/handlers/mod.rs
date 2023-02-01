pub mod compiler_versions;
pub mod solidity_multi_part;
pub mod solidity_standard_json;
pub mod sourcify;
pub mod vyper_multi_part;

////////////////////////////////////////////////////////////////////////////////////////////

use super::{
    db,
    errors::Error,
    smart_contract_verifier,
    types::{BytecodePart, BytecodeType, Source, VerificationType},
};
use anyhow::Context;
use sea_orm::DatabaseConnection;

enum ProcessResponseAction {
    IgnoreDb,
    SaveData {
        bytecode_type: BytecodeType,
        raw_request_bytecode: Vec<u8>,
        verification_settings: serde_json::Value,
        verification_type: VerificationType,
    },
}

async fn process_verify_response(
    db_client: &DatabaseConnection,
    response: smart_contract_verifier::VerifyResponse,
    action: ProcessResponseAction,
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
        raw_creation_input,
        raw_deployed_bytecode,
        creation_input_parts,
        deployed_bytecode_parts,
    };

    match action {
        ProcessResponseAction::IgnoreDb => {}
        ProcessResponseAction::SaveData {
            bytecode_type,
            raw_request_bytecode,
            verification_settings,
            verification_type,
        } => {
            let source_id = db::insert_data(db_client, source.clone())
                .await
                .context("Insert data into database")
                .map_err(Error::Internal)?;

            // For historical data we just log any errors but do not propagate them further
            let _ = db::insert_verified_contract_data(
                db_client,
                source_id,
                raw_request_bytecode,
                bytecode_type,
                verification_settings,
                verification_type,
            )
            .await
            .map_err(|err| tracing::warn!("Error while inserting verified contract data: {}", err));
        }
    }

    Ok(source)
}
