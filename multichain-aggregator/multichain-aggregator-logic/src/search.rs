use crate::{
    clients::{
        dapp::{SearchDapps, SearchDappsParams},
        token_info::{endpoints::SearchTokenInfos, proto::SearchTokenInfosRequest, Client},
    },
    error::ServiceError,
    repository::{addresses, block_ranges, hashes},
    types::{
        chains::Chain,
        dapp::MarketplaceDapp,
        search_results::{ChainSearchResult, SearchResults},
        token_info::Token,
        ChainId,
    },
};
use api_client_framework::HttpApiClient;
use sea_orm::DatabaseConnection;
use std::collections::BTreeMap;
use tokio::join;
use tracing::instrument;

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
    token_info_client: &Client,
    query: String,
    chains: &[Chain],
) -> Result<SearchResults, ServiceError> {
    let raw_query = query.trim();

    let dapp_search_endpoint = SearchDapps {
        params: SearchDappsParams {
            query: raw_query.to_string(),
        },
    };

    let token_info_search_endpoint = SearchTokenInfos::new(SearchTokenInfosRequest {
        query: raw_query.to_string(),
        chain_id: None,
        page_size: Some(100),
        page_token: None,
    });

    let (hashes, block_numbers, addresses, dapps, token_infos) = join!(
        hashes::search_by_query(db, raw_query),
        block_ranges::search_by_query(db, raw_query),
        addresses::search_by_query(db, raw_query),
        dapp_client.request(&dapp_search_endpoint),
        token_info_client.request(&token_info_search_endpoint),
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

    match token_infos {
        Ok(token_infos) => {
            let tokens: Vec<Token> = token_infos
                .token_infos
                .into_iter()
                .filter_map(|t| t.try_into().ok())
                .collect();
            populate_search_results!(results, explorers, tokens, tokens);
        }
        Err(err) => {
            tracing::error!(error = ?err, "failed to search token infos");
        }
    }

    Ok(results)
}
