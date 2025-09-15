use crate::{
    error::ServiceError,
    repository::addresses,
    services::{MIN_QUERY_LENGTH, chains, cluster::Cluster},
    types::{ChainId, domains::Domain, hashes::HashType, search_results::QuickSearchResult},
};
use api_client_framework::HttpApiClient;
use recache::{handler::CacheHandler, stores::redis::RedisStore};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::instrument;

const QUICK_SEARCH_NUM_ITEMS: u64 = 50;
const QUICK_SEARCH_ENTITY_LIMIT: usize = 5;

#[allow(clippy::too_many_arguments)]
#[instrument(skip_all, level = "info", fields(query = query))]
pub async fn quick_search(
    query: String,
    priority_chain_ids: &[ChainId],
    search_context: &SearchContext<'_>,
) -> Result<QuickSearchResult, ServiceError> {
    let raw_query = query.trim();

    let terms = parse_search_terms(raw_query);

    // Each search term produces its own `SearchResults` struct.
    // E.g. `SearchTerm::Dapp` job populates only the `dapps` field of its result.
    // We need to merge all of them into a single `SearchResults` struct.
    let jobs = terms.into_iter().map(|t| t.search(search_context));

    let mut results = futures::future::join_all(jobs)
        .await
        .into_iter()
        .fold(QuickSearchResult::default(), |mut acc, r| {
            if let Ok(r) = r {
                acc.merge(r);
            }
            acc
        })
        .filter_and_sort_entities_by_priority(priority_chain_ids);

    results.balance_entities(QUICK_SEARCH_NUM_ITEMS as usize, QUICK_SEARCH_ENTITY_LIMIT);

    Ok(results)
}

pub type DomainSearchCache = CacheHandler<RedisStore, String, (Vec<Domain>, Option<String>)>;

pub struct SearchContext<'a> {
    pub cluster: &'a Cluster,
    pub db: Arc<DatabaseConnection>,
    pub dapp_client: &'a HttpApiClient,
    pub token_info_client: &'a HttpApiClient,
    pub bens_client: &'a HttpApiClient,
    pub bens_protocols: Option<&'static [String]>,
    pub domain_primary_chain_id: ChainId,
    pub marketplace_enabled_cache: &'a chains::MarketplaceEnabledCache,
    pub domain_search_cache: Option<&'a DomainSearchCache>,
    pub is_aggregated: bool,
}

impl<'a> SearchContext<'a> {
    pub async fn num_active_chains(&self) -> Result<u64, ServiceError> {
        Ok(chains::list_repo_chains_cached(self.db.as_ref(), true)
            .await?
            .len() as u64)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum SearchTerm {
    Hash(alloy_primitives::B256),
    AddressHash(alloy_primitives::Address),
    BlockNumber(alloy_primitives::BlockNumber),
    Dapp(String),
    TokenInfo(String),
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
                let (addresses, _) = search_context
                    .cluster
                    .search_addresses(
                        address.to_string(),
                        vec![],
                        search_context.is_aggregated,
                        num_active_chains,
                        None,
                    )
                    .await?;

                let domains = addresses
                    .iter()
                    .filter_map(|a| a.domain_info.clone())
                    .map(Domain::from)
                    .collect::<Vec<_>>();

                results.domains.extend(domains);
                results.addresses.extend(addresses);
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
                let (tokens, _) = search_context
                    .cluster
                    .search_tokens(
                        query,
                        active_chain_ids,
                        // TODO: temporary increase number of tokens to improve search quality
                        // until we have a dedicated endpoint for quick search which returns
                        // only one token per chain_id.
                        QUICK_SEARCH_NUM_ITEMS * 2,
                        None,
                    )
                    .await?;

                results.tokens.extend(tokens);
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
                    let (mut addresses, _) = addresses::list_address_infos(
                        db,
                        addresses,
                        Some(active_chain_ids.clone()),
                        QUICK_SEARCH_NUM_ITEMS,
                        None,
                    )
                    .await?;

                    let domain_infos = search_context
                        .cluster
                        .get_domain_info(addresses.iter().map(|a| *a.hash))
                        .await;

                    addresses
                        .iter_mut()
                        .for_each(|a| a.domain_info = domain_infos.get(&*a.hash).cloned());

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
                SearchTerm::Domain("0x00".to_string()),
                SearchTerm::Dapp("0x00".to_string()),
            ]
        );

        assert_eq!(
            parse_search_terms("1234"),
            vec![
                SearchTerm::BlockNumber(1234),
                SearchTerm::TokenInfo("1234".to_string()),
                SearchTerm::Domain("1234".to_string()),
                SearchTerm::Dapp("1234".to_string()),
            ]
        );

        assert_eq!(
            parse_search_terms("test.domain"),
            vec![
                SearchTerm::TokenInfo("test.domain".to_string()),
                SearchTerm::Domain("test.domain".to_string()),
                SearchTerm::Dapp("test.domain".to_string()),
            ]
        );
    }
}
