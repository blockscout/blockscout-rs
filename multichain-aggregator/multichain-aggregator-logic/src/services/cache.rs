use crate::{
    repository::tokens::ListClusterTokensPageToken,
    services::{self, coin_price::CoinPriceCache},
    types::{
        chain_metrics::ChainMetrics,
        domains::{Domain, DomainInfo, ProtocolInfo},
        tokens::AggregatedToken,
    },
};
use recache::{handler::CacheHandler, stores::redis::RedisStore};

pub type DecodedCalldataCache = CacheHandler<RedisStore, String, serde_json::Value>;
pub type DomainSearchCache = CacheHandler<RedisStore, String, (Vec<Domain>, Option<String>)>;
pub type DomainInfoCache = CacheHandler<RedisStore, String, Option<DomainInfo>>;
pub type DomainProtocolsCache = CacheHandler<RedisStore, String, Vec<ProtocolInfo>>;
pub type TokenSearchCache =
    CacheHandler<RedisStore, String, (Vec<AggregatedToken>, Option<ListClusterTokensPageToken>)>;
pub type ChainMetricsCache = CacheHandler<RedisStore, String, Vec<ChainMetrics>>;

#[derive(Default, Clone)]
pub struct ClusterCaches {
    pub decoded_calldata: Option<DecodedCalldataCache>,
    pub domain_search: Option<DomainSearchCache>,
    pub domain_info: Option<DomainInfoCache>,
    pub domain_protocols: Option<DomainProtocolsCache>,
    pub token_search: Option<TokenSearchCache>,
    pub chain_metrics: Option<ChainMetricsCache>,
    pub marketplace_enabled: services::chains::MarketplaceEnabledCache,
    pub coin_price: Option<CoinPriceCache>,
}
