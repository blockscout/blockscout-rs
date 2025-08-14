use blockscout_service_launcher::{
    launcher::{
        ConfigSettings, GrpcServerSettings, HttpServerSettings, MetricsSettings, ServerSettings,
    },
    tracing::{JaegerSettings, TracingSettings},
};
use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use stats::{
    ChartProperties, Named,
    counters::{
        ArbitrumNewOperationalTxns24h, ArbitrumTotalOperationalTxns,
        ArbitrumYesterdayOperationalTxns, NewZetachainCrossChainTxns24h,
        OpStackNewOperationalTxns24h, OpStackTotalOperationalTxns, OpStackYesterdayOperationalTxns,
        PendingZetachainCrossChainTxns, TotalZetachainCrossChainTxns,
        multichain::TotalInteropMessages,
    },
    indexing_status::BlockscoutIndexingStatus,
    lines::{
        ArbitrumNewOperationalTxns, ArbitrumNewOperationalTxnsWindow,
        ArbitrumOperationalTxnsGrowth, Eip7702AuthsGrowth, NewEip7702Auths,
        NewZetachainCrossChainTxns, OpStackNewOperationalTxns, OpStackNewOperationalTxnsWindow,
        OpStackOperationalTxnsGrowth, ZetachainCrossChainTxnsGrowth,
    },
};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    net::SocketAddr,
    path::PathBuf,
    str::FromStr,
};
use tracing::warn;

use crate::{
    RuntimeSetup,
    config::{self, types::AllChartSettings},
};

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub db_url: String,
    pub create_database: bool,
    pub run_migrations: bool,
    pub blockscout_db_url: Option<String>,
    pub indexer_db_url: Option<String>,
    /// Url for second db of indexer (currently assumed to be CCTX (cross chain transactions) indexer, see `zetachain-cctx` service)
    pub second_indexer_db_url: Option<String>,
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
    /// Enable op-stack-specific charts
    pub enable_all_op_stack: bool,
    /// Enable EIP-7702 charts
    pub enable_all_eip_7702: bool,
    /// Enable stats for zetachain CCTX data. Requires `second_indexer_db_url` to be set.
    ///
    /// Note: it's not a interop/multichain CCTX stats, therefore it's not compatible with
    /// `multichain_mode`.
    pub enable_zetachain_cctx: bool,
    /// Enable multichain mode.
    ///
    /// This will run stats service for a multichain explorer.
    /// It takes precedence over other chart-related flags like `enable_all_arbitrum`,
    /// `enable_all_op_stack`, `enable_all_eip_7702`.
    pub multichain_mode: bool,
    #[serde_as(as = "DisplayFromStr")]
    pub default_schedule: Schedule,
    pub force_update_on_start: Option<bool>, // None = no update
    pub concurrent_start_updates: usize,
    pub limits: LimitsSettings,
    pub conditional_start: StartConditionSettings,
    pub charts_config: PathBuf,
    pub layout_config: PathBuf,
    pub update_groups_config: PathBuf,
    /// Used only if `multichain_mode` is enabled as a second-priority config.
    pub multichain_charts_config: Option<PathBuf>,
    /// Used only if `multichain_mode` is enabled as a second-priority config.
    pub multichain_layout_config: Option<PathBuf>,
    /// Used only if `multichain_mode` is enabled as a second-priority config.
    pub multichain_update_groups_config: Option<PathBuf>,
    /// Location of swagger file to serve
    pub swagger_path: PathBuf,
    pub api_keys: HashMap<String, String>,

    pub server: ServerSettings,
    pub metrics: MetricsSettings,
    pub jaeger: JaegerSettings,
    pub tracing: TracingSettings,
}

