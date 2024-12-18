use crate::{
    dapp_client::DappClient,
    error::ServiceError,
    repository::{addresses, block_ranges, hashes},
    types::{
        chains::Chain,
        dapp::MarketplaceDapp,
        search_results::{ChainSearchResult, SearchResults},
        ChainId,
    },
};
use sea_orm::DatabaseConnection;
use std::collections::BTreeMap;
use tokio::join;

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

pub async fn quick_search(
    db: &DatabaseConnection,
    dapp_client: &DappClient,
    query: String,
    chains: &[Chain],
) -> Result<SearchResults, ServiceError> {
    let raw_query = query.trim();

    let (hashes, block_numbers, addresses, dapps) = join!(
        hashes::search_by_query(db, raw_query),
        block_ranges::search_by_query(db, raw_query),
        addresses::search_by_query(db, raw_query),
        dapp_client.search_dapps(raw_query),
    );

    let explorers: BTreeMap<ChainId, String> = chains
        .iter()
        .filter_map(|c| c.explorer_url.as_ref().map(|url| (c.id, url.clone())))
        .collect();

    let mut results = SearchResults::default();

    match hashes {
        Ok((blocks, transactions)) => {
            populate_search_results!(results, explorers, blocks, blocks);
            populate_search_results!(results, explorers, transactions, transactions);
        }
        Err(err) => {
            tracing::error!(error = ?err, "failed to search hashes");
        }
    }

    match block_numbers {
        Ok(block_numbers) => {
            populate_search_results!(results, explorers, block_numbers, block_numbers);
        }
        Err(err) => {
            tracing::error!(error = ?err, "failed to search block numbers");
        }
    }

    match addresses {
        Ok(addresses) => {
            populate_search_results!(results, explorers, addresses, addresses);
        }
        Err(err) => {
            tracing::error!(error = ?err, "failed to search addresses");
        }
    }

    match dapps {
        Ok(dapps) => {
            let dapps: Vec<MarketplaceDapp> = dapps
                .into_iter()
                .filter_map(|d| d.try_into().ok())
                .collect();
            populate_search_results!(results, explorers, dapps, dapps);
        }
        Err(err) => {
            tracing::error!(error = ?err, "failed to search dapps");
        }
    }

    Ok(results)
}
