use super::ChainId;
use blockscout_chains::BlockscoutChainData;
use entity::chains::Model;

#[derive(Debug, Clone)]
pub struct Chain {
    pub id: ChainId,
    pub explorer_url: Option<String>,
    pub icon_url: Option<String>,
}

impl From<Chain> for Model {
    fn from(v: Chain) -> Self {
        Self {
            id: v.id,
            explorer_url: v.explorer_url,
            icon_url: v.icon_url,
            created_at: Default::default(),
            updated_at: Default::default(),
        }
    }
}

impl From<(i64, BlockscoutChainData)> for Chain {
    fn from((id, chain): (i64, BlockscoutChainData)) -> Self {
        Self {
            id,
            explorer_url: chain.explorers.first().map(|e| e.url.clone()),
            icon_url: Some(chain.logo),
        }
    }
}
