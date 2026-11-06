// SPDX-License-Identifier: LicenseRef-Blockscout

use std::{collections::BTreeMap, num::NonZeroU32, path::PathBuf};

use blockscout_service_launcher::{
    database::{DatabaseConnectSettings, DatabaseSettings},
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use interchain_indexer_logic::{
    ChainInfoServiceSettings, TokenInfoServiceSettings, amb::settings::AmbIndexerSettings,
    avalanche::settings::AvalancheIndexerSettings, example::settings::ExampleIndexerSettings,
    settings::MessageBufferSettings, xdai::settings::XDaiIndexerSettings,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub chains_config: PathBuf,
    pub bridges_config: PathBuf,
    /// Optional JSON array of cluster presets (named presets of
    /// `home_chain_id` / `counterparty_chain_ids` / `bridge_ids` /
    /// `include_unindexed_chains`). Unset ⇒ clusters come only from
    /// `INTERCHAIN_INDEXER_CLUSTERS*` env vars; with neither, none are configured.
    #[serde(default)]
    pub clusters_config: Option<PathBuf>,

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
    pub amb_indexer: AmbIndexerSettings,

    #[serde(default)]
    pub xdai_indexer: XDaiIndexerSettings,

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

    #[serde(default)]
    pub write_api: WriteApiSettings,
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
    /// When true, run the stats backfill at startup (after migrations, before
    /// indexers or the API start): it scans canonical rows with
    /// `stats_processed = 0` and projects only the eligible ones — finalized
    /// (completed on any bridge, or failed on an AMB bridge) and, for messages,
    /// with a non-null destination chain. Ineligible rows are left unprojected.
    ///
    /// This is the required catch-up mechanism after any projection-invalidating
    /// migration (for example the bridge-qualified stats rebuild), not only the
    /// original stats-table introduction: such a migration clears the additive
    /// aggregates and resets the canonical markers, so the maintenance
    /// deployment MUST run once with this flag enabled and MUST block until
    /// backfill reaches idle before serving. Starting with the flag false on
    /// such a database would serve empty historical projections. Keep it
    /// default-false for steady-state operation to avoid routine startup scans.
    /// Env: `INTERCHAIN_INDEXER__STATS__BACKFILL_ON_START`.
    pub backfill_on_start: bool,

    /// Interval between full recomputations of the distinct message/transfer
    /// user counters, both the global `stats_chains` snapshot and the
    /// bridge-qualified `stats_chains_by_bridge` snapshot — one worker
    /// rebuilds both, atomically, in the same transaction.
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

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct WriteApiSettings {
    /// Write API key catalogue: key name -> hex SHA-256 digest of the key.
    /// The name (lowercased by the env loader) is the actor recorded in the
    /// audit log. Env: `INTERCHAIN_INDEXER__WRITE_API__KEYS_SHA256__<KEY_NAME>`.
    pub keys_sha256: BTreeMap<String, String>,
}

// Names only: a digest is derived from a credential, and a mistyped value may be
// the credential itself.
impl std::fmt::Debug for WriteApiSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteApiSettings")
            .field("key_names", &self.keys_sha256.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Settings {
    pub fn default(database_url: String) -> Self {
        Self {
            chains_config: PathBuf::from("config/omnibridge/chains.json"),
            bridges_config: PathBuf::from("config/omnibridge/bridges.json"),
            clusters_config: None,
            token_info: TokenInfoServiceSettings::default(),
            chain_info: ChainInfoServiceSettings::default(),
            example_indexer: Default::default(),
            avalanche_indexer: Default::default(),
            amb_indexer: Default::default(),
            xdai_indexer: Default::default(),
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
            write_api: Default::default(),
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

    #[derive(Debug, Deserialize)]
    struct WriteApiSettingsWrapper {
        #[serde(default)]
        write_api: WriteApiSettings,
    }

    #[test]
    fn write_api_settings_deserializes_named_digests() {
        let digest = "ab".repeat(32);
        let cfg = Config::builder()
            .set_override("write_api.keys_sha256.alice", digest.clone())
            .expect("set_override")
            .build()
            .expect("config");

        let v: WriteApiSettingsWrapper = cfg.try_deserialize().expect("deserialize");
        assert_eq!(
            v.write_api.keys_sha256,
            BTreeMap::from([("alice".to_string(), digest.clone())])
        );
        assert!(
            !format!("{:?}", v.write_api).contains(&digest),
            "Debug must not print digests"
        );
    }

    #[test]
    fn write_api_settings_default_is_empty() {
        let s = Settings::default("postgres://localhost/db".into());
        assert_eq!(s.write_api, WriteApiSettings::default());
        assert!(s.write_api.keys_sha256.is_empty());
    }

    #[derive(Debug, Deserialize)]
    struct ApiSettingsWrapper {
        #[serde(default)]
        api: ApiSettings,
    }

    fn api_settings_with_max_page_size(
        max_page_size: u64,
    ) -> Result<ApiSettings, config::ConfigError> {
        Config::builder()
            .set_override("api.max_page_size", max_page_size)
            .expect("set_override")
            .build()
            .expect("config")
            .try_deserialize::<ApiSettingsWrapper>()
            .map(|wrapper| wrapper.api)
    }

    #[test]
    fn api_settings_reject_zero_max_page_size() {
        let err = api_settings_with_max_page_size(0)
            .expect_err("a zero max_page_size must fail at config load");
        assert!(err.to_string().contains("nonzero"), "{err}");
    }

    #[test]
    fn api_settings_accept_positive_max_page_size() {
        let api = api_settings_with_max_page_size(5).expect("deserialize");
        assert_eq!(api.max_page_size.get(), 5);
        assert_eq!(
            api.default_page_size,
            ApiSettings::default().default_page_size
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ApiSettings {
    pub default_page_size: u32,
    /// Non-zero by type: list handlers clamp `page_size` to `1..=max_page_size`,
    /// and `clamp(1, 0)` panics, so a zero maximum fails at config load
    /// instead of on every list request.
    pub max_page_size: NonZeroU32,
}

const DEFAULT_MAX_PAGE_SIZE: NonZeroU32 = NonZeroU32::new(100).unwrap();

impl Default for ApiSettings {
    fn default() -> Self {
        Self {
            default_page_size: 50,
            max_page_size: DEFAULT_MAX_PAGE_SIZE,
        }
    }
}
