use crate::{
    clients::blockscout, error::ServiceError, services::cluster::BlockscoutClients, types::ChainId,
};
use recache::{handler::CacheHandler, stores::redis::RedisStore};
use std::{sync::Arc, time::Duration};

pub type CoinPriceCache = CacheHandler<RedisStore, String, Option<String>>;

/// Default coin price cache with 1 min ttl
pub fn build_coin_price_cache(redis_cache: Arc<RedisStore>) -> CoinPriceCache {
    CoinPriceCache::builder(redis_cache)
        .default_ttl(Duration::from_secs(60))
        .maybe_default_refresh_ahead(Some(Duration::from_secs(12)))
        .build()
}

/// Try to fetch coin price from each client until we get a result.
/// All provided chains must share the same native coin.
pub async fn try_fetch_coin_price(
    blockscout_clients: BlockscoutClients,
    chain_ids: Vec<ChainId>,
) -> Option<String> {
    for chain_id in chain_ids {
        if let Ok(coin_price) = try_fetch_client_coin_price(&blockscout_clients, chain_id).await {
            return Some(coin_price);
        }
    }
    None
}

async fn try_fetch_client_coin_price(
    blockscout_clients: &BlockscoutClients,
    chain_id: ChainId,
) -> Result<String, ServiceError> {
    let blockscout_client = blockscout_clients.get(&chain_id).ok_or_else(|| {
        ServiceError::Internal(anyhow::anyhow!(
            "blockscout client for chain id {chain_id} not found",
        ))
    })?;
    let res = blockscout_client
        .request(&blockscout::stats::Stats {})
        .await?;
    res.coin_price.ok_or_else(|| {
        ServiceError::Internal(anyhow::anyhow!(
            "coin_price not available for chain {chain_id}"
        ))
    })
}
