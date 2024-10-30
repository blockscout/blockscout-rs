use super::ChainId;
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
