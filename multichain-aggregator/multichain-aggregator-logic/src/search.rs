use crate::{
    error::ServiceError,
    repository::{addresses, block_ranges, hashes},
    types::{
        chains::Chain,
        search_results::{ChainSearchResult, SearchResults},
    },
};
use sea_orm::DatabaseConnection;
use std::collections::BTreeMap;
use tokio::try_join;

macro_rules! populate_search_results {
    ($target:expr, $explorers:expr, $from:expr, $field:ident) => {
        for e in $from {
            let chain_id = e.chain_id.to_string();
            if let Some(explorer_url) = $explorers.get(&chain_id).cloned() {
                let entry = $target
                    .items
                    .entry(chain_id.clone())
                    .or_insert_with(ChainSearchResult::default);
                entry.$field.push(e);
                entry.explorer_url = explorer_url;
            }
        }
    };
}

pub async fn quick_search(
    db: &DatabaseConnection,
    query: String,
    chains: &[Chain],
) -> Result<SearchResults, ServiceError> {
    let raw_query = query.trim();

    let ((blocks, transactions), block_numbers, addresses) = try_join!(
        hashes::search_by_query(db, raw_query),
        block_ranges::search_by_query(db, raw_query),
        addresses::search_by_query(db, raw_query)
    )?;

    let explorers: BTreeMap<String, String> = chains
        .iter()
        .filter_map(|c| {
            c.explorer_url
                .as_ref()
                .map(|url| (c.id.to_string(), url.clone()))
        })
        .collect();

    let mut results = SearchResults::default();
    populate_search_results!(results, explorers, addresses, addresses);
    populate_search_results!(results, explorers, blocks, blocks);
    populate_search_results!(results, explorers, transactions, transactions);
    populate_search_results!(results, explorers, block_numbers, block_numbers);

    Ok(results)
}
