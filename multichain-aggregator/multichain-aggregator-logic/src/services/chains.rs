use crate::{
    clients::{dapp, token_info},
    error::ServiceError,
    repository,
    types::{chains::Chain, ChainId},
};
use api_client_framework::HttpApiClient;
use blockscout_chains::BlockscoutChainsClient;
use cached::proc_macro::{cached, once};
use sea_orm::DatabaseConnection;
use std::collections::HashSet;

#[cached(
    key = "bool",
    convert = r#"{ with_active_api_keys }"#,
    time = 600, // 10 minutes
    result = true
)]
pub async fn list_repo_chains_cached(
    db: &DatabaseConnection,
    with_active_api_keys: bool,
) -> Result<Vec<Chain>, ServiceError> {
    let chains = repository::chains::list_chains(db, with_active_api_keys)
        .await?
        .into_iter()
        .map(|c| c.into())
        .collect();
    Ok(chains)
}

#[once(
    time = 600, // 10 minutes
    result = true
)]
async fn list_token_info_chains_cached(
    token_info_client: &HttpApiClient,
) -> Result<Vec<ChainId>, ServiceError> {
    let chain_ids = token_info_client
        .request(&token_info::list_chains::ListChains {})
        .await?
        .chains
        .into_iter()
        .filter_map(|id| ChainId::try_from(id).ok())
        .collect();

    Ok(chain_ids)
}

#[once(
    time = 600, // 10 minutes
    result = true
)]
async fn list_dapp_chains_cached(
    dapp_client: &HttpApiClient,
) -> Result<Vec<ChainId>, ServiceError> {
    let chain_ids = dapp_client
        .request(&dapp::list_chains::ListChains {})
        .await?
        .into_iter()
        .filter_map(|id| id.parse::<ChainId>().ok())
        .collect();

    Ok(chain_ids)
}

pub enum ChainSource<'a> {
    Repository,
    TokenInfo {
        token_info_client: &'a HttpApiClient,
    },
    Dapp {
        dapp_client: &'a HttpApiClient,
    },
}

pub async fn list_active_chains(
    db: &DatabaseConnection,
    sources: &[ChainSource<'_>],
) -> Result<Vec<Chain>, ServiceError> {
    let mut chain_ids = HashSet::new();

    for source in sources {
        match source {
            ChainSource::Repository => {
                let active_repo_chain_ids = list_repo_chains_cached(db, true)
                    .await?
                    .into_iter()
                    .map(|c| c.id);
                chain_ids.extend(active_repo_chain_ids);
            }
            ChainSource::TokenInfo { token_info_client } => {
                let token_info_chain_ids = list_token_info_chains_cached(token_info_client).await?;
                chain_ids.extend(token_info_chain_ids);
            }
            ChainSource::Dapp { dapp_client } => {
                let dapp_chain_ids = list_dapp_chains_cached(dapp_client).await?;
                chain_ids.extend(dapp_chain_ids);
            }
        }
    }

    let repo_chains = list_repo_chains_cached(db, false).await?;

    let items = repo_chains
        .into_iter()
        .filter(|c| chain_ids.contains(&c.id))
        .collect::<Vec<_>>();

    Ok(items)
}

pub async fn fetch_and_upsert_blockscout_chains(
    db: &DatabaseConnection,
) -> Result<(), ServiceError> {
    let blockscout_chains = BlockscoutChainsClient::builder()
        .with_max_retries(0)
        .build()
        .fetch_all()
        .await
        .map_err(|e| anyhow::anyhow!("failed to fetch blockscout chains: {:?}", e))?
        .into_iter()
        .filter_map(|(id, chain)| {
            let id = id.parse::<i64>().ok()?;
            Some((id, chain).into())
        })
        .collect::<Vec<_>>();
    repository::chains::upsert_many(db, blockscout_chains).await?;
    Ok(())
}
