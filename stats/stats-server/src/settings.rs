// SPDX-License-Identifier: LicenseRef-Blockscout

use blockscout_service_launcher::{
    launcher::{
        ConfigSettings, GrpcServerSettings, HttpServerSettings, MetricsSettings, ServerSettings,
    },
    tracing::{JaegerSettings, TracingSettings},
};
use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, StringWithSeparator, formats::CommaSeparator, serde_as};
use stats::{
    ChartProperties,
    counters::{
        ArbitrumNewOperationalTxns24h, ArbitrumTotalOperationalTxns,
        ArbitrumYesterdayOperationalTxns, FilecoinChainFees24h, NewZetachainCrossChainTxns24h,
        OpStackNewOperationalTxns24h, OpStackTotalOperationalTxns, OpStackYesterdayOperationalTxns,
        PendingZetachainCrossChainTxns, TotalZetachainCrossChainTxns, TxnsFee24h,
    },
    indexing_status::BlockscoutIndexingStatus,
    lines::{
        ArbitrumNewOperationalTxns, ArbitrumNewOperationalTxnsWindow,
        ArbitrumOperationalTxnsGrowth, Eip7702AuthsGrowth, FilecoinChainFeesGrowth,
        FilecoinNewChainFees, NewEip7702Auths, NewZetachainCrossChainTxns,
        OpStackNewOperationalTxns, OpStackNewOperationalTxnsWindow, OpStackOperationalTxnsGrowth,
        TxnsFee, ZetachainCrossChainTxnsGrowth,
    },
};
use std::{
    collections::{BTreeSet, HashMap},
    net::SocketAddr,
    path::PathBuf,
    str::FromStr,
    time::Duration,
};
use tracing::{debug, warn};

use crate::{
    RuntimeSetup,
    config::{self, types::AllChartSettings},
};

pub use stats::Mode;

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub db_url: String,
    pub create_database: bool,
    pub run_migrations: bool,

    /// Mode determines the type of the underlying database and feature flags that were
    /// previously controlled by `enable_zetachain_cctx` and `multichain_mode`.
    ///
    /// The service can be run in one of the following modes:
    /// - `Blockscout`: run the service for a single blockscout instance (default)
    /// - `MultichainAggregator`: run the service for a multichain_aggregator
    /// - `Zetachain`: run the service for a zetachain instance
    /// - `Interchain`: run the service for a interchain indexer (aka Universal Bridge Indexer)
    ///
    /// Modes are mutually exclusive by design.
    pub mode: Mode,

    pub blockscout_db_url: Option<String>, // deprecated, use `indexer_db_url` instead
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
    /// Enable the Filecoin-specific API surface: `filecoinChainFeesGrowth`
    /// is enabled under its own id; the public `txnsFee` id is force-enabled
    /// and served with the `filecoinNewChainFees` implementation (chain-wide
    /// fees), and the public `txnsFee24h` id is force-enabled and served
    /// with the `filecoinChainFees24h` implementation (24h burn + tips);
    /// under this flag, `filecoinNewChainFees` and `filecoinChainFees24h`
    /// are never exposed as public chart ids.
    pub enable_all_filecoin: bool,
    /// Filter by chain ids for multichain mode.
    /// TODO: recalculate statistics data when multichain_filter has been changed.
    ///       Interchain solves the equivalent problem with a filter fingerprint
    ///       stored alongside the chart data
    ///       (`stats::charts::db_interaction::filters::interchain::filter_fingerprint`);
    ///       copy that pattern here. Multichain has no such mechanism yet.
    #[serde_as(as = "Option<StringWithSeparator<CommaSeparator, u64>>")]
    pub multichain_filter: Option<Vec<u64>>,
    /// DEPRECATED: use `STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID`.
    ///
    /// Still honoured, and treated as exactly that value. Setting both to
    /// different values is a startup error.
    pub interchain_primary_id: Option<u64>,
    /// Read filter applied to the interchain indexer DB in `Interchain` mode.
    /// Mirrors the indexer's own read API parameters, so a stats deployment and
    /// the equivalent API request return the same subset.
    ///
    /// Changing any of these values (or `interchain_primary_id`) is detected
    /// automatically: the filter fingerprint stored in
    /// `chart_data.min_blockscout_block` forces a clear-and-rebuild of every
    /// interchain chart on the next update.
    pub interchain_filter: InterchainFilterSettings,
    /// Base URL of the interchain indexer's HTTP API, used in `Interchain` mode
    /// to read `GET /api/v1/status/indexing`. When a `(bridge, chain)` pair
    /// relevant to the configured interchain filter is still catching up, every
    /// interchain chart is recomputed from the indexer's current earliest data
    /// instead of only moving forward, so history the indexer backfills is
    /// picked up. Optional: with no URL the check is disabled and charts still
    /// pick up history that extends backwards, but not interior gaps. Ignored
    /// outside `Interchain` mode.
    pub interchain_indexer_api_url: Option<url::Url>,
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
    pub swagger_path: PathBuf,
    /// Linked secondary stats settings. A client is created only when [`LinkedStatsSettings::base_url`]
    /// is set; otherwise linked forwarding is disabled.
    ///
    /// Chaining linked services is technically allowed, but should be avoided unless
    /// there is a strong operational reason for it.
    pub linked_stats: LinkedStatsSettings,
    pub api_keys: HashMap<String, String>,

    pub server: ServerSettings,
    pub metrics: MetricsSettings,
    pub jaeger: JaegerSettings,
    pub tracing: TracingSettings,
}

