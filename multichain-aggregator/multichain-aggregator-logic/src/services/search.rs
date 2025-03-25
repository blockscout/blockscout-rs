use crate::{
    clients::{dapp::search_dapps, token_info::search_token_infos},
    error::ServiceError,
    repository::{addresses, block_ranges, hashes},
    types::{
        addresses::{Address, TokenType},
        block_ranges::ChainBlockNumber,
        dapp::MarketplaceDapp,
        hashes::{Hash, HashType},
        search_results::QuickSearchResult,
        token_info::Token,
        ChainId,
    },
};
use alloy_primitives::Address as AddressAlloy;
use api_client_framework::HttpApiClient;
use sea_orm::DatabaseConnection;
use std::str::FromStr;
use tracing::instrument;

const MIN_QUERY_LENGTH: usize = 3;
const QUICK_SEARCH_NUM_ITEMS: u64 = 50;

pub async fn search_addresses(
    db: &DatabaseConnection,
    query: String,
    chain_id: Option<ChainId>,
    token_types: Option<Vec<TokenType>>,
    page_size: u64,
    page_token: Option<(AddressAlloy, ChainId)>,
) -> Result<(Vec<Address>, Option<(AddressAlloy, ChainId)>), ServiceError> {
    if query.len() < MIN_QUERY_LENGTH {
        return Ok((vec![], None));
    }

    let (address, query) = match alloy_primitives::Address::from_str(&query) {
        Ok(address) => (Some(address), None),
        Err(_) => (None, Some(query)),
    };

    let (addresses, page_token) = addresses::list(
        db,
        address,
        query,
        chain_id.map(|v| vec![v]),
        token_types,
        page_size,
        page_token,
    )
    .await?;

    Ok((
        addresses
            .into_iter()
            .map(Address::try_from)
            .collect::<Result<Vec<_>, _>>()?,
        page_token,
    ))
}

pub async fn search_hashes(
    db: &DatabaseConnection,
    query: String,
    hash_type: Option<HashType>,
    chain_ids: Option<Vec<ChainId>>,
    page_size: u64,
    page_token: Option<ChainId>,
) -> Result<(Vec<Hash>, Option<ChainId>), ServiceError> {
    let hash = match alloy_primitives::B256::from_str(&query) {
        Ok(hash) => hash,
        Err(_) => return Ok((vec![], None)),
    };

    let (hashes, page_token) =
        hashes::list(db, hash, hash_type, chain_ids, page_size, page_token).await?;

    Ok((
        hashes
            .into_iter()
            .map(Hash::try_from)
            .collect::<Result<Vec<_>, _>>()?,
        page_token,
    ))
}

pub async fn search_tokens(
    token_info_client: &HttpApiClient,
    query: String,
    chain_id: Vec<ChainId>,
    page_size: u64,
    page_token: Option<String>,
) -> Result<(Vec<Token>, Option<String>), ServiceError> {
    if query.len() < MIN_QUERY_LENGTH {
        return Ok((vec![], None));
    }

    let token_info_search_endpoint = search_token_infos::SearchTokenInfos {
        params: search_token_infos::SearchTokenInfosParams {
            query,
            chain_id,
            page_size: Some(page_size as u32),
            page_token,
        },
    };

    let res = token_info_client
        .request(&token_info_search_endpoint)
        .await
        .map_err(|err| anyhow::anyhow!("failed to search tokens: {:?}", err))?;

    let tokens = res
        .token_infos
        .into_iter()
        .map(Token::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    Ok((tokens, res.next_page_params.map(|p| p.page_token)))
}

pub async fn search_dapps(
    dapp_client: &HttpApiClient,
    query: Option<String>,
    categories: Option<String>,
    chain_ids: Vec<ChainId>,
) -> Result<Vec<MarketplaceDapp>, ServiceError> {
    let res = dapp_client
        .request(&search_dapps::SearchDapps {
            params: search_dapps::SearchDappsParams {
                title: query,
                categories,
                chain_ids,
            },
        })
        .await
        .map_err(|err| anyhow::anyhow!("failed to search dapps: {:?}", err))?;

    let dapps = res
        .into_iter()
        .filter_map(|d| d.try_into().ok())
        .collect::<Vec<_>>();

    Ok(dapps)
}

#[instrument(skip_all, level = "info", fields(query = query))]
pub async fn quick_search(
    db: &DatabaseConnection,
    dapp_client: &HttpApiClient,
    token_info_client: &HttpApiClient,
    query: String,
    chain_ids: &[ChainId],
) -> Result<QuickSearchResult, ServiceError> {
    let raw_query = query.trim();

    let terms = parse_search_terms(raw_query);
    let context = SearchContext {
        db,
        dapp_client,
        token_info_client,
        chain_ids,
    };

    // Each search term produces its own `SearchResults` struct.
    // E.g. `SearchTerm::Dapp` job populates only the `dapps` field of its result.
    // We need to merge all of them into a single `SearchResults` struct.
    let jobs = terms.into_iter().map(|t| t.search(&context));

    let mut results = futures::future::join_all(jobs).await.into_iter().fold(
        QuickSearchResult::default(),
        |mut acc, r| {
            if let Ok(r) = r {
                acc.merge(r);
            }
            acc
        },
    );

    results.balance_entities(QUICK_SEARCH_NUM_ITEMS as usize);

    Ok(results)
}

#[derive(Debug, Eq, PartialEq)]
pub enum SearchTerm {
    Hash(alloy_primitives::B256),
    AddressHash(alloy_primitives::Address),
    BlockNumber(alloy_primitives::BlockNumber),
    Dapp(String),
    TokenInfo(String),
    ContractName(String),
}

struct SearchContext<'a> {
    db: &'a DatabaseConnection,
    dapp_client: &'a HttpApiClient,
    token_info_client: &'a HttpApiClient,
    chain_ids: &'a [ChainId],
}

