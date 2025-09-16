use crate::{
    clients::dapp::search_dapps,
    error::ServiceError,
    services::{MIN_QUERY_LENGTH, chains},
    types::{ChainId, dapp::MarketplaceDapp},
};
use api_client_framework::HttpApiClient;

/// Search dapps without any cluster-related validation or preparation of chain ids.
pub async fn search_dapps(
    dapp_client: &HttpApiClient,
    query: Option<String>,
    categories: Option<String>,
    chain_ids: Vec<ChainId>,
    marketplace_enabled_cache: &chains::MarketplaceEnabledCache,
) -> Result<Vec<MarketplaceDapp>, ServiceError> {
    if let Some(query) = query.as_ref()
        && query.len() < MIN_QUERY_LENGTH
    {
        return Ok(vec![]);
    }

    let chain_ids = marketplace_enabled_cache
        .filter_marketplace_enabled_chains(chain_ids, |id| *id)
        .await;

    if chain_ids.is_empty() {
        return Ok(vec![]);
    }

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