/// The interchain read filter, as the operator configures it.
///
/// Every field mirrors an `interchain-indexer` read-API parameter of the same
/// name, including its default, so that a stats deployment and the equivalent API
/// request describe the same subset of rows. Only meaningful in
/// [`Mode::Interchain`]; setting any field in another mode is a startup error.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct InterchainFilterSettings {
    /// Focal chain. Equivalent to the indexer API's `home_chain_id`.
    /// With a counterparty set, keeps
    /// `(src = home AND dst IN cp) OR (dst = home AND src IN cp)`;
    /// alone, keeps `src = home OR dst = home`.
    pub home_chain_id: Option<u64>,
    /// Counterparties of the focal chain, comma-separated (e.g. `10,137`).
    /// Note: **without** `home_chain_id` this means "both endpoints inside the
    /// set" (`src IN set AND dst IN set`), a conjunction — not a focal OR.
    #[serde_as(as = "Option<StringWithSeparator<CommaSeparator, u64>>")]
    pub counterparty_chain_ids: Option<Vec<u64>>,
    /// Additional restriction on the source chain, comma-separated. Applied as a
    /// separate AND term, never folded into the focal OR.
    #[serde_as(as = "Option<StringWithSeparator<CommaSeparator, u64>>")]
    pub src_chain_ids: Option<Vec<u64>>,
    /// Additional restriction on the destination chain, comma-separated. Applied
    /// as a separate AND term. A message with no known destination is excluded by
    /// this term, because `dst IN (...)` is NULL for a NULL destination.
    #[serde_as(as = "Option<StringWithSeparator<CommaSeparator, u64>>")]
    pub dst_chain_ids: Option<Vec<u64>>,
    /// Restriction on the indexer's bridge ids, comma-separated.
    #[serde_as(as = "Option<StringWithSeparator<CommaSeparator, u32>>")]
    pub bridge_ids: Option<Vec<u32>>,
    /// Mirrors the indexer API flag of the same name, including its default.
    /// `false` keeps only rows the row's own bridge could have fully observed —
    /// in particular it excludes every message with no known destination. Set
    /// `true` to count everything the indexer stored.
    pub include_unindexed_chains: bool,
}

