use super::ChainId;
use crate::{error::ParseError, proto};
use blockscout_chains::BlockscoutChainData;
use entity::chains::Model;

#[derive(Debug, Clone)]
pub struct Chain {
    pub id: ChainId,
    pub name: Option<String>,
    pub explorer_url: Option<String>,
    pub icon_url: Option<String>,
}

impl From<Chain> for Model {
    fn from(v: Chain) -> Self {
        Self {
            id: v.id,
            name: v.name,
            explorer_url: v.explorer_url,
            icon_url: v.icon_url,
            created_at: Default::default(),
            updated_at: Default::default(),
        }
    }
}

impl From<(ChainId, BlockscoutChainData)> for Chain {
    fn from((id, chain): (ChainId, BlockscoutChainData)) -> Self {
        Self {
            id,
            name: Some(chain.name),
            explorer_url: chain.explorers.first().map(|e| e.url.clone()),
            icon_url: Some(chain.logo),
        }
    }
}

impl From<Model> for Chain {
    fn from(v: Model) -> Self {
        Self {
            id: v.id,
            name: v.name,
            explorer_url: v.explorer_url,
            icon_url: v.icon_url,
        }
    }
}

impl TryFrom<Chain> for proto::Chain {
    type Error = ParseError;

    fn try_from(v: Chain) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id.to_string(),
            name: v
                .name
                .ok_or(ParseError::Custom("name is missing".to_string()))?,
            explorer_url: v
                .explorer_url
                .ok_or(ParseError::Custom("explorer_url is missing".to_string()))?,
            icon_url: v
                .icon_url
                .ok_or(ParseError::Custom("icon_url is missing".to_string()))?,
        })
    }
}
