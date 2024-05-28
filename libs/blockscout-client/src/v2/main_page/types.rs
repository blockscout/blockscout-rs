use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct IndexingStatus {
    pub finished_indexing: bool,
    pub finished_indexing_blocks: bool,
    pub indexed_blocks_ratio: String,
    pub indexed_internal_transactions_ratio: String,
}
