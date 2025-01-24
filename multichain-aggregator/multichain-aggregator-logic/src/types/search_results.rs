use crate::{
    proto,
    types::{
        addresses::Address, block_ranges::ChainBlockNumber, dapp::MarketplaceDapp, hashes::Hash,
        token_info::Token, ChainId,
    },
};
use std::collections::BTreeMap;

#[derive(Default, Debug)]
pub struct ChainSearchResult {
    pub explorer_url: String,
    pub addresses: Vec<Address>,
    pub blocks: Vec<Hash>,
    pub transactions: Vec<Hash>,
    pub block_numbers: Vec<ChainBlockNumber>,
    pub dapps: Vec<MarketplaceDapp>,
    pub tokens: Vec<Token>,
}

impl ChainSearchResult {
    pub fn merge(&mut self, other: ChainSearchResult) {
        self.addresses.extend(other.addresses);
        self.blocks.extend(other.blocks);
        self.transactions.extend(other.transactions);
        self.block_numbers.extend(other.block_numbers);
        self.dapps.extend(other.dapps);
        self.tokens.extend(other.tokens);
    }
}

impl From<ChainSearchResult> for proto::quick_search_response::ChainSearchResult {
    fn from(v: ChainSearchResult) -> Self {
        Self {
            explorer_url: v.explorer_url,
            addresses: v.addresses.into_iter().map(|a| a.into()).collect(),
            blocks: v.blocks.into_iter().map(|b| b.into()).collect(),
            transactions: v.transactions.into_iter().map(|t| t.into()).collect(),
            block_numbers: v.block_numbers.into_iter().map(|b| b.into()).collect(),
            dapps: v.dapps.into_iter().map(|d| d.into()).collect(),
            tokens: v.tokens.into_iter().map(|t| t.into()).collect(),
        }
    }
}

#[derive(Default, Debug)]
pub struct SearchResults {
    pub items: BTreeMap<ChainId, ChainSearchResult>,
}

impl SearchResults {
    pub fn merge(&mut self, other: SearchResults) {
        for (chain_id, result) in other.items {
            self.items.entry(chain_id).or_default().merge(result);
        }
    }
}

impl From<SearchResults> for proto::QuickSearchResponse {
    fn from(v: SearchResults) -> Self {
        Self {
            items: v
                .items
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.into()))
                .collect(),
        }
    }
}
