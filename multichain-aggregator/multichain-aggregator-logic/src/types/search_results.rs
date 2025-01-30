use crate::{
    proto,
    types::{
        addresses::Address, block_ranges::ChainBlockNumber, dapp::MarketplaceDapp, hashes::Hash,
        token_info::Token,
    },
};

#[derive(Default, Debug)]
pub struct QuickSearchResult {
    pub addresses: Vec<Address>,
    pub blocks: Vec<Hash>,
    pub transactions: Vec<Hash>,
    pub block_numbers: Vec<ChainBlockNumber>,
    pub dapps: Vec<MarketplaceDapp>,
    pub tokens: Vec<Token>,
    pub nfts: Vec<Address>,
}

impl QuickSearchResult {
    pub fn merge(&mut self, other: QuickSearchResult) {
        self.addresses.extend(other.addresses);
        self.blocks.extend(other.blocks);
        self.transactions.extend(other.transactions);
        self.block_numbers.extend(other.block_numbers);
        self.dapps.extend(other.dapps);
        self.tokens.extend(other.tokens);
        self.nfts.extend(other.nfts);
    }
}

impl From<QuickSearchResult> for proto::QuickSearchResponse {
    fn from(v: QuickSearchResult) -> Self {
        Self {
            addresses: v.addresses.into_iter().map(|a| a.into()).collect(),
            blocks: v.blocks.into_iter().map(|b| b.into()).collect(),
            transactions: v.transactions.into_iter().map(|t| t.into()).collect(),
            block_numbers: v.block_numbers.into_iter().map(|b| b.into()).collect(),
            dapps: v.dapps.into_iter().map(|d| d.into()).collect(),
            tokens: v.tokens.into_iter().map(|t| t.into()).collect(),
            nfts: v.nfts.into_iter().map(|n| n.into()).collect(),
        }
    }
}
