use crate::{
    clients::{
        dapp::{SearchDapps, SearchDappsParams},
        token_info::{SearchTokenInfos, SearchTokenInfosParams},
    },
    error::ServiceError,
    repository::{addresses, block_ranges, hashes},
    types::{
        addresses::Address,
        block_ranges::ChainBlockNumber,
        chains::Chain,
        dapp::MarketplaceDapp,
        hashes::Hash,
        search_results::{ChainSearchResult, SearchResults},
        token_info::Token,
        ChainId,
    },
};
use api_client_framework::HttpApiClient;
use entity::sea_orm_active_enums as db_enum;
use sea_orm::DatabaseConnection;
use std::collections::BTreeMap;
use tracing::instrument;

const MIN_QUERY_LENGTH: usize = 3;
const QUICK_SEARCH_NUM_ITEMS: u64 = 10;

macro_rules! populate_search_results {
    ($target:expr, $explorers:expr, $from:expr, $field:ident) => {
        for e in $from {
            if let Some(explorer_url) = $explorers.get(&e.chain_id).cloned() {
                let entry = $target
                    .items
                    .entry(e.chain_id)
                    .or_insert_with(ChainSearchResult::default);
                entry.$field.push(e);
                entry.explorer_url = explorer_url;
            }
        }
    };
}

#[instrument(skip_all, level = "info", fields(query = query))]
pub async fn quick_search(
    db: &DatabaseConnection,
    dapp_client: &HttpApiClient,
    token_info_client: &HttpApiClient,
    query: String,
    chains: &[Chain],
) -> Result<SearchResults, ServiceError> {
    let raw_query = query.trim();

    let explorers = &chains
        .iter()
        .filter_map(|c| c.explorer_url.as_ref().map(|url| (c.id, url.clone())))
        .collect::<BTreeMap<ChainId, String>>();

    let terms = parse_search_terms(raw_query);
    let context = SearchContext {
        db,
        dapp_client,
        token_info_client,
        explorers,
    };

    // Each search term produces its own `SearchResults` struct.
    // E.g. `SearchTerm::Dapp` job populates only the `dapps` field of its result.
    // We need to merge all of them into a single `SearchResults` struct.
    let jobs = terms.into_iter().map(|t| t.search(&context));

    let results = futures::future::join_all(jobs).await.into_iter().fold(
        SearchResults::default(),
        |mut acc, r| {
            if let Ok(r) = r {
                acc.merge(r);
            }
            acc
        },
    );

    Ok(results)
}

#[derive(Debug, Eq, PartialEq)]
pub enum SearchTerm {
    Hash(alloy_primitives::B256),
    AddressHash(alloy_primitives::Address),
    BlockNumber(alloy_primitives::BlockNumber),
    Dapp(String),
    TokenInfo(String),
}

struct SearchContext<'a> {
    db: &'a DatabaseConnection,
    dapp_client: &'a HttpApiClient,
    token_info_client: &'a HttpApiClient,
    explorers: &'a BTreeMap<ChainId, String>,
}

