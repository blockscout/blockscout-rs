pub mod solidity_multi_part;
pub mod solidity_standard_json;
pub mod vyper_multi_part;

////////////////////////////////////////////////////////////////////////////////////////////

use super::{
    db,
    errors::Error,
    smart_contract_verifier,
    types::{BytecodePart, BytecodeType, MatchType, Source, SourceType, VerificationType},
};
use anyhow::Context;
use sea_orm::DatabaseConnection;

async fn process_verify_response(
    db_client: &DatabaseConnection,
    response: smart_contract_verifier::VerifyResponse,
    bytecode_type: BytecodeType,
    raw_request_bytecode: Vec<u8>,
    source_type_fn: fn(&str) -> Result<SourceType, Error>,
    verification_settings: serde_json::Value,
    verification_type: VerificationType,
) -> Result<Source, Error> {
    let result = match response.status.as_str() {
        "0" if response.result.is_some() => response.result.unwrap(),
        "1" => Err(Error::VerificationFailed {
            message: response.message,
        })?,
        _ => Err(Error::Internal(
            anyhow::anyhow!(
                "invalid status: {}. One of \"0\" or \"1\" expected",
                response.status
            )
            .context("verifier service connection"),
        ))?,
    };

    let source_type = source_type_fn(result.file_name.as_str())?;

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
        parse_local_parts(result.local_creation_input_parts, "creation input")?;
    let (deployed_bytecode_parts, raw_deployed_bytecode) =
        parse_local_parts(result.local_deployed_bytecode_parts, "deployed bytecode")?;

    let match_type = MatchType::from(
        smart_contract_verifier::MatchType::from_i32(result.match_type).unwrap_or_default(),
    );

    let source = Source {
        file_name: result.file_name,
        contract_name: result.contract_name,
        compiler_version: result.compiler_version,
        compiler_settings: result.compiler_settings,
        source_type,
        source_files: result.sources,
        abi: result.abi,
        constructor_arguments: result.constructor_arguments,
        match_type,
        raw_creation_input,
        raw_deployed_bytecode,
        creation_input_parts,
        deployed_bytecode_parts,
    };

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

    Ok(source)
}