impl SearchTerm {
    #[instrument(skip_all, level = "info", fields(term = ?self), err)]
    async fn search(
        self,
        search_context: &SearchContext<'_>,
    ) -> Result<QuickSearchResult, ServiceError> {
        let mut results = QuickSearchResult::default();

        let db = search_context.db;

        match self {
            SearchTerm::Hash(hash) => {
                let (hashes, _) = hashes::list(
                    db,
                    hash,
                    None,
                    Some(search_context.chain_ids.to_vec()),
                    QUICK_SEARCH_NUM_ITEMS,
                    None,
                )
                .await?;
                let (blocks, transactions): (Vec<_>, Vec<_>) = hashes
                    .into_iter()
                    .map(Hash::try_from)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .partition(|h| h.hash_type == HashType::Block);

                results.blocks.extend(blocks);
                results.transactions.extend(transactions);
            }
            SearchTerm::AddressHash(address) => {
                let (addresses, _) = addresses::list(
                    db,
                    Some(address),
                    None,
                    Some(search_context.chain_ids.to_vec()),
                    None,
                    QUICK_SEARCH_NUM_ITEMS,
                    None,
                )
                .await?;
                let addresses: Vec<_> = addresses
                    .into_iter()
                    .map(Address::try_from)
                    .collect::<Result<Vec<_>, _>>()?;
                let nfts = addresses
                    .iter()
                    .filter(|a| {
                        matches!(
                            a.token_type,
                            Some(TokenType::Erc721) | Some(TokenType::Erc1155)
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>();

                results.addresses.extend(addresses);
                results.nfts.extend(nfts);
            }
            SearchTerm::BlockNumber(block_number) => {
                let (block_ranges, _) = block_ranges::list_matching_block_ranges_paginated(
                    db,
                    block_number,
                    Some(search_context.chain_ids.to_vec()),
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

                results.block_numbers.extend(block_numbers);
            }
            SearchTerm::Dapp(query) => {
                let dapps = search_dapps(
                    search_context.dapp_client,
                    Some(query),
                    None,
                    search_context.chain_ids.to_vec(),
                )
                .await?;

                results.dapps.extend(dapps);
            }
            SearchTerm::TokenInfo(query) => {
                let (tokens, _) = search_tokens(
                    search_context.token_info_client,
                    query,
                    search_context.chain_ids.to_vec(),
                    QUICK_SEARCH_NUM_ITEMS,
                    None,
                )
                .await?;

                results.tokens.extend(tokens);
            }
            SearchTerm::ContractName(query) => {
                let addresses = addresses::uniform_chain_search(
                    db,
                    query,
                    Some(vec![]),
                    search_context.chain_ids.to_vec(),
                )
                .await?
                .into_iter()
                .map(Address::try_from)
                .collect::<Result<Vec<_>, _>>()?;

                results.addresses.extend(addresses);
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
        terms.push(SearchTerm::ContractName(query.to_string()));
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
                SearchTerm::TokenInfo("0x00".to_string()),
                SearchTerm::ContractName("0x00".to_string()),
                SearchTerm::Dapp("0x00".to_string()),
            ]
        );

        assert_eq!(
            parse_search_terms("1234"),
            vec![
                SearchTerm::BlockNumber(1234),
                SearchTerm::TokenInfo("1234".to_string()),
                SearchTerm::ContractName("1234".to_string()),
                SearchTerm::Dapp("1234".to_string()),
            ]
        );
    }
}
