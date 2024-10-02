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
    pub api_url: Option<url::Url>,
    pub create_database: bool,
    pub run_migrations: bool,
    pub blockscout_db_url: String,
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
            api_url: None,
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
    pub enabled: bool,
    pub blocks_ratio_threshold: Option<f32>,
    pub internal_transactions_ratio_threshold: Option<f32>,
    pub check_period_secs: u32,
}

impl Default for StartConditionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            // in some networks it's always almost 1
            blocks_ratio_threshold: Some(0.98),
            internal_transactions_ratio_threshold: Some(0.98),
            check_period_secs: 5,
        }
    }
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "STATS";
}