// Written out rather than derived — and yes, `derive(Default)` would produce the
// identical impl. `include_unindexed_chains: false` is a *restrictive* default
// copied from the indexer API, not an incidental "zero value", and the one place a
// reader looks to confirm that is this impl. The generated env-docs table takes its
// default column from here too.
#[allow(clippy::derivable_impls)]
impl Default for InterchainFilterSettings {
    fn default() -> Self {
        Self {
            home_chain_id: None,
            counterparty_chain_ids: None,
            src_chain_ids: None,
            dst_chain_ids: None,
            bridge_ids: None,
            include_unindexed_chains: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LinkedStatsSettings {
    #[serde(default)]
    pub base_url: Option<url::Url>,
    #[serde(default = "default_linked_stats_timeout")]
    pub timeout: u64,
    /// Requested hop budget for linked requests. Values above the hard cap are truncated.
    #[serde(default = "default_linked_stats_max_hops")]
    pub max_hops: u32,
}

pub const LINKED_STATS_MAX_HOPS_HARD_CAP: u32 = 4;

fn default_linked_stats_timeout() -> u64 {
    3_000
}

fn default_linked_stats_max_hops() -> u32 {
    1
}

impl Default for LinkedStatsSettings {
    fn default() -> Self {
        Self {
            base_url: None,
            timeout: default_linked_stats_timeout(),
            max_hops: default_linked_stats_max_hops(),
        }
    }
}

impl LinkedStatsSettings {
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout)
    }

    pub fn max_hops(&self) -> u32 {
        self.max_hops.min(LINKED_STATS_MAX_HOPS_HARD_CAP)
    }
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
            mode: Mode::Blockscout,
            default_schedule: Schedule::from_str("0 0 1 * * * *").unwrap(),
            force_update_on_start: Some(false),
            concurrent_start_updates: 3,
            limits: Default::default(),
            conditional_start: Default::default(),
            charts_config: PathBuf::from_str("config/blockscout_instance/charts.json").unwrap(),
            layout_config: PathBuf::from_str("config/blockscout_instance/layout.json").unwrap(),
            update_groups_config: PathBuf::from_str(
                "config/blockscout_instance/update_groups.json",
            )
            .unwrap(),
            swagger_path: default_swagger_path(),
            linked_stats: LinkedStatsSettings::default(),
            blockscout_db_url: Default::default(),
            indexer_db_url: Default::default(),
            second_indexer_db_url: Default::default(),
            blockscout_api_url: None,
            ignore_blockscout_api_absence: false,
            disable_internal_transactions: false,
            enable_all_arbitrum: false,
            enable_all_op_stack: false,
            enable_all_eip_7702: false,
            enable_all_filecoin: false,
            multichain_filter: Default::default(),
            interchain_primary_id: Default::default(),
            interchain_filter: Default::default(),
            interchain_indexer_api_url: None,
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

        // an entry is disabled based on the chart it actually serves — its
        // `implementation` when remapped, its own name otherwise — so that
        // a remapped entry serving an internal-transactions-dependent chart
        // is caught, and a mere name collision with one is not
        let mut unserved = to_disable.clone();
        for (name, settings) in charts.lines.iter_mut().chain(charts.counters.iter_mut()) {
            let served_chart = settings.implementation.as_deref().unwrap_or(name);
            if to_disable.contains(served_chart) {
                settings.enabled = false;
                unserved.remove(served_chart);
            }
        }
        // a leftover name means the binary registers an
        // internal-transactions-dependent chart that no config entry serves,
        // so there was nothing to disable
        if !unserved.is_empty() {
            debug!(
                "Could not disable internal transactions related charts \
                {unserved:?}: not served by any config entry. \
                This should not be a problem for running the service.",
            );
        }
    }
}

/// Finds the entry served under `id` in either config section. Chart ids are
/// unique across counters and line charts *among enabled entries* only —
/// `RuntimeSetup::build_charts_info` drops disabled entries before its
/// collision check — so a duplicate id with a disabled side reaches here and
/// the `lines` probe below wins. That preference is not something an operator
/// can predict from the config alone, so the ambiguous case is warned about
/// (naming both sides and their `enabled` states) instead of being resolved
/// silently.
fn find_chart_settings_mut<'a>(
    charts: &'a mut config::charts::Config<AllChartSettings>,
    id: &str,
) -> Option<&'a mut AllChartSettings> {
    match (charts.lines.get_mut(id), charts.counters.get_mut(id)) {
        (Some(line_settings), Some(counter_settings)) => {
            warn!(
                "Chart id '{id}' is served by an entry in both config sections: \
                line charts (enabled: {}) and counters (enabled: {}). \
                Chart ids must be unique across both sections; the line chart \
                entry is selected here, which may not be the intended one. \
                Rename or remove one of the two entries.",
                line_settings.enabled, counter_settings.enabled,
            );
            Some(line_settings)
        }
        (Some(settings), _) => Some(settings),
        (_, Some(settings)) => Some(settings),
        _ => None,
    }
}

fn enable_charts(
    to_enable: &[&str],
    charts: &mut config::charts::Config<AllChartSettings>,
    charts_name_for_logs: &str,
) {
    for enable_key in to_enable {
        let Some(settings) = find_chart_settings_mut(charts, enable_key) else {
            warn!(
                "Could not enable '{charts_name_for_logs}'-specific chart {enable_key}: \
                chart not found in settings. \
                This should not be a problem for running the service.",
            );
            continue;
        };
        settings.enabled = true;
    }
}

