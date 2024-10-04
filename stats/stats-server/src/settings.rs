use blockscout_service_launcher::{
    launcher::{
        ConfigSettings, GrpcServerSettings, HttpServerSettings, MetricsSettings, ServerSettings,
    },
    tracing::{JaegerSettings, TracingSettings},
};
use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::{net::SocketAddr, path::PathBuf, str::FromStr};

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
            create_database: Default::default(),
            run_migrations: Default::default(),
            metrics: Default::default(),
            jaeger: Default::default(),
            tracing: Default::default(),
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
    pub blocks_ratio_threshold: ToggleableThreshold,
    pub internal_transactions_ratio_threshold: ToggleableThreshold,
    pub check_period_secs: u32,
}

impl Default for StartConditionSettings {
    fn default() -> Self {
        Self {
            // in some networks it's always almost 1
            blocks_ratio_threshold: ToggleableThreshold::default(),
            internal_transactions_ratio_threshold: ToggleableThreshold::default(),
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

    use super::*;

    #[test]
    fn start_condition_thresholds_can_be_disabled_with_envs() {
        check_envs_parsed_to(
            "START_SETTINGS",
            [(
                "START_SETTINGS__BLOCKS_RATIO_THRESHOLD__ENABLED".to_owned(),
                "false".to_owned(),
            )]
            .into(),
            StartConditionSettings {
                blocks_ratio_threshold: ToggleableThreshold::default().set_disabled(),
                ..StartConditionSettings::default()
            },
        )
        .unwrap()
    }
}
