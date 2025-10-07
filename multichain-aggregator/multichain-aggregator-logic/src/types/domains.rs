use crate::{error::ParseError, proto};
use bens_proto::blockscout::bens::v1 as bens_proto;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolInfo {
    pub id: String,
    pub short_name: String,
    pub title: String,
    pub description: String,
    pub deployment_blockscout_base_url: String,
    pub tld_list: Vec<String>,
    pub icon_url: Option<String>,
    pub docs_url: Option<String>,
}

impl From<bens_proto::ProtocolInfo> for ProtocolInfo {
    fn from(protocol: bens_proto::ProtocolInfo) -> Self {
        Self {
            id: protocol.id,
            short_name: protocol.short_name,
            title: protocol.title,
            description: protocol.description,
            deployment_blockscout_base_url: protocol.deployment_blockscout_base_url,
            tld_list: protocol.tld_list,
            icon_url: protocol.icon_url,
            docs_url: protocol.docs_url,
        }
    }
}

impl From<ProtocolInfo> for proto::ProtocolInfo {
    fn from(protocol: ProtocolInfo) -> Self {
        Self {
            id: protocol.id,
            short_name: protocol.short_name,
            title: protocol.title,
            description: protocol.description,
            deployment_blockscout_base_url: protocol.deployment_blockscout_base_url,
            tld_list: protocol.tld_list,
            icon_url: protocol.icon_url,
            docs_url: protocol.docs_url,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Domain {
    pub address: Option<alloy_primitives::Address>,
    pub name: String,
    pub expiry_date: Option<String>,
    pub protocol: ProtocolInfo,
}

impl TryFrom<bens_proto::Domain> for Domain {
    type Error = ParseError;

    fn try_from(domain: bens_proto::Domain) -> Result<Self, Self::Error> {
        let address = domain
            .resolved_address
            .map(|address| address.hash.parse().map_err(ParseError::from))
            .transpose()?;
        let protocol = domain
            .protocol
            .ok_or_else(|| ParseError::Custom("protocol is missing".to_string()))?
            .into();

        Ok(Self {
            name: domain.name,
            address,
            expiry_date: domain.expiry_date,
            protocol,
        })
    }
}

impl From<Domain> for proto::Domain {
    fn from(v: Domain) -> Self {
        // convert protocol to prost_wkt struct
        let protocol = serde_json::from_value(
            serde_json::to_value(v.protocol).expect("failed to serialize protocol"),
        )
        .expect("failed to deserialize protocol");
        Self {
            address: v.address.map(|a| a.to_checksum(None)),
            name: v.name,
            expiry_date: v.expiry_date,
            protocol,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainInfo {
    pub address: alloy_primitives::Address,
    pub name: String,
    pub expiry_date: Option<String>,
    pub protocol: ProtocolInfo,
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
            .ok_or_else(|| ParseError::Custom("protocol is missing".to_string()))?
            .into();

        Ok(Self {
            name: domain.name,
            address,
            expiry_date: domain.expiry_date,
            protocol,
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

#[derive(Debug, Clone)]
pub struct BasicDomainInfo {
    pub name: String,
    pub protocol: String,
}

impl From<DomainInfo> for BasicDomainInfo {
    fn from(v: DomainInfo) -> Self {
        Self {
            name: v.name,
            protocol: v.protocol.short_name,
        }
    }
}

impl From<BasicDomainInfo> for proto::BasicDomainInfo {
    fn from(v: BasicDomainInfo) -> Self {
        Self {
            name: v.name,
            protocol: v.protocol,
        }
    }
}