/// Sets `implementation` on the entry served under `public_id`, unless the
/// operator already configured one — an explicit operator-provided mapping
/// wins over a flag. Looks the entry up in both sections via
/// [`find_chart_settings_mut`]; warns and continues when neither holds it.
fn set_default_implementation(
    charts: &mut config::charts::Config<AllChartSettings>,
    public_id: &str,
    implementation_id: String,
    charts_name_for_logs: &str,
) {
    let Some(settings) = find_chart_settings_mut(charts, public_id) else {
        warn!(
            "Could not remap '{charts_name_for_logs}'-specific chart {public_id}: \
            chart not found in settings. \
            Nothing will be served under this public id; the service starts without it.",
        );
        return;
    };
    if settings.implementation.is_none() {
        settings.implementation = Some(implementation_id);
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

/// Switches the whole Filecoin API surface with one flag: enables
/// `filecoinChainFeesGrowth` under its own id and force-enables the public
/// `txnsFee` id, serving it with the `filecoinNewChainFees` implementation
/// (chain-wide REV-style fees) unless an explicit `implementation` is already
/// configured. Likewise force-enables the public `txnsFee24h` id, serving it
/// with the `filecoinChainFees24h` implementation (the 24h burn + tips
/// counter) unless an explicit `implementation` is already configured.
/// Under this flag, `filecoinNewChainFees` and `filecoinChainFees24h` are
/// never exposed as public chart ids. Both have `layout.json` slots and may
/// be explicitly enabled standalone only while this flag is off: the flag
/// makes each of them a remap target, and a remap target that is also
/// enabled under its own id is rejected at startup
/// (`RuntimeSetup::validate_implementation_mappings`). The intermediate
/// charts (`burnActorBalance`,
/// `fevmFeeTips`) stay disabled — hidden from the API — and are updated
/// transitively as dependencies of the public charts.
pub fn handle_enable_all_filecoin(
    enable_all: bool,
    charts: &mut config::charts::Config<AllChartSettings>,
) {
    if enable_all {
        // force-enabling `txnsFee`/`txnsFee24h` keeps the single-env-var
        // promise even for configs that disable the entry; `enable_charts`
        // warns and continues if an entry is absent
        enable_charts(
            &[
                FilecoinChainFeesGrowth::key().name(),
                TxnsFee::key().name(),
                TxnsFee24h::key().name(),
            ],
            charts,
            "filecoin",
        );
        // config keys are camelCase at this point (post config load)
        set_default_implementation(
            charts,
            TxnsFee::key().name(),
            FilecoinNewChainFees::key().into_name(),
            "filecoin",
        );
        set_default_implementation(
            charts,
            TxnsFee24h::key().name(),
            FilecoinChainFees24h::key().into_name(),
            "filecoin",
        );
    }
}

pub fn apply_zetachain_cctx_mode_settings(
    settings: &mut Settings,
    charts: &mut config::charts::Config<AllChartSettings>,
) {
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
    let check_enabled = &mut settings
        .conditional_start
        .zetachain_indexed_until_today
        .enabled;
    if check_enabled.is_none() {
        *check_enabled = Some(true);
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

/// Apply settings for Interchain mode (separate indexer DB, no blockscout API).
pub fn apply_interchain_mode_settings(settings: &mut Settings) {
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
    pub zetachain_indexed_until_today: ToggleableOptionalCheck,
    /// Interchain mode only, and **off by default**.
    ///
    /// When enabled, the service waits before its first chart update until every
    /// `(bridge, chain)` pair relevant to `interchain_filter` reports at least
    /// this much catch-up progress, as a **ratio** in `0.0..=1.0` (`0.95` = 95%),
    /// matching `blocks_ratio` and `internal_transactions_ratio` above. Progress
    /// is aggregated as the **minimum** over the relevant pairs, so the slowest
    /// pair decides.
    ///
    /// The indexer's `catchup_progress_percent` is the share of the configured
    /// block range that has been **scanned**, *not* the share of data that is
    /// present: a range scanned but failed downstream still counts as scanned.
    /// Read `0.95` as "95% of blocks scanned", never as "95% of the data is
    /// there".
    ///
    /// While the service waits, `/api/v1/update-status` reports
    /// `WAITING_FOR_STARTING_CONDITION` and no chart has any data. A pair with no
    /// checkpoint row yet, or whose realtime cursor is still below its configured
    /// start block, reports 0% — so a single stuck pair holds the whole service
    /// at "no data" indefinitely. That is why this is opt-in; leaving it disabled
    /// is the supported configuration, and the per-cycle catch-up check
    /// (`interchain_indexer_api_url`) then keeps chart history tracking the
    /// indexer without ever withholding data.
    ///
    /// This threshold gates the **start**; the per-cycle catch-up verdict governs
    /// **rebuilds after** the start. The two are independent, and they resolve an
    /// unreachable API in opposite directions on purpose — see
    /// `check_interchain_status`.
    ///
    /// **Requires `interchain_indexer_api_url`.** Enabling this without a URL is
    /// a startup error, not a warning: an ungated start that looks gated is worse
    /// than a refusal to boot.
    pub interchain_catchup_min_progress: ToggleableThreshold,
    pub check_period_secs: u32,
}

impl Default for StartConditionSettings {
    fn default() -> Self {
        Self {
            // in some networks it's always almost 1
            blocks_ratio: ToggleableThreshold::default(),
            internal_transactions_ratio: ToggleableThreshold::default(),
            user_ops_past_indexing_finished: ToggleableCheck::default(),
            zetachain_indexed_until_today: ToggleableOptionalCheck::default(),
            // written out on purpose. `ToggleableThreshold::default()` is
            // `Self::enabled(0.98)` (see below) — *enabled* and 0.98 — so
            // deriving this default would turn the check on for every existing
            // interchain deployment, none of which has the indexer API URL
            // set, so they would all hit the enabled-without-source `bail!` on
            // upgrade. The value matches `blocks_ratio`'s 0.98 deliberately:
            // there is no reason to carry two different-but-close defaults, and
            // it means `__ENABLED=true` alone yields a sensible threshold rather
            // than the useless 0.0 a bare `disabled()` would give.
            interchain_catchup_min_progress: ToggleableThreshold::disabled().set_threshold(0.98),
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
        self.zetachain_indexed_until_today.enabled.unwrap_or(false)
    }
    /// Whether the interchain catch-up start check participates in the wait.
    ///
    /// `validate_interchain_filter` `bail!`s when this is enabled without
    /// `interchain_indexer_api_url`, so by the time the aggregator runs,
    /// `interchain_checks_enabled() == true` implies a configured source. That is
    /// why this takes no argument even though `decisions.md` Q11 words the
    /// requirement as "must also require the API URL to be configured":
    /// `StartConditionSettings` cannot see the URL, and normalising the
    /// invariant at startup is cleaner than giving one of four sibling helpers a
    /// different signature.
    pub fn interchain_checks_enabled(&self) -> bool {
        self.interchain_catchup_min_progress.enabled
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

impl Default for ToggleableCheck {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ToggleableOptionalCheck {
    pub enabled: Option<bool>,
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

    /// Under Q11 the default *value* is aligned with `blocks_ratio` at `0.98`,
    /// so the threshold alone no longer distinguishes the correct default from
    /// the bug this test exists to catch (turning the check on by default). The
    /// **pair** must be asserted.
    #[test]
    fn interchain_catchup_gate_is_disabled_by_default() {
        assert_eq!(
            StartConditionSettings::default().interchain_catchup_min_progress,
            ToggleableThreshold::disabled().set_threshold(0.98)
        );
    }

    #[test]
    fn interchain_catchup_gate_stays_disabled_with_no_envs() {
        check_envs_parsed_to(
            "START_SETTINGS",
            std::collections::HashMap::new(),
            StartConditionSettings::default(),
        )
        .unwrap();
    }

    /// Pins the toggleable-threshold "partial env sets `enabled`" behaviour
    /// (parity with `STATS__CONDITIONAL_START__BLOCKS_RATIO__*`, not a bug):
    /// setting only `…__ENABLED=true` yields the written-out `0.98` threshold,
    /// and setting only `…__THRESHOLD` alone flips `enabled` to `true`.
    #[test]
    fn interchain_catchup_gate_partial_env_inherits_the_toggleable_default() {
        check_envs_parsed_to(
            "START_SETTINGS",
            [(
                "START_SETTINGS__INTERCHAIN_CATCHUP_MIN_PROGRESS__ENABLED".to_owned(),
                "true".to_owned(),
            )]
            .into(),
            StartConditionSettings {
                interchain_catchup_min_progress: ToggleableThreshold::enabled(0.98),
                ..StartConditionSettings::default()
            },
        )
        .unwrap();

        check_envs_parsed_to(
            "START_SETTINGS",
            [(
                "START_SETTINGS__INTERCHAIN_CATCHUP_MIN_PROGRESS__THRESHOLD".to_owned(),
                "0.95".to_owned(),
            )]
            .into(),
            StartConditionSettings {
                interchain_catchup_min_progress: ToggleableThreshold::enabled(0.95),
                ..StartConditionSettings::default()
            },
        )
        .unwrap();
    }

    #[test]
    fn interchain_checks_enabled_follows_the_threshold_toggle() {
        assert!(
            !StartConditionSettings {
                interchain_catchup_min_progress: ToggleableThreshold::disabled(),
                ..StartConditionSettings::default()
            }
            .interchain_checks_enabled()
        );
        assert!(
            StartConditionSettings {
                interchain_catchup_min_progress: ToggleableThreshold::enabled(0.9),
                ..StartConditionSettings::default()
            }
            .interchain_checks_enabled()
        );
    }

    fn interchain_filter_envs_parse_to(
        env_values: impl IntoIterator<Item = (&'static str, &'static str)>,
        expected: InterchainFilterSettings,
    ) -> anyhow::Result<()> {
        check_envs_parsed_to(
            "INTERCHAIN_FILTER",
            env_values
                .into_iter()
                .map(|(k, v)| (format!("INTERCHAIN_FILTER__{k}"), v.to_owned()))
                .collect(),
            expected,
        )
    }

    #[test]
    fn interchain_filter_defaults_to_no_dimensions_and_the_horizon_enabled() {
        interchain_filter_envs_parse_to([], InterchainFilterSettings::default()).unwrap();
        assert!(!InterchainFilterSettings::default().include_unindexed_chains);
    }

    #[test]
    fn interchain_filter_parses_csv_lists_and_scalars() {
        interchain_filter_envs_parse_to(
            [
                ("HOME_CHAIN_ID", "1"),
                ("COUNTERPARTY_CHAIN_IDS", "10,137"),
                ("SRC_CHAIN_IDS", "1"),
                ("DST_CHAIN_IDS", "137,10,137"),
                ("BRIDGE_IDS", "7,3"),
                ("INCLUDE_UNINDEXED_CHAINS", "true"),
            ],
            InterchainFilterSettings {
                home_chain_id: Some(1),
                counterparty_chain_ids: Some(vec![10, 137]),
                src_chain_ids: Some(vec![1]),
                // parsing is verbatim: de-duplication and sorting happen in
                // `build_interchain_filter_config`, not here
                dst_chain_ids: Some(vec![137, 10, 137]),
                bridge_ids: Some(vec![7, 3]),
                include_unindexed_chains: true,
            },
        )
        .unwrap();
    }

    #[test]
    fn interchain_filter_rejects_out_of_range_ids() {
        let too_big_for_u64 = format!("{}0", u64::MAX);
        check_envs_parsed_to(
            "INTERCHAIN_FILTER",
            [(
                "INTERCHAIN_FILTER__COUNTERPARTY_CHAIN_IDS".to_owned(),
                too_big_for_u64,
            )]
            .into(),
            InterchainFilterSettings::default(),
        )
        .unwrap_err();
        check_envs_parsed_to(
            "INTERCHAIN_FILTER",
            [(
                "INTERCHAIN_FILTER__BRIDGE_IDS".to_owned(),
                format!("{}", u64::from(u32::MAX) + 1),
            )]
            .into(),
            InterchainFilterSettings::default(),
        )
        .unwrap_err();
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

    #[test]
    fn disable_internal_transactions_follows_implementation_remap() {
        let mut settings = Settings::default();
        // `txnsFee` itself is not internal-transactions-dependent but is
        // remapped onto `contractsGrowth`, which is; the entry named
        // `contractsGrowth` serves an independent chart instead
        let mut charts = config::charts::Config {
            counters: Default::default(),
            lines: [
                (
                    TxnsFee::key().into_name(),
                    config::types::AllChartSettings {
                        enabled: true,
                        implementation: Some(ContractsGrowth::key().into_name()),
                        ..Default::default()
                    },
                ),
                (
                    ContractsGrowth::key().into_name(),
                    config::types::AllChartSettings {
                        enabled: true,
                        implementation: Some(FilecoinNewChainFees::key().into_name()),
                        ..Default::default()
                    },
                ),
            ]
            .into_iter()
            .collect(),
        };

        settings.disable_internal_transactions = true;
        handle_disable_internal_transactions(
            settings.disable_internal_transactions,
            &mut settings.conditional_start,
            &mut charts,
        );

        // the entry serving the internal-transactions-dependent chart must
        // be disabled even though its own name is not in the dependent set
        assert!(!charts.lines[TxnsFee::key().name()].enabled);
        // ...while a name collision with a dependent chart must not disable
        // an entry that actually serves an independent one
        assert!(charts.lines[ContractsGrowth::key().name()].enabled);
    }

    // post-load config: entries are keyed via `key().name()` (camelCase),
    // matching the state after config load
    fn filecoin_charts_config(
        txns_fee_settings: config::types::AllChartSettings,
    ) -> config::charts::Config<AllChartSettings> {
        let disabled = config::types::AllChartSettings {
            enabled: false,
            ..Default::default()
        };
        config::charts::Config {
            counters: [
                (FilecoinChainFees24h::key().into_name(), disabled.clone()),
                (TxnsFee24h::key().into_name(), txns_fee_settings.clone()),
            ]
            .into_iter()
            .collect(),
            lines: [
                (FilecoinChainFeesGrowth::key().into_name(), disabled.clone()),
                (FilecoinNewChainFees::key().into_name(), disabled),
                (TxnsFee::key().into_name(), txns_fee_settings),
            ]
            .into_iter()
            .collect(),
        }
    }

    /// The one property the four Filecoin flag tests cannot express: the
    /// remap helper locates the public entry **regardless of which section
    /// holds it**. This is the assertion that fails on the classic
    /// copy-paste mistake the helper exists to prevent — a new remap block
    /// hard-coding the wrong map (lines vs counters), which at HEAD before
    /// the helper would silently set nothing.
    #[test]
    fn set_default_implementation_finds_entry_in_either_section() {
        let mut charts = filecoin_charts_config(config::types::AllChartSettings::default());

        // a line-chart id and a counter id, through the same call
        set_default_implementation(
            &mut charts,
            TxnsFee::key().name(),
            "someImpl".into(),
            "test",
        );
        set_default_implementation(
            &mut charts,
            TxnsFee24h::key().name(),
            "someOtherImpl".into(),
            "test",
        );
        assert_eq!(
            charts.lines[TxnsFee::key().name()]
                .implementation
                .as_deref(),
            Some("someImpl")
        );
        assert_eq!(
            charts.counters[TxnsFee24h::key().name()]
                .implementation
                .as_deref(),
            Some("someOtherImpl")
        );

        // an id present in neither section warns and continues — no panic,
        // no change
        set_default_implementation(&mut charts, "absentChart", "ignored".into(), "test");
    }

    #[test]
    fn enable_all_filecoin_enables_and_remaps_charts() {
        let mut charts = filecoin_charts_config(config::types::AllChartSettings {
            enabled: true,
            ..Default::default()
        });

        handle_enable_all_filecoin(true, &mut charts);

        assert!(charts.lines[FilecoinChainFeesGrowth::key().name()].enabled);
        let txns_fee = &charts.lines[TxnsFee::key().name()];
        assert!(txns_fee.enabled);
        assert_eq!(
            txns_fee.implementation,
            Some(FilecoinNewChainFees::key().into_name())
        );
        // the implementation must never become a public id
        assert!(!charts.lines[FilecoinNewChainFees::key().name()].enabled);

        let txns_fee_24h = &charts.counters[TxnsFee24h::key().name()];
        assert!(txns_fee_24h.enabled);
        assert_eq!(
            txns_fee_24h.implementation,
            Some(FilecoinChainFees24h::key().into_name())
        );
        // the implementation must never become a public id
        assert!(!charts.counters[FilecoinChainFees24h::key().name()].enabled);
    }

    #[test]
    fn enable_all_filecoin_force_enables_disabled_txns_fee() {
        let mut charts = filecoin_charts_config(config::types::AllChartSettings {
            enabled: false,
            ..Default::default()
        });

        handle_enable_all_filecoin(true, &mut charts);

        let txns_fee = &charts.lines[TxnsFee::key().name()];
        assert!(txns_fee.enabled);
        assert_eq!(
            txns_fee.implementation,
            Some(FilecoinNewChainFees::key().into_name())
        );

        let txns_fee_24h = &charts.counters[TxnsFee24h::key().name()];
        assert!(txns_fee_24h.enabled);
        assert_eq!(
            txns_fee_24h.implementation,
            Some(FilecoinChainFees24h::key().into_name())
        );
    }

    #[test]
    fn enable_all_filecoin_does_not_overwrite_explicit_implementation() {
        let mut charts = filecoin_charts_config(config::types::AllChartSettings {
            enabled: false,
            implementation: Some("someOtherImplementation".to_owned()),
            ..Default::default()
        });

        handle_enable_all_filecoin(true, &mut charts);

        let txns_fee = &charts.lines[TxnsFee::key().name()];
        // enablement is still forced, the operator-provided mapping wins
        assert!(txns_fee.enabled);
        assert_eq!(
            txns_fee.implementation.as_deref(),
            Some("someOtherImplementation")
        );

        let txns_fee_24h = &charts.counters[TxnsFee24h::key().name()];
        // enablement is still forced, the operator-provided mapping wins
        assert!(txns_fee_24h.enabled);
        assert_eq!(
            txns_fee_24h.implementation.as_deref(),
            Some("someOtherImplementation")
        );
    }

    #[test]
    fn disabled_enable_all_filecoin_changes_nothing() {
        let mut charts = filecoin_charts_config(config::types::AllChartSettings {
            enabled: true,
            ..Default::default()
        });

        handle_enable_all_filecoin(false, &mut charts);

        assert!(!charts.lines[FilecoinChainFeesGrowth::key().name()].enabled);
        assert!(!charts.lines[FilecoinNewChainFees::key().name()].enabled);
        let txns_fee = &charts.lines[TxnsFee::key().name()];
        assert!(txns_fee.enabled);
        assert_eq!(txns_fee.implementation, None);

        assert!(!charts.counters[FilecoinChainFees24h::key().name()].enabled);
        let txns_fee_24h = &charts.counters[TxnsFee24h::key().name()];
        assert!(txns_fee_24h.enabled);
        assert_eq!(txns_fee_24h.implementation, None);
    }

    // a duplicate id with one side disabled survives
    // `RuntimeSetup::build_charts_info`, so this preference is observable in a
    // running service; pin that the line chart entry is the one acted upon
    #[test]
    fn duplicate_id_across_sections_resolves_to_the_line_chart_entry() {
        let disabled = config::types::AllChartSettings::default();
        let id = "some_duplicated_id".to_owned();
        let mut charts = config::charts::Config {
            counters: [(id.clone(), disabled.clone())].into_iter().collect(),
            lines: [(id.clone(), disabled.clone())].into_iter().collect(),
        };

        enable_charts(&[&id], &mut charts, "test");

        assert!(charts.lines[&id].enabled);
        assert!(!charts.counters[&id].enabled);
    }

    #[test]
    fn linked_stats_without_base_url_deserializes() {
        let settings: LinkedStatsSettings =
            serde_json::from_str(r#"{"timeout": 10}"#).expect("valid config should deserialize");
        assert!(settings.base_url.is_none());
        assert_eq!(settings.timeout, 10);
    }

    #[test]
    fn linked_stats_empty_object_deserializes_to_defaults() {
        let settings: LinkedStatsSettings =
            serde_json::from_str(r#"{}"#).expect("empty linked_stats should deserialize");
        assert!(settings.base_url.is_none());
        assert_eq!(settings.timeout, 3_000);
        assert_eq!(settings.max_hops, 1);
    }

    #[test]
    fn linked_stats_defaults_timeout_and_max_hops_when_base_url_is_set() {
        let settings: LinkedStatsSettings =
            serde_json::from_str(r#"{"base_url":"http://example.com"}"#)
                .expect("valid linked_stats config should deserialize");

        assert_eq!(
            settings.base_url.as_ref().unwrap().as_str(),
            "http://example.com/"
        );
        assert_eq!(settings.timeout, 3_000);
        assert_eq!(settings.max_hops, 1);
        assert_eq!(settings.max_hops(), 1);
    }

    #[test]
    fn linked_stats_max_hops_is_capped_to_hard_limit() {
        let settings: LinkedStatsSettings =
            serde_json::from_str(r#"{"base_url":"http://example.com","max_hops":100}"#)
                .expect("valid linked_stats config should deserialize");

        assert_eq!(settings.max_hops, 100);
        assert_eq!(settings.max_hops(), LINKED_STATS_MAX_HOPS_HARD_CAP);
    }
}
