use blockscout_service_launcher::{
    database::{DatabaseConnectSettings, DatabaseSettings, ReplicaDatabaseSettings},
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use serde::{Deserialize, Serialize};
use serde_with::{StringWithSeparator, formats::CommaSeparator, serde_as};
use std::{collections::HashMap, time};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub server: ServerSettings,
    #[serde(default)]
    pub metrics: MetricsSettings,
    #[serde(default)]
    pub tracing: TracingSettings,
    #[serde(default)]
    pub jaeger: JaegerSettings,
    pub database: DatabaseSettings,
    // Optional database read-only replica. If provided, all search queries will be redirected to this database.
    #[serde(default)]
    pub replica_database: Option<ReplicaDatabaseSettings>,
    pub service: ServiceSettings,
    pub cache: Option<CacheSettings>,
    #[serde(default)]
    pub cluster_explorer: ClusterExplorerSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CacheSettings {
    pub redis: RedisSettings,
    #[serde(default = "default_domain_search_cache")]
    pub domain_search_cache: CacheEntrySettings,
    #[serde(default = "default_decoded_calldata_cache")]
    pub decoded_calldata_cache: CacheEntrySettings,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CacheEntrySettings {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub ttl: time::Duration,
    #[serde_as(as = "Option<serde_with::DurationSeconds<u64>>")]
    pub refresh_ahead: Option<time::Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RedisSettings {
    pub url: Url,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ServiceSettings {
    pub dapp_client: HttpApiClientSettings,
    pub bens_client: HttpApiClientSettings,
    #[serde(default)]
    pub api: ApiSettings,
    // Chains that will be used for quick search (ordered by priority).
    // NOTE: entities from other chains may still appear in the final result,
    // but only if we can't find enough entries from the chains in the list.
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, i64>")]
    #[serde(default = "default_quick_search_chains")]
    pub quick_search_chains: Vec<i64>,
    #[serde(default)]
    pub fetch_chains: bool,
    #[serde(default = "default_bens_protocols")]
    pub bens_protocols: Option<Vec<String>>,
    #[serde(default = "default_domain_primary_chain_id")]
    pub domain_primary_chain_id: i64,
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    #[serde(default = "default_marketplace_enabled_cache_update_interval")]
    pub marketplace_enabled_cache_update_interval: time::Duration,
    #[serde(default = "default_marketplace_enabled_cache_fetch_concurrency")]
    pub marketplace_enabled_cache_fetch_concurrency: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ClusterExplorerSettings {
    pub clusters: HashMap<String, ClusterSettings>,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ClusterSettings {
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, i64>")]
    pub chain_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApiSettings {
    #[serde(default = "default_default_page_size")]
    pub default_page_size: u32,
    #[serde(default = "default_max_page_size")]
    pub max_page_size: u32,
}

impl Default for ApiSettings {
    fn default() -> Self {
        Self {
            default_page_size: default_default_page_size(),
            max_page_size: default_max_page_size(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HttpApiClientSettings {
    pub url: Url,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "MULTICHAIN_AGGREGATOR";
}

impl Settings {
    pub fn default(database_url: String) -> Self {
        Self {
            server: Default::default(),
            metrics: Default::default(),
            tracing: Default::default(),
            jaeger: Default::default(),
            database: DatabaseSettings {
                connect: DatabaseConnectSettings::Url(database_url),
                connect_options: Default::default(),
                create_database: Default::default(),
                run_migrations: Default::default(),
            },
            replica_database: Default::default(),
            service: ServiceSettings {
                dapp_client: HttpApiClientSettings {
                    url: Url::parse("http://localhost:8050").unwrap(),
                },
                bens_client: HttpApiClientSettings {
                    url: Url::parse("http://localhost:8052").unwrap(),
                },
                api: ApiSettings {
                    default_page_size: default_default_page_size(),
                    max_page_size: default_max_page_size(),
                },
                quick_search_chains: default_quick_search_chains(),
                fetch_chains: false,
                bens_protocols: default_bens_protocols(),
                domain_primary_chain_id: default_domain_primary_chain_id(),
                marketplace_enabled_cache_update_interval:
                    default_marketplace_enabled_cache_update_interval(),
                marketplace_enabled_cache_fetch_concurrency:
                    default_marketplace_enabled_cache_fetch_concurrency(),
            },
            cache: None,
            cluster_explorer: Default::default(),
        }
    }
}

fn default_max_page_size() -> u32 {
    100
}

fn default_default_page_size() -> u32 {
    50
}

fn default_quick_search_chains() -> Vec<i64> {
    vec![
        1, 8453, 57073, 698, 109, 7777777, 100, 10, 42161, 690, 534352,
    ]
}

fn default_bens_protocols() -> Option<Vec<String>> {
    Some(vec!["ens".to_string()])
}

fn default_domain_primary_chain_id() -> i64 {
    1
}

fn default_marketplace_enabled_cache_update_interval() -> time::Duration {
    time::Duration::from_secs(6 * 60 * 60) // 6 hours
}

fn default_marketplace_enabled_cache_fetch_concurrency() -> usize {
    10
}

fn default_enabled() -> bool {
    true
}

fn default_decoded_calldata_cache() -> CacheEntrySettings {
    let hour = 60 * 60;
    let ttl = time::Duration::from_secs(hour);
    CacheEntrySettings {
        enabled: default_enabled(),
        ttl,
        refresh_ahead: None,
    }
}

fn default_domain_search_cache() -> CacheEntrySettings {
    let hour = 60 * 60;
    let ttl = time::Duration::from_secs(6 * hour);
    let refresh_ahead = ttl / 5; // 20% of ttl
    CacheEntrySettings {
        enabled: default_enabled(),
        ttl,
        refresh_ahead: Some(refresh_ahead),
    }
}
