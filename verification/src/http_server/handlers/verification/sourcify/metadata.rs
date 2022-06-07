use ethers_solc::artifacts::Metadata;

use crate::http_server::handlers::verification::VerificationResponse;

use super::types::Files;

const METADATA_FILE_NAME: &str = "metadata.json";

impl TryFrom<Files> for VerificationResponse {
    type Error = anyhow::Error;

    fn try_from(files: Files) -> Result<Self, Self::Error> {
        let metadata = Metadata::try_from(files)?;
        let _contract_name = metadata
            .settings
            .compilation_target
            .iter()
            .next()
            .ok_or_else(|| anyhow::Error::msg("compilation target not found"))?
            .1;
        let _compiler_version = metadata.compiler.version;
        let _evm_version = metadata.settings.inner.evm_version.unwrap_or_default();
        let _contract_libraries = metadata.settings.inner.libraries;
        let _abi = serde_json::to_string(&metadata.output.abi)?;

        // let _sources = files
        //     .0
        //     .into_iter()
        //     .filter(|(name, _)| !name.ends_with(METADATA_FILE_NAME));

        log::info!("{:?}", _abi);

        todo!()
    }
}

impl TryFrom<Files> for Metadata {
    type Error = anyhow::Error;

    fn try_from(files: Files) -> Result<Self, Self::Error> {
        let metadata_content = files
            .0
            .get(METADATA_FILE_NAME)
            .ok_or_else(|| anyhow::Error::msg(format!("file {} not found", METADATA_FILE_NAME)))?;

        serde_json::from_str(metadata_content.as_str()).map_err(Self::Error::msg)
    }
}

// TODO: tests
