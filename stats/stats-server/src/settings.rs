use blockscout_service_launcher::{
    launcher::{
        ConfigSettings, GrpcServerSettings, HttpServerSettings, MetricsSettings, ServerSettings,
    },
    tracing::{JaegerSettings, TracingSettings},
};
use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use stats::{
    counters::{LastNewContracts, TotalOperationalTxns},
    lines::{ContractsGrowth, NewContracts, NewOperationalTxns, OperationalTxnsGrowth},
    ChartProperties,
};
use std::{net::SocketAddr, path::PathBuf, str::FromStr};
use tracing::warn;

use crate::config::{self, types::AllChartSettings};

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub db_url: String,
    pub create_database: bool,
    pub run_migrations: bool,
    pub blockscout_db_url: String,
    /// Blockscout API url.
    ///
    /// Required. To launch without it api use [`Settings::ignore_blockscout_api_absence`].
    pub blockscout_api_url: Option<url::Url>,
    /// Disable functionality that utilizes [`Settings::blockscout_api_url`] if the parameter
    /// is not provided. By default the url is required to not silently suppress such features.
    pub ignore_blockscout_api_absence: bool,
    /// Disable functionality that utilizes internal transactions. In particular, it disables
    /// internal transactions ratio check for starting the service and related charts.
    ///
    /// It has a higher priority than config files and respective envs.
    pub disable_internal_transactions: bool,
    /// Enable arbitrum-specific charts
    pub enable_all_arbitrum: bool,
    #[serde_as(as = "DisplayFromStr")]
    pub default_schedule: Schedule,
    pub force_update_on_start: Option<bool>, // None = no update
    pub concurrent_start_updates: usize,
    pub limits: LimitsSettings,
    pub conditional_start: StartConditionSettings,
    pub charts_config: PathBuf,
    pub layout_config: PathBuf,
    pub update_groups_config: PathBuf,
    /// Location of swagger file to serve
    pub swagger_file: PathBuf,

    pub server: ServerSettings,
    pub metrics: MetricsSettings,
    pub jaeger: JaegerSettings,
    pub tracing: TracingSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: ServerSettings {
                http: HttpServerSettings {
                    enabled: true,
                    addr: SocketAddr::from_str("0.0.0.0:8050").unwrap(),
                    max_body_size: 2 * 1024 * 1024, // 2 Mb - default Actix value
                    cors: Default::default(),
                },
                grpc: GrpcServerSettings {
                    enabled: false,
                    addr: SocketAddr::from_str("0.0.0.0:8051").unwrap(),
                },
            },
            db_url: Default::default(),
            default_schedule: Schedule::from_str("0 0 1 * * * *").unwrap(),
            force_update_on_start: Some(false),
            concurrent_start_updates: 3,
            limits: Default::default(),
            conditional_start: Default::default(),
            charts_config: PathBuf::from_str("config/charts.json").unwrap(),
            layout_config: PathBuf::from_str("config/layout.json").unwrap(),
            update_groups_config: PathBuf::from_str("config/update_groups.json").unwrap(),
            swagger_file: PathBuf::from("../stats-proto/swagger/stats.swagger.yaml"),
            blockscout_db_url: Default::default(),
            blockscout_api_url: None,
            ignore_blockscout_api_absence: false,
            disable_internal_transactions: false,
            enable_all_arbitrum: false,
            create_database: Default::default(),
            run_migrations: Default::default(),
            metrics: Default::default(),
            jaeger: Default::default(),
            tracing: Default::default(),
        }
    }
}

pub fn handle_disable_internal_transactions(
    disable_internal_transactions: bool,
    conditional_start: &mut StartConditionSettings,
    charts: &mut config::charts::Config<AllChartSettings>,
) {
    if disable_internal_transactions {
        conditional_start.internal_transactions_ratio.enabled = false;
        for disable_key in [
            NewContracts::key().name(),
            LastNewContracts::key().name(),
            ContractsGrowth::key().name(),
        ] {
            let settings = match (
                charts.lines.get_mut(disable_key),
                charts.counters.get_mut(disable_key),
            ) {
                (Some(settings), _) => settings,
                (_, Some(settings)) => settings,
                _ => {
                    warn!(
                        "Could not disable internal transactions related chart {}: chart not found in settings. \
                        This should not be a problem for running the service.",
                        disable_key
                    );
                    continue;
                }
            };
            settings.enabled = false;
        }
    }
}

