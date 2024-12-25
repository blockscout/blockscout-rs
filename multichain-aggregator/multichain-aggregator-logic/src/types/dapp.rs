use super::ChainId;
use crate::{clients::dapp::DappWithChainId, error::ParseError, proto};

#[derive(Debug)]
pub struct MarketplaceDapp {
    pub id: String,
    pub title: String,
    pub logo: String,
    pub short_description: String,
    pub chain_id: ChainId,
}

impl TryFrom<DappWithChainId> for MarketplaceDapp {
    type Error = ParseError;

    fn try_from(v: DappWithChainId) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.dapp.id,
            title: v.dapp.title,
            logo: v.dapp.logo,
            short_description: v.dapp.short_description,
            chain_id: v.chain_id.parse()?,
        })
    }
}

impl From<MarketplaceDapp> for proto::MarketplaceDapp {
    fn from(v: MarketplaceDapp) -> Self {
        Self {
            id: v.id,
            title: v.title,
            logo: v.logo,
            short_description: v.short_description,
            chain_id: v.chain_id.to_string(),
        }
    }
}
