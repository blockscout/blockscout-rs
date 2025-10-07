use crate::{
    error::ServiceError,
    repository::addresses,
    services::{MIN_QUERY_LENGTH, cluster::Cluster, macros::preload_domain_info},
    types::{ChainId, domains::Domain, hashes::HashType, search_results::QuickSearchResult},
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::instrument;

const QUICK_SEARCH_NUM_ITEMS: u64 = 50;
const QUICK_SEARCH_DEFAULT_ENTITY_LIMIT: usize = 5;

#[allow(clippy::too_many_arguments)]
#[instrument(skip_all, level = "info", fields(query = query))]
pub async fn quick_search(
    query: String,
    priority_chain_ids: &[ChainId],
    search_context: &SearchContext<'_>,
    unlimited_per_chain: bool,
) -> Result<QuickSearchResult, ServiceError> {
    let raw_query = query.trim();

    let terms = parse_search_terms(raw_query);

    // Each search term produces its own `SearchResults` struct.
    // E.g. `SearchTerm::Dapp` job populates only the `dapps` field of its result.
    // We need to merge all of them into a single `SearchResults` struct.
    let jobs = terms.into_iter().map(|t| t.search(search_context));

    let mut results = futures::future::join_all(jobs).await.into_iter().fold(
        QuickSearchResult::default(),
        |mut acc, r| {
            if let Ok(r) = r {
                acc.merge(r);
            }
            acc
        },
    );

    if !search_context.is_aggregated {
        results.flatten_aggregated_addresses();
    }

    if !unlimited_per_chain {
        results = results.filter_and_sort_entities_by_priority(priority_chain_ids);
    }

    let total_limit = QUICK_SEARCH_NUM_ITEMS as usize;
    let entity_limit = if unlimited_per_chain {
        total_limit
    } else {
        QUICK_SEARCH_DEFAULT_ENTITY_LIMIT
    };
    results.balance_entities(total_limit, entity_limit);

    Ok(results)
}

pub struct SearchContext<'a> {
    pub cluster: &'a Cluster,
    pub db: Arc<DatabaseConnection>,
    pub domain_primary_chain_id: ChainId,
    pub is_aggregated: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub enum SearchTerm {
    Hash(alloy_primitives::B256),
    AddressHash(alloy_primitives::Address),
    BlockNumber(alloy_primitives::BlockNumber),
    Dapp(String),
    TokenInfo(String),
    Nft(String),
    Domain(String),
}

impl SearchTerm {
    #[instrument(skip_all, level = "info", fields(term = ?self), err)]
    async fn search(
        self,
        search_context: &SearchContext<'_>,
    ) -> Result<QuickSearchResult, ServiceError> {
        let mut results = QuickSearchResult::default();

        let db = search_context.db.as_ref();

        let active_chain_ids = search_context.cluster.active_chain_ids().await?;
        let num_active_chains = active_chain_ids.len() as u64;

        match self {
            SearchTerm::Hash(hash) => {
                let (hashes, _) = search_context
                    .cluster
                    .search_hashes(hash.to_string(), None, vec![], num_active_chains, None)
                    .await?;

                let (blocks, transactions): (Vec<_>, Vec<_>) = hashes
                    .into_iter()
                    .partition(|h| h.hash_type == HashType::Block);

                results.blocks.extend(blocks);
                results.transactions.extend(transactions);
            }
            SearchTerm::AddressHash(address) => {
                let address = addresses::get_aggregated_address_info(
                    db,
                    address,
                    Some(active_chain_ids.clone()),
                )
                .await?;

                if let Some(mut address) = address {
                    let domain_info = search_context
                        .cluster
                        .get_domain_info_cached(*address.hash)
                        .await?;

                    if let Some(domain_info) = domain_info {
                        address.domain_info = Some(domain_info.clone());
                        results.domains.extend(vec![Domain::from(domain_info)]);
                    }
                    results.addresses.extend(vec![address]);
                };
            }
            SearchTerm::BlockNumber(block_number) => {
                let (block_numbers, _) = search_context
                    .cluster
                    .search_block_numbers(block_number.to_string(), vec![], num_active_chains, None)
                    .await?;

                results.block_numbers.extend(block_numbers);
            }
            SearchTerm::Dapp(query) => {
                let dapps = search_context
                    .cluster
                    .search_dapps(Some(query), vec![], None)
                    .await?;

                results.dapps.extend(dapps);
            }
            SearchTerm::TokenInfo(query) => {
                let (mut tokens, _) = search_context
                    .cluster
                    .search_token_infos_cached(
                        query,
                        active_chain_ids,
                        // TODO: temporary increase number of tokens to improve search quality
                        // until we have a dedicated endpoint for quick search which returns
                        // only one token per chain_id.
                        QUICK_SEARCH_NUM_ITEMS * 2,
                        None,
                    )
                    .await?;

                // clean up tokens with no name or symbol
                tokens.retain(|t| t.name.is_some() && t.symbol.is_some());

                results.tokens.extend(tokens);
            }
            SearchTerm::Nft(query) => {
                let (nfts, _) = search_context
                    .cluster
                    .search_nfts_cached(query, active_chain_ids, QUICK_SEARCH_NUM_ITEMS, None)
                    .await?;

                results.nfts.extend(nfts);
            }
            SearchTerm::Domain(query) => {
                let (domains, _) = search_context
                    .cluster
                    .search_domains_cached(
                        query,
                        vec![search_context.domain_primary_chain_id],
                        QUICK_SEARCH_NUM_ITEMS,
                        None,
                    )
                    .await?;

                let addresses = domains.iter().filter_map(|d| d.address).collect::<Vec<_>>();
                if !addresses.is_empty() {
                    let (mut addresses, _) = addresses::list_aggregated_address_infos(
                        db,
                        addresses,
                        Some(active_chain_ids.clone()),
                        None,
                        QUICK_SEARCH_NUM_ITEMS,
                        None,
                    )
                    .await?;

                    preload_domain_info!(search_context.cluster, addresses);

                    results.addresses.extend(addresses);
                }

                results.domains.extend(domains);
            }
        }

        Ok(results)
    }
}

pub fn parse_search_terms(query: &str) -> Vec<SearchTerm> {
    let query = query.trim();
    let mut terms = vec![];

    // If a term is an address or a hash, we can ignore other search types
    if let Ok(hash) = query.parse::<alloy_primitives::B256>() {
        terms.push(SearchTerm::Hash(hash));
        return terms;
    }
    if let Ok(address) = query.parse::<alloy_primitives::Address>() {
        terms.push(SearchTerm::TokenInfo(address.to_string()));
        terms.push(SearchTerm::AddressHash(address));
        return terms;
    }

    if let Ok(block_number) = query.parse::<alloy_primitives::BlockNumber>() {
        terms.push(SearchTerm::BlockNumber(block_number));
    }

    if query.len() >= MIN_QUERY_LENGTH {
        terms.push(SearchTerm::TokenInfo(query.to_string()));
        terms.push(SearchTerm::Nft(query.to_string()));
        terms.push(SearchTerm::Domain(query.to_string()));
    }

    terms.push(SearchTerm::Dapp(query.to_string()));

    terms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_search_terms() {
        assert_eq!(
            parse_search_terms("0x0000000000000000000000000000000000000000"),
            vec![
                SearchTerm::TokenInfo(alloy_primitives::Address::ZERO.to_string()),
                SearchTerm::AddressHash(alloy_primitives::Address::ZERO),
            ]
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
                SearchTerm::Nft("0x00".to_string()),
                SearchTerm::Domain("0x00".to_string()),
                SearchTerm::Dapp("0x00".to_string()),
            ]
        );

        assert_eq!(
            parse_search_terms("1234"),
            vec![
                SearchTerm::BlockNumber(1234),
                SearchTerm::TokenInfo("1234".to_string()),
                SearchTerm::Nft("1234".to_string()),
                SearchTerm::Domain("1234".to_string()),
                SearchTerm::Dapp("1234".to_string()),
            ]
        );

        assert_eq!(
            parse_search_terms("test.domain"),
            vec![
                SearchTerm::TokenInfo("test.domain".to_string()),
                SearchTerm::Nft("test.domain".to_string()),
                SearchTerm::Domain("test.domain".to_string()),
                SearchTerm::Dapp("test.domain".to_string()),
            ]
        );
    }
}