pub fn handle_enable_all_arbitrum(
    enable_all_arbitrum: bool,
    charts: &mut config::charts::Config<AllChartSettings>,
) {
    if enable_all_arbitrum {
        for enable_key in [
            NewOperationalTxns::key().name(),
            OperationalTxnsGrowth::key().name(),
            TotalOperationalTxns::key().name(),
        ] {
            let settings = match (
                charts.lines.get_mut(enable_key),
                charts.counters.get_mut(enable_key),
            ) {
                (Some(settings), _) => settings,
                (_, Some(settings)) => settings,
                _ => {
                    warn!(
                        "Could not enable arbitrum-specific chart {}: chart not found in settings. \
                        This should not be a problem for running the service.",
                        enable_key
                    );
                    continue;
                }
            };
            settings.enabled = true;
        }
    }
}

/// Various limits like rate limiting and restrictions on input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct LimitsSettings {
    /// Limit date interval on corresponding requests (in days).
    /// (i.e. `?from=2024-03-17&to=2024-04-17`).
    ///
    /// If start or end of the range is left empty, min/max values
    /// from DB are considered.
    pub requested_points_limit: u32,
}

impl Default for LimitsSettings {
    fn default() -> Self {
        Self {
            // ~500 years for days seems reasonable
            requested_points_limit: 182500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StartConditionSettings {
    pub blocks_ratio: ToggleableThreshold,
    pub internal_transactions_ratio: ToggleableThreshold,
    pub check_period_secs: u32,
}

impl Default for StartConditionSettings {
    fn default() -> Self {
        Self {
            // in some networks it's always almost 1
            blocks_ratio: ToggleableThreshold::default(),
            internal_transactions_ratio: ToggleableThreshold::default(),
            check_period_secs: 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ToggleableThreshold {
    pub enabled: bool,
    pub threshold: f64,
}

impl ToggleableThreshold {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            threshold: 0.0,
        }
    }

    pub fn enabled(value: f64) -> Self {
        Self {
            enabled: true,
            threshold: value,
        }
    }

    pub fn set_threshold(mut self, value: f64) -> Self {
        self.threshold = value;
        self
    }

    pub fn set_disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl Default for ToggleableThreshold {
    fn default() -> Self {
        Self::enabled(0.98)
    }
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "STATS";
}

#[cfg(test)]
mod tests {
    use crate::config_env::test_utils::check_envs_parsed_to;
    use stats::counters::TotalContracts;

    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn start_condition_thresholds_can_be_disabled_with_envs() {
        check_envs_parsed_to(
            "START_SETTINGS",
            [(
                "START_SETTINGS__BLOCKS_RATIO__ENABLED".to_owned(),
                "false".to_owned(),
            )]
            .into(),
            StartConditionSettings {
                blocks_ratio: ToggleableThreshold::default().set_disabled(),
                ..StartConditionSettings::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn disable_internal_transactions_works_correctly() {
        let mut settings = Settings::default();
        let charts_settings_default_enabled = config::types::AllChartSettings {
            enabled: true,
            ..Default::default()
        };
        let mut charts = config::charts::Config {
            counters: [
                (
                    LastNewContracts::key().name().to_owned(),
                    charts_settings_default_enabled.clone(),
                ),
                (
                    TotalContracts::key().name().to_owned(),
                    charts_settings_default_enabled.clone(),
                ),
            ]
            .iter()
            .cloned()
            .collect(),
            lines: [
                (
                    NewContracts::key().name().to_owned(),
                    charts_settings_default_enabled.clone(),
                ),
                (
                    ContractsGrowth::key().name().to_owned(),
                    charts_settings_default_enabled.clone(),
                ),
            ]
            .iter()
            .cloned()
            .collect(),
        };

        settings.disable_internal_transactions = true;
        handle_disable_internal_transactions(
            settings.disable_internal_transactions,
            &mut settings.conditional_start,
            &mut charts,
        );

        assert_eq!(
            settings
                .conditional_start
                .internal_transactions_ratio
                .enabled,
            false
        );
        assert_eq!(
            charts
                .lines
                .get(NewContracts::key().name())
                .unwrap()
                .enabled,
            false
        );
        assert_eq!(
            charts
                .lines
                .get(ContractsGrowth::key().name())
                .unwrap()
                .enabled,
            false
        );
        assert_eq!(
            charts
                .counters
                .get(LastNewContracts::key().name())
                .unwrap()
                .enabled,
            false
        );
        assert_eq!(
            charts
                .counters
                .get(TotalContracts::key().name())
                .unwrap()
                .enabled,
            true
        );
    }
}