fn default_swagger_path() -> PathBuf {
    blockscout_endpoint_swagger::default_swagger_path_from_service_name("stats")
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
                    base_path: None,
                },
                grpc: GrpcServerSettings {
                    enabled: false,
                    addr: SocketAddr::from_str("0.0.0.0:8051").unwrap(),
                },
            },
            api_keys: Default::default(),
            db_url: Default::default(),
            default_schedule: Schedule::from_str("0 0 1 * * * *").unwrap(),
            force_update_on_start: Some(false),
            concurrent_start_updates: 3,
            limits: Default::default(),
            conditional_start: Default::default(),
            charts_config: PathBuf::from_str("config/charts.json").unwrap(),
            layout_config: PathBuf::from_str("config/layout.json").unwrap(),
            update_groups_config: PathBuf::from_str("config/update_groups.json").unwrap(),
            multichain_charts_config: Some(
                PathBuf::from_str("config/multichain/charts.json").unwrap(),
            ),
            multichain_layout_config: Some(
                PathBuf::from_str("config/multichain/layout.json").unwrap(),
            ),
            multichain_update_groups_config: Some(
                PathBuf::from_str("config/multichain/update_groups.json").unwrap(),
            ),
            swagger_path: default_swagger_path(),
            blockscout_db_url: Default::default(),
            indexer_db_url: Default::default(),
            second_indexer_db_url: Default::default(),
            blockscout_api_url: None,
            ignore_blockscout_api_absence: false,
            disable_internal_transactions: false,
            enable_all_arbitrum: false,
            enable_all_op_stack: false,
            enable_all_eip_7702: false,
            enable_zetachain_cctx: false,
            multichain_mode: false,
            create_database: Default::default(),
            run_migrations: Default::default(),
            metrics: Default::default(),
            jaeger: Default::default(),
            tracing: Default::default(),
        }
    }
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "STATS";
}

impl Settings {
    fn config_paths(
        multichain_mode: bool,
        main_config: PathBuf,
        multichain_config: &Option<PathBuf>,
    ) -> Vec<PathBuf> {
        let mut list = vec![main_config];
        match multichain_config {
            Some(m) if multichain_mode => list.push(m.clone()),
            _ => (),
        }
        list
    }

    pub fn charts_config_paths(&self) -> Vec<PathBuf> {
        Self::config_paths(
            self.multichain_mode,
            self.charts_config.clone(),
            &self.multichain_charts_config,
        )
    }

    pub fn layout_config_paths(&self) -> Vec<PathBuf> {
        Self::config_paths(
            self.multichain_mode,
            self.layout_config.clone(),
            &self.multichain_layout_config,
        )
    }

    pub fn update_groups_config_paths(&self) -> Vec<PathBuf> {
        Self::config_paths(
            self.multichain_mode,
            self.update_groups_config.clone(),
            &self.multichain_update_groups_config,
        )
    }
}

pub fn handle_disable_internal_transactions(
    disable_internal_transactions: bool,
    conditional_start: &mut StartConditionSettings,
    charts: &mut config::charts::Config<AllChartSettings>,
) {
    if disable_internal_transactions {
        conditional_start.internal_transactions_ratio.enabled = false;
        let charts_dependant_on_internal_transactions =
            RuntimeSetup::all_members_indexing_status_requirements()
                .into_iter()
                .filter(|(_k, req)| {
                    req.blockscout == BlockscoutIndexingStatus::InternalTransactionsIndexed
                })
                .map(|(k, _req)| k.into_name());
        let to_disable: BTreeSet<_> = charts_dependant_on_internal_transactions.collect();

        for disable_name in to_disable {
            let settings = match (
                charts.lines.get_mut(&disable_name),
                charts.counters.get_mut(&disable_name),
            ) {
                (Some(settings), _) => settings,
                (_, Some(settings)) => settings,
                _ => {
                    warn!(
                        "Could not disable internal transactions related chart {}: chart not found in settings. \
                        This should not be a problem for running the service.",
                        disable_name
                    );
                    continue;
                }
            };
            settings.enabled = false;
        }
    }
}

fn enable_charts(
    to_enable: &[&str],
    charts: &mut config::charts::Config<AllChartSettings>,
    charts_name_for_logs: &str,
) {
    for enable_key in to_enable {
        let settings = match (
            charts.lines.get_mut(*enable_key),
            charts.counters.get_mut(*enable_key),
        ) {
            (Some(settings), _) => settings,
            (_, Some(settings)) => settings,
            _ => {
                warn!(
                    "Could not enable '{charts_name_for_logs}'-specific chart {enable_key}: \
                    chart not found in settings. \
                    This should not be a problem for running the service.",
                );
                continue;
            }
        };
        settings.enabled = true;
    }
}

pub fn handle_enable_all_arbitrum(
    enable_all: bool,
    charts: &mut config::charts::Config<AllChartSettings>,
) {
    if enable_all {
        enable_charts(
            &[
                ArbitrumNewOperationalTxns::key().name(),
                ArbitrumNewOperationalTxnsWindow::key().name(),
                ArbitrumTotalOperationalTxns::key().name(),
                ArbitrumNewOperationalTxns24h::key().name(),
                ArbitrumOperationalTxnsGrowth::key().name(),
                ArbitrumYesterdayOperationalTxns::key().name(),
            ],
            charts,
            "arbitrum",
        )
    }
}