impl SearchTerm {
    #[instrument(skip_all, level = "info", fields(term = ?self), err)]
    async fn search(
        self,
        search_context: &SearchContext<'_>,
    ) -> Result<SearchResults, ServiceError> {
        let mut results = SearchResults::default();

        let db = search_context.db;
        let explorers = search_context.explorers;

        match self {
            SearchTerm::Hash(hash) => {
                let (hashes, _) = hashes::list_hashes_paginated(
                    db,
                    hash,
                    None,
                    None,
                    QUICK_SEARCH_NUM_ITEMS,
                    None,
                )
                .await?;
                let (blocks, transactions): (Vec<_>, Vec<_>) = hashes
                    .into_iter()
                    .map(Hash::try_from)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .partition(|h| h.hash_type == db_enum::HashType::Block);

                populate_search_results!(results, explorers, blocks, blocks);
                populate_search_results!(results, explorers, transactions, transactions);
            }
            SearchTerm::AddressHash(address) => {
                let (addresses, _) = addresses::list_addresses_paginated(
                    db,
                    Some(address),
                    None,
                    None,
                    QUICK_SEARCH_NUM_ITEMS,
                    None,
                )
                .await?;
                let addresses: Vec<_> = addresses
                    .into_iter()
                    .map(Address::try_from)
                    .collect::<Result<Vec<_>, _>>()?;

                populate_search_results!(results, explorers, addresses, addresses)
            }
            SearchTerm::BlockNumber(block_number) => {
                let (block_ranges, _) = block_ranges::list_matching_block_ranges_paginated(
                    db,
                    block_number,
                    QUICK_SEARCH_NUM_ITEMS,
                    None,
                )
                .await?;
                let block_numbers: Vec<_> = block_ranges
                    .into_iter()
                    .map(|r| ChainBlockNumber {
                        chain_id: r.chain_id,
                        block_number,
                    })
                    .collect::<Vec<_>>();

                populate_search_results!(results, explorers, block_numbers, block_numbers);
            }
            SearchTerm::Dapp(query) => {
                let dapps: Vec<MarketplaceDapp> = search_context
                    .dapp_client
                    .request(&SearchDapps {
                        params: SearchDappsParams { query },
                    })
                    .await
                    .map_err(|err| ServiceError::Internal(err.into()))?
                    .into_iter()
                    .filter_map(|d| d.try_into().ok())
                    .collect();
                populate_search_results!(results, explorers, dapps, dapps);
            }
            SearchTerm::TokenInfo(query) => {
                let tokens: Vec<Token> = search_context
                    .token_info_client
                    .request(&SearchTokenInfos {
                        params: SearchTokenInfosParams {
                            query,
                            chain_id: None,
                            page_size: Some(QUICK_SEARCH_NUM_ITEMS as u32),
                            page_token: None,
                        },
                    })
                    .await
                    .map_err(|err| ServiceError::Internal(err.into()))?
                    .token_infos
                    .into_iter()
                    .filter_map(|t| t.try_into().ok())
                    .collect();
                populate_search_results!(results, explorers, tokens, tokens);
            }
        }

        Ok(results)
    }
}

pub fn parse_search_terms(query: &str) -> Vec<SearchTerm> {
    let mut terms = vec![];

    // If a term is an address or a hash, we can ignore other search types
    if let Ok(hash) = query.parse::<alloy_primitives::B256>() {
        terms.push(SearchTerm::Hash(hash));
        return terms;
    }
    if let Ok(address) = query.parse::<alloy_primitives::Address>() {
        terms.push(SearchTerm::AddressHash(address));
        return terms;
    }

    if let Ok(block_number) = query.parse::<alloy_primitives::BlockNumber>() {
        terms.push(SearchTerm::BlockNumber(block_number));
    }

    if query.len() >= MIN_QUERY_LENGTH {
        terms.push(SearchTerm::TokenInfo(query.to_string()));
    }

    terms.push(SearchTerm::Dapp(query.to_string()));

    terms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_search_terms_works() {
        assert_eq!(
            parse_search_terms("0x0000000000000000000000000000000000000000"),
            vec![SearchTerm::AddressHash(alloy_primitives::Address::ZERO)]
        );
        assert_eq!(
            parse_search_terms(
                "0x0000000000000000000000000000000000000000000000000000000000000000"
            ),
            vec![SearchTerm::Hash(alloy_primitives::B256::ZERO)]
        );

        assert_eq!(
            parse_search_terms("0x00"),
            vec![
                SearchTerm::Dapp("0x00".to_string()),
                SearchTerm::TokenInfo("0x00".to_string())
            ]
        );

        assert_eq!(
            parse_search_terms("1234"),
            vec![
                SearchTerm::BlockNumber(1234),
                SearchTerm::Dapp("1234".to_string()),
                SearchTerm::TokenInfo("1234".to_string())
            ]
        );
    }
}
