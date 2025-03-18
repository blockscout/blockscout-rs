use super::ChainId;
use crate::proto;
use entity::block_ranges::Model;

#[derive(Debug, Clone)]
pub struct BlockRange {
    pub chain_id: ChainId,
    pub min_block_number: u64,
    pub max_block_number: u64,
}

impl From<BlockRange> for Model {
    fn from(v: BlockRange) -> Self {
        Self {
            chain_id: v.chain_id,
            min_block_number: v.min_block_number as i32,
            max_block_number: v.max_block_number as i32,
            created_at: Default::default(),
            updated_at: Default::default(),
        }
    }
}

impl From<Model> for BlockRange {
    fn from(v: Model) -> Self {
        Self {
            chain_id: v.chain_id,
            min_block_number: v.min_block_number as u64,
            max_block_number: v.max_block_number as u64,
        }
    }
}

#[derive(Default, Debug)]
pub struct ChainBlockNumber {
    pub chain_id: ChainId,
    pub block_number: u64,
}

impl From<ChainBlockNumber> for proto::quick_search_response::ChainBlockNumber {
    fn from(v: ChainBlockNumber) -> Self {
        Self {
            chain_id: v.chain_id,
            block_number: v.block_number,
        }
    }
}
