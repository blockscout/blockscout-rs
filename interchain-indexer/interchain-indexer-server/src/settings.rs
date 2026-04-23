use std::path::PathBuf;

use blockscout_service_launcher::{
    database::{DatabaseConnectSettings, DatabaseSettings},
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use interchain_indexer_logic::{
    ChainInfoServiceSettings, TokenInfoServiceSettings,
    avalanche::settings::AvalancheIndexerSettings, example::settings::ExampleIndexerSettings,
    settings::MessageBufferSettings,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub chains_config: PathBuf,
    pub bridges_config: PathBuf,

    #[serde(default)]
    pub token_info: TokenInfoServiceSettings,

    #[serde(default)]
    pub chain_info: ChainInfoServiceSettings,

    #[serde(default)]
    pub buffer_settings: MessageBufferSettings,

    #[serde(default)]
    pub example_indexer: ExampleIndexerSettings,

    #[serde(default)]
    pub avalanche_indexer: AvalancheIndexerSettings,

    #[serde(default)]
    pub server: ServerSettings,
    #[serde(default)]
    pub metrics: MetricsSettings,
    #[serde(default)]
    pub tracing: TracingSettings,
    #[serde(default)]
    pub jaeger: JaegerSettings,
    #[serde(default = "default_swagger_path")]
    pub swagger_path: PathBuf,

    pub database: DatabaseSettings,

    #[serde(default)]
    pub api: ApiSettings,

    #[serde(default)]
    pub stats: StatsSettings,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "INTERCHAIN_INDEXER";
}

fn default_swagger_path() -> PathBuf {
    blockscout_endpoint_swagger::default_swagger_path_from_service_name("interchain-indexer")
}

fn default_stats_chains_recalculation_period_secs() -> u64 {
    3600
}

fn default_stats_include_zero_chains() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct StatsSettings {
    /// When true, project all rows with `stats_processed = 0` into stats tables at startup
    /// (after migrations), before indexers start.
    /// Env: `INTERCHAIN_INDEXER__STATS__BACKFILL_ON_START`.
    pub backfill_on_start: bool,

    /// Interval between full `stats_chains` recomputations (distinct message/transfer users per chain).
    /// Set to `0` to disable the background task. Unit: seconds.
    /// Env: `INTERCHAIN_INDEXER__STATS__CHAINS_RECALCULATION_PERIOD_SECS`.
    pub chains_recalculation_period_secs: u64,

    /// When true, stats endpoints include known chains from `chains` even when aggregated stats
    /// are missing or zero.
    /// Env: `INTERCHAIN_INDEXER__STATS__INCLUDE_ZERO_CHAINS`.
    pub include_zero_chains: bool,
}

impl Default for StatsSettings {
    fn default() -> Self {
        Self {
            backfill_on_start: false,
            chains_recalculation_period_secs: default_stats_chains_recalculation_period_secs(),
            include_zero_chains: default_stats_include_zero_chains(),
        }
    }
}

impl Settings {
    pub fn default(database_url: String) -> Self {
        Self {
            chains_config: PathBuf::from("config/omnibridge/chains.json"),
            bridges_config: PathBuf::from("config/omnibridge/bridges.json"),
            token_info: TokenInfoServiceSettings::default(),
            chain_info: ChainInfoServiceSettings::default(),
            example_indexer: Default::default(),
            avalanche_indexer: Default::default(),
            server: Default::default(),
            metrics: Default::default(),
            tracing: Default::default(),
            jaeger: Default::default(),
            swagger_path: default_swagger_path(),
            database: DatabaseSettings {
                connect: DatabaseConnectSettings::Url(database_url),
                create_database: Default::default(),
                run_migrations: Default::default(),
                connect_options: Default::default(),
            },
            api: Default::default(),
            buffer_settings: Default::default(),
            stats: StatsSettings::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::Config;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct StatsSettingsWrapper {
        #[serde(default)]
        stats: StatsSettings,
    }

    #[test]
    fn settings_default_populates_nested_stats_defaults() {
        let s = Settings::default("postgres://localhost/db".into());
        assert_eq!(s.stats, StatsSettings::default());
    }

    #[test]
    fn stats_settings_empty_config_uses_nested_defaults() {
        let cfg = Config::builder().build().expect("config");
        let v: StatsSettings = cfg.try_deserialize().expect("deserialize");
        assert_eq!(v, StatsSettings::default());
    }

    #[test]
    fn stats_settings_deserializes_nested_overrides() {
        let cfg = Config::builder()
            .set_override("stats.backfill_on_start", true)
            .expect("set_override")
            .set_override("stats.chains_recalculation_period_secs", 120u64)
            .expect("set_override")
            .set_override("stats.include_zero_chains", false)
            .expect("set_override")
            .build()
            .expect("config");

        let v: StatsSettingsWrapper = cfg.try_deserialize().expect("deserialize");
        assert_eq!(
            v.stats,
            StatsSettings {
                backfill_on_start: true,
                chains_recalculation_period_secs: 120,
                include_zero_chains: false,
            }
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ApiSettings {
    pub default_page_size: u32,
    pub max_page_size: u32,
    pub use_pagination_token: bool,
}

impl Default for ApiSettings {
    fn default() -> Self {
        Self {
            default_page_size: 50,
            max_page_size: 100,
            use_pagination_token: true,
        }
    }
}
