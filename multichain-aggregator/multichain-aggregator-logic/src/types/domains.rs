use crate::{error::ParseError, proto};
use bens_proto::blockscout::bens::v1::Domain as BensDomain;

#[derive(Debug)]
pub struct Domain {
    pub address: alloy_primitives::Address,
    pub name: String,
    pub expiry_date: Option<String>,
    pub protocol: serde_json::Value,
}

impl TryFrom<BensDomain> for Domain {
    type Error = ParseError;

    fn try_from(domain: BensDomain) -> Result<Self, Self::Error> {
        let address = domain
            .resolved_address
            .ok_or_else(|| ParseError::Custom("resolved_address is missing".to_string()))?
            .hash
            .parse()
            .map_err(ParseError::from)?;

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
            address: v.address.to_string(),
            name: v.name,
            expiry_date: v.expiry_date,
            protocol: serde_json::from_value(v.protocol).expect("failed to deserialize protocol"),
        }
    }
}
