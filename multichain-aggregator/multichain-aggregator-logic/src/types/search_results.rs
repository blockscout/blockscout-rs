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

    pub fn balance_entities(&mut self, n: usize) {
        macro_rules! balance_entities {
            ( $n:expr, $( $arg:expr => $ind:expr ),+ ) => {
                let lengths = [$( $arg.len() ),*];

                let result = evenly_take_elements(lengths, $n);

                $(
                    $arg.truncate(result[$ind]);
                )*
            };
        }

        balance_entities!(
            n,
            self.addresses => 0,
            self.blocks => 1,
            self.transactions => 2,
            self.block_numbers => 3,
            self.dapps => 4,
            self.tokens => 5,
            self.nfts => 6
        );
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
