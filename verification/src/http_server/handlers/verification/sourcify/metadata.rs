use ethers_solc::artifacts::Metadata;

use crate::http_server::handlers::verification::VerificationResponse;

use crate::http_server::handlers::verification::sourcify;

use super::api::verification_files;

const METADATA_FILE_NAME: &str = "metadata.json";

impl TryFrom<Metadata> for VerificationResponse {
    type Error = anyhow::Error;

    fn try_from(_metadata: Metadata) -> Result<Self, Self::Error> {
        // let _contract_name = metadata
        //     .settings
        //     .compilation_target
        //     .iter()
        //     .next()
        //     .ok_or(Self::Error::msg("compilation target not found"))?
        //     .1;
        // let _compiler_version = metadata.compiler.version;
        // let _evm_version = todo!();
        // let _contract_libraries = todo!();
        // let _abi = serde_json::to_string(&metadata.output.abi)?;
        // let _sources =

        Ok(Self { verified: true })
    }
}

/// Tries to extract and parse metadata.json file from provided files.  
/// If it cannot find metadata.json file, but contract is verified,
/// probably this contract was verified by someone else before,
/// then we need to make request to sourcify api to get metadata.json file.
pub async fn try_extract_metadata(
    params: &sourcify::types::ApiRequest,
    sourcify_api_url: &str,
) -> Result<Metadata, anyhow::Error> {
    if let Some(metadata) = try_extract_provided_metadata(params) {
        Ok(metadata)
    } else {
        try_extract_metadata_from_api(params, sourcify_api_url).await
    }
}

fn try_extract_provided_metadata(params: &sourcify::types::ApiRequest) -> Option<Metadata> {
    params
        .files
        .get(METADATA_FILE_NAME)
        .and_then(|provided_metadata| serde_json::from_str(provided_metadata.as_str()).ok())
}

async fn try_extract_metadata_from_api(
    params: &sourcify::types::ApiRequest,
    sourcify_api_url: &str,
) -> Result<Metadata, anyhow::Error> {
    let response = verification_files(params, sourcify_api_url).await?;
    let metadata_file = response
        .files
        .into_iter()
        .find(|f| f.name == METADATA_FILE_NAME)
        .ok_or_else(|| anyhow::Error::msg(format!("file {} not found", METADATA_FILE_NAME)))?;

    serde_json::from_str(metadata_file.content.as_str()).map_err(anyhow::Error::msg)
}
