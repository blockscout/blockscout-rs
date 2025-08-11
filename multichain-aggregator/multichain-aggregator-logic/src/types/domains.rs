use crate::{error::ParseError, proto};
use bens_proto::blockscout::bens::v1 as bens_proto;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Domain {
    pub address: Option<alloy_primitives::Address>,
    pub name: String,
    pub expiry_date: Option<String>,
    pub protocol: serde_json::Value,
}

impl TryFrom<bens_proto::Domain> for Domain {
    type Error = ParseError;

    fn try_from(domain: bens_proto::Domain) -> Result<Self, Self::Error> {
        let address = domain
            .resolved_address
            .map(|address| address.hash.parse().map_err(ParseError::from))
            .transpose()?;

        Ok(Self {
            name: domain.name,
            address,
            expiry_date: domain.expiry_date,
            protocol: serde_json::to_value(
                domain
                    .protocol
                    .ok_or_else(|| ParseError::Custom("protocol is missing".to_string()))?,
            )
            .map_err(ParseError::from)?,
        })
    }
}

impl From<Domain> for proto::Domain {
    fn from(v: Domain) -> Self {
        Self {
            address: v.address.map(|a| a.to_checksum(None)),
            name: v.name,
            expiry_date: v.expiry_date,
            protocol: serde_json::from_value(v.protocol).expect("failed to deserialize protocol"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainInfo {
    pub address: alloy_primitives::Address,
    pub name: String,
    pub expiry_date: Option<String>,
    pub protocol: serde_json::Value,
    pub names_count: u32,
}

impl TryFrom<bens_proto::GetAddressResponse> for DomainInfo {
    type Error = ParseError;

    fn try_from(res: bens_proto::GetAddressResponse) -> Result<Self, Self::Error> {
        let domain = res
            .domain
            .ok_or_else(|| ParseError::Custom("domain is missing".to_string()))?;
        let address = domain
            .resolved_address
            .map(|address| address.hash.parse().map_err(ParseError::from))
            .ok_or_else(|| ParseError::Custom("address is missing".to_string()))??;
        let protocol = domain
            .protocol
            .ok_or_else(|| ParseError::Custom("protocol is missing".to_string()))?;

        Ok(Self {
            name: domain.name,
            address,
            expiry_date: domain.expiry_date,
            protocol: serde_json::to_value(protocol).map_err(ParseError::from)?,
            names_count: res.resolved_domains_count as u32,
        })
    }
}

impl From<DomainInfo> for proto::DomainInfo {
    fn from(v: DomainInfo) -> Self {
        Self {
            address: v.address.to_checksum(None),
            name: v.name,
            expiry_date: v.expiry_date,
            names_count: v.names_count,
        }
    }
}

impl From<DomainInfo> for Domain {
    fn from(v: DomainInfo) -> Self {
        Self {
            address: Some(v.address),
            name: v.name,
            expiry_date: v.expiry_date,
            protocol: v.protocol,
        }
    }
}
