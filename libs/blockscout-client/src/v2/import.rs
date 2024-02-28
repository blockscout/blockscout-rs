use super::types;
use crate::{Client, Result};

pub mod smart_contracts {
    pub use super::*;

    pub async fn get(
        client: &Client,
        address_hash: ethers_core::types::Address,
    ) -> Result<types::ImportSmartContractResponse> {
        let path = format!(
            "/api/v2/import/smart-contracts/0x{}",
            hex::encode(address_hash)
        );
        let headers = match client.api_sensitive_endpoints_key() {
            None => vec![],
            Some(key) => vec![("x-api-key", key)],
        };
        client
            .get_request_with_headers(client.build_url(&path), headers)
            .await
    }
}
