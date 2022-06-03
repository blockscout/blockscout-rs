use ethers_solc::artifacts::Metadata;

use crate::http_server::handlers::verification::VerificationResponse;

pub struct MetadataContent<'a>(pub &'a String);

impl<'a> TryFrom<MetadataContent<'a>> for VerificationResponse {
    type Error = anyhow::Error;

    fn try_from(metadata_content: MetadataContent) -> Result<Self, Self::Error> {
        let _metadata: Metadata =
            serde_json::from_str(metadata_content.0.as_str()).map_err(anyhow::Error::msg)?;
        todo!()
    }
}
