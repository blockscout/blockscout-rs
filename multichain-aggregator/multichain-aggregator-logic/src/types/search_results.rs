use super::ChainId;
use crate::{
    error::ParseError,
    types::{
        addresses::{Address, AggregatedAddressInfo},
        block_ranges::ChainBlockNumber,
        dapp::MarketplaceDapp,
        domains::Domain,
        hashes::Hash,
        tokens::AggregatedToken,
    },
};
use multichain_aggregator_proto::blockscout::{
    cluster_explorer::v1 as cluster_proto, multichain_aggregator::v1 as multichain_proto,
};
use std::collections::{HashMap, HashSet};

#[derive(Default, Debug)]
pub struct QuickSearchResult {
    pub addresses: Vec<AggregatedAddressInfo>,
    pub blocks: Vec<Hash>,
    pub transactions: Vec<Hash>,
    pub block_numbers: Vec<ChainBlockNumber>,
    pub dapps: Vec<MarketplaceDapp>,
    pub tokens: Vec<AggregatedToken>,
    pub nfts: Vec<Address>,
    pub domains: Vec<Domain>,
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
        self.domains.extend(other.domains);
    }

    pub fn filter_and_sort_entities_by_priority(mut self, priority_chain_ids: &[ChainId]) -> Self {
        macro_rules! filter_and_sort_by_priority {
            ($search_result: ident, [$(($field: ident, $get_chain_id: expr)),*]) => {
                $(
                    $search_result.$field = Self::filter_and_sort_array_by_priority($search_result.$field, |e| $get_chain_id(e), priority_chain_ids);
                )*
            };
        }

        filter_and_sort_by_priority!(
            self,
            [
                (addresses, |e: &AggregatedAddressInfo| e
                    .chain_infos
                    .iter()
                    .max_by_key(|c| &c.coin_balance)
                    .map(|c| c.chain_id)
                    .unwrap_or_default()),
                (blocks, |e: &Hash| e.chain_id),
                (transactions, |e: &Hash| e.chain_id),
                (block_numbers, |e: &ChainBlockNumber| e.chain_id),
                (dapps, |e: &MarketplaceDapp| e.chain_id),
                (tokens, |e: &AggregatedToken| e.chain_id),
                (nfts, |e: &Address| e.chain_id)
            ]
        );

        self
    }

    fn filter_and_sort_array_by_priority<T>(
        items: impl IntoIterator<Item = T>,
        get_chain_id: impl Fn(&T) -> ChainId,
        priority_chain_ids: &[ChainId],
    ) -> Vec<T> {
        // Filter to keep only one item per chain_id,
        // assuming they are already presented in a relevant order.
        let mut seen_chain_ids = HashSet::new();
        let mut filtered_items = items
            .into_iter()
            .filter(|item| {
                let chain_id = get_chain_id(item);
                seen_chain_ids.insert(chain_id)
            })
            .collect::<Vec<_>>();

        let chain_id_priority = priority_chain_ids
            .iter()
            .enumerate()
            .map(|(idx, &chain_id)| (chain_id, idx))
            .collect::<HashMap<_, _>>();
        let num_priority_chains = priority_chain_ids.len();
        let get_chain_id_score = |chain_id: ChainId| {
            chain_id_priority
                .get(&chain_id)
                .copied()
                .unwrap_or(num_priority_chains + chain_id as usize)
        };
        filtered_items.sort_by_key(|item| {
            let chain_id = get_chain_id(item);
            get_chain_id_score(chain_id)
        });

        filtered_items
    }

    pub fn flatten_aggregated_addresses(&mut self) {
        self.addresses = self
            .addresses
            .iter()
            .flat_map(|address| {
                address
                    .chain_infos
                    .iter()
                    .map(|c| AggregatedAddressInfo {
                        hash: address.hash.clone(),
                        chain_infos: vec![c.clone()],
                        has_tokens: address.has_tokens,
                        has_interop_message_transfers: address.has_interop_message_transfers,
                        domain_info: address.domain_info.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
    }

    pub fn balance_entities(&mut self, total_limit: usize, entity_limit: usize) {
        macro_rules! balance_entities {
            ( $n:expr, $( $arg:expr => $ind:expr ),+ ) => {
                let lengths = [$( $arg.len() ),*];

                let result = evenly_take_elements(lengths, total_limit);

                $(
                    $arg.truncate(result[$ind].min(entity_limit));
                )*
            };
        }

        balance_entities!(
            total_limit,
            self.addresses => 0,
            self.blocks => 1,
            self.transactions => 2,
            self.block_numbers => 3,
            self.dapps => 4,
            self.tokens => 5,
            self.nfts => 6,
            self.domains => 7
        );
    }
}

impl TryFrom<QuickSearchResult> for multichain_proto::QuickSearchResponse {
    type Error = ParseError;

    fn try_from(v: QuickSearchResult) -> Result<Self, Self::Error> {
        Ok(Self {
            addresses: v
                .addresses
                .into_iter()
                .map(|a| a.try_into())
                .collect::<Result<Vec<_>, _>>()?,
            blocks: v.blocks.into_iter().map(|b| b.into()).collect(),
            transactions: v.transactions.into_iter().map(|t| t.into()).collect(),
            block_numbers: v.block_numbers.into_iter().map(|b| b.into()).collect(),
            dapps: v.dapps.into_iter().map(|d| d.into()).collect(),
            tokens: v.tokens.into_iter().map(|t| t.into()).collect(),
            nfts: v.nfts.into_iter().map(|n| n.into()).collect(),
            domains: v.domains.into_iter().map(|d| d.into()).collect(),
        })
    }
}

impl TryFrom<QuickSearchResult> for cluster_proto::ClusterQuickSearchResponse {
    type Error = ParseError;

    fn try_from(v: QuickSearchResult) -> Result<Self, Self::Error> {
        Ok(Self {
            addresses: v.addresses.into_iter().map(|a| a.into()).collect(),
            blocks: v.blocks.into_iter().map(|b| b.into()).collect(),
            transactions: v.transactions.into_iter().map(|t| t.into()).collect(),
            block_numbers: v.block_numbers.into_iter().map(|b| b.into()).collect(),
            dapps: v.dapps.into_iter().map(|d| d.into()).collect(),
            tokens: v.tokens.into_iter().map(|t| t.into()).collect(),
            nfts: v.nfts.into_iter().map(|n| n.into()).collect(),
            domains: v.domains.into_iter().map(|d| d.into()).collect(),
        })
    }
}

fn evenly_take_elements<const N: usize>(
    mut lengths: [usize; N],
    mut remained: usize,
) -> [usize; N] {
    let mut taken_lengths = [0; N];

    while remained > 0 {
        let non_zero_count = lengths.iter().filter(|&&x| x > 0).count();
        // No more elements to take
        if non_zero_count == 0 {
            break;
        }

        let target = if remained < non_zero_count {
            1
        } else {
            remained / non_zero_count
        };

        let mut sum_taken = 0;

        for (len, taken_len) in lengths.iter_mut().zip(&mut taken_lengths) {
            if sum_taken >= remained {
                break;
            }

            let take = target.min(*len);
            *len -= take;
            *taken_len += take;
            sum_taken += take;
        }

        remained = remained.saturating_sub(sum_taken);
    }

    taken_lengths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evenly_take_elements() {
        assert_eq!(evenly_take_elements([10, 20, 30], 2), [1, 1, 0]);
        assert_eq!(evenly_take_elements([10, 20, 30], 10), [4, 3, 3]);
        assert_eq!(evenly_take_elements([30, 20, 10], 50), [20, 20, 10]);
        assert_eq!(evenly_take_elements([8, 9, 5], 100), [8, 9, 5]);
        assert_eq!(evenly_take_elements([3, 2, 1], 0), [0, 0, 0]);
    }
}