pub fn handle_enable_all_op_stack(
    enable_all: bool,
    charts: &mut config::charts::Config<AllChartSettings>,
) {
    if enable_all {
        enable_charts(
            &[
                OpStackNewOperationalTxns::key().name(),
                OpStackNewOperationalTxnsWindow::key().name(),
                OpStackTotalOperationalTxns::key().name(),
                OpStackNewOperationalTxns24h::key().name(),
                OpStackOperationalTxnsGrowth::key().name(),
                OpStackYesterdayOperationalTxns::key().name(),
            ],
            charts,
            "op-stack",
        )
    }
}

pub fn handle_enable_all_eip_7702(
    enable_all: bool,
    charts: &mut config::charts::Config<AllChartSettings>,
) {
    if enable_all {
        enable_charts(
            &[
                NewEip7702Auths::key().name(),
                Eip7702AuthsGrowth::key().name(),
            ],
            charts,
            "eip-7702",
        )
    }
}

pub fn handle_enable_zetachain_cctx(
    settings: &mut Settings,
    charts: &mut config::charts::Config<AllChartSettings>,
) {
    if !settings.enable_zetachain_cctx {
        return;
    }
    if settings.multichain_mode {
        panic!(
            "Multichain mode is not compatible with enabling local cross chain transactions stats"
        );
    }
    enable_charts(
        &[
            NewZetachainCrossChainTxns::key().name(),
            ZetachainCrossChainTxnsGrowth::key().name(),
            NewZetachainCrossChainTxns24h::key().name(),
            PendingZetachainCrossChainTxns::key().name(),
            TotalZetachainCrossChainTxns::key().name(),
        ],
        charts,
        "zetachain-cctx",
    );
    settings
        .conditional_start
        .zetachain_indexed_until_today
        .enabled = true;
}

pub fn disable_all_non_multichain_charts(charts: &mut config::charts::Config<AllChartSettings>) {
    let multichain_charts = HashSet::from([TotalInteropMessages::name()]);
    for (name, settings) in charts.lines.iter_mut() {
        if !multichain_charts.contains(name) {
            settings.enabled = false;
        }
    }
    for (name, settings) in charts.counters.iter_mut() {
        if !multichain_charts.contains(name) {
            settings.enabled = false;
        }
    }
}

pub fn apply_multichain_mode_settings(settings: &mut Settings) {
    settings.blockscout_api_url = None;
    settings.ignore_blockscout_api_absence = true;
    settings.conditional_start.blocks_ratio.enabled = false;
    settings
        .conditional_start
        .internal_transactions_ratio
        .enabled = false;
    settings
        .conditional_start
        .user_ops_past_indexing_finished
        .enabled = false;
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
    pub user_ops_past_indexing_finished: ToggleableCheck,
    #[serde(default = "ToggleableCheck::disabled")]
    pub zetachain_indexed_until_today: ToggleableCheck,
    pub check_period_secs: u32,
}

impl Default for StartConditionSettings {
    fn default() -> Self {
        Self {
            // in some networks it's always almost 1
            blocks_ratio: ToggleableThreshold::default(),
            internal_transactions_ratio: ToggleableThreshold::default(),
            user_ops_past_indexing_finished: ToggleableCheck::default(),
            zetachain_indexed_until_today: ToggleableCheck::disabled(),
            check_period_secs: 5,
        }
    }
}

impl StartConditionSettings {
    pub fn blockscout_checks_enabled(&self) -> bool {
        self.blocks_ratio.enabled || self.internal_transactions_ratio.enabled
    }
    pub fn user_ops_checks_enabled(&self) -> bool {
        self.user_ops_past_indexing_finished.enabled
    }
    pub fn zetachain_checks_enabled(&self) -> bool {
        self.zetachain_indexed_until_today.enabled
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ToggleableCheck {
    pub enabled: bool,
}

impl ToggleableCheck {
    pub fn disabled() -> Self {
        Self { enabled: false }
    }
}

impl Default for ToggleableCheck {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[cfg(test)]
mod tests {
    use crate::config_env::test_utils::check_envs_parsed_to;
    use stats::{
        counters::{LastNewContracts, TotalContracts},
        lines::{ContractsGrowth, NewContracts},
    };

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
