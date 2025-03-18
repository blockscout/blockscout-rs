use crate::{
    protocols::{D3ConnectProtocol, DomainNameOnProtocol},
    subgraph::offchain::Reader,
};
use alloy::sol;
use chrono::{DateTime, Utc};
use serde::Deserialize;

sol! {
    // SPDX-License-Identifier: MIT
    // OpenZeppelin Contracts (last updated v5.0.0) (token/ERC721/extensions/IERC721Metadata.sol)

    pragma solidity ^0.8.20;

    import {IERC721} from "../IERC721.sol";

    interface IERC721Metadata is IERC721 {

        function name() external view returns (string memory);

        function symbol() external view returns (string memory);

        function tokenURI(uint256 tokenId) external view returns (string memory);
    }
}

#[derive(Debug, Deserialize)]
pub struct D3NameMetadata {
    #[allow(dead_code)]
    pub name: String,
    #[serde(default)]
    pub attributes: Vec<D3NameAttribute>,
}

#[derive(Debug, Deserialize)]
pub struct D3NameAttribute {
    pub trait_type: String,
    pub value: serde_json::Value,
    #[serde(default)]
    #[allow(dead_code)]
    pub display_type: Option<String>,
}

pub async fn get_metadata(
    reader: &Reader,
    name: &DomainNameOnProtocol<'_>,
    d3: &D3ConnectProtocol,
) -> Result<D3NameMetadata, anyhow::Error> {
    let call = IERC721Metadata::tokenURICall {
        tokenId: name.inner.id_bytes.into(),
    };
    let uri = reader
        .call_ccip_solidity_method(d3.native_token_contract, call)
        .await?
        .into_value()
        ._0;
    let metadata = reqwest::get(uri).await?.json::<D3NameMetadata>().await?;
    Ok(metadata)
}

impl D3NameMetadata {
    pub fn get_attribute(&self, trait_type: &str) -> Option<&D3NameAttribute> {
        self.attributes
            .iter()
            .find(|attr| attr.trait_type == trait_type)
    }

    pub fn get_value(&self, trait_type: &str) -> Option<&serde_json::Value> {
        self.get_attribute(trait_type).map(|attr| &attr.value)
    }

    // Parse expiration date from "Expiration Date" attribute as timestamp
    pub fn get_expiration_date(&self) -> Option<DateTime<Utc>> {
        self.get_value("Expiration Date").and_then(parse_timestamp)
    }
}

fn parse_timestamp(value: &serde_json::Value) -> Option<DateTime<Utc>> {
    match value {
        serde_json::Value::Number(num) => num
            .as_i64()
            .and_then(|timestamp| DateTime::from_timestamp(timestamp, 0)),
        serde_json::Value::String(s) => s
            .parse::<i64>()
            .ok()
            .and_then(|timestamp| DateTime::from_timestamp(timestamp, 0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn it_works() {
        let content = r#"
        {
            "attributes": [
                {
                    "display_type": "date",
                    "trait_type": "Expiration Date",
                    "value": 1761696000
                }
            ],
            "description": "Futureproof, Interoperable Digital Identities with D3.",
            "image": "https://cdn.d3.app/tokens/24894092657704973579132373869140463995882457799934966181909516953102602684545.png?hash=IMAQuhDJ9xXBtwG8AtDViyZQehj7BbGcaC6xk2V9sy4%3D",
            "name": "d3testbscout1*shib"
        }
        "#;
        let metadata: D3NameMetadata = serde_json::from_str(content).unwrap();
        let expected = DateTime::parse_from_rfc3339("2025-10-29T00:00:00Z")
            .unwrap()
            .to_utc();
        assert_eq!(metadata.get_expiration_date(), Some(expected));
    }
}
