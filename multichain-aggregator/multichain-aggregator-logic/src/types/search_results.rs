use super::block_ranges::ChainBlockNumber;
use crate::{
    proto,
    types::{addresses::Address, hashes::Hash},
};
use std::collections::BTreeMap;

#[derive(Default, Debug)]
pub struct ChainSearchResult {
    pub explorer_url: String,
    pub addresses: Vec<Address>,
    pub blocks: Vec<Hash>,
    pub transactions: Vec<Hash>,
    pub block_numbers: Vec<ChainBlockNumber>,
}

impl From<ChainSearchResult> for proto::quick_search_response::ChainSearchResult {
    fn from(v: ChainSearchResult) -> Self {
        Self {
            explorer_url: v.explorer_url,
            addresses: v.addresses.into_iter().map(|a| a.into()).collect(),
            blocks: v.blocks.into_iter().map(|b| b.into()).collect(),
            transactions: v.transactions.into_iter().map(|t| t.into()).collect(),
            block_numbers: v.block_numbers.into_iter().map(|b| b.into()).collect(),
        }
    }
}

#[derive(Default, Debug)]
pub struct SearchResults {
    // chain_id -> ChainSearchResult
    pub items: BTreeMap<String, ChainSearchResult>,
}

impl From<SearchResults> for proto::QuickSearchResponse {
    fn from(v: SearchResults) -> Self {
        Self {
            items: v.items.into_iter().map(|(k, v)| (k, v.into())).collect(),
        }
    }
}
