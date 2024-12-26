//! Setting up the runtime according to provided configs.
//!
//! ## Adding new charts
//!
//! 1. Create charts & update group(-s) (if necessary) according to
//!     documentation in [`stats::update_group`] (steps 1-2).
//! 2. If new groups were added:
//!
//!     2.1. Add new update groups to [`RuntimeSetup::all_update_groups`] (if any)
//!
//!     2.2. Configure the group update schedule in `update_groups.json` config
//! 3. Add the new charts to `charts.json` and `layout.json` (if needed)
//! 4. If some were added in the previous step, also consider adding the
//!     new charts to integration tests (`tests` folder).
//!

use crate::{
    config::{
        self,
        types::{AllChartSettings, EnabledChartSettings, LineChartCategory},
    },
    ReadService,
};
use cron::Schedule;
use itertools::Itertools;
use stats::{
    entity::sea_orm_active_enums::ChartType,
    query_dispatch::ChartTypeSpecifics,
    update_group::{ArcUpdateGroup, SyncUpdateGroup},
    ChartKey, ChartObject, ResolutionKind,
};
use std::{
    collections::{btree_map::Entry, BTreeMap, HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct EnabledChartEntry {
    pub settings: EnabledChartSettings,
    /// Static information presented as dynamic object
    pub resolutions: HashMap<ResolutionKind, EnabledResolutionEntry>,
}

impl EnabledChartEntry {
    pub fn build_proto_line_chart_info(
        &self,
        id: String,
    ) -> stats_proto::blockscout::stats::v1::LineChartInfo {
        let settings = self.settings.clone();
        stats_proto::blockscout::stats::v1::LineChartInfo {
            id,
            title: settings.title,
            description: settings.description,
            units: settings.units,
            resolutions: self
                .resolutions
                .keys()
                .map(|r| String::from(*r))
                .collect_vec(),
        }
    }
}

#[derive(Debug)]
pub struct EnabledResolutionEntry {
    pub name: String,
    pub missing_date_policy: stats::MissingDatePolicy,
    pub approximate_trailing_points: u64,
    pub type_specifics: ChartTypeSpecifics,
}

impl From<ChartObject> for EnabledResolutionEntry {
    fn from(value: ChartObject) -> Self {
        let ChartObject {
            properties: props,
            type_specifics,
        } = value;
        Self {
            name: props.name,
            missing_date_policy: props.missing_date_policy,
            approximate_trailing_points: props.approximate_trailing_points,
            type_specifics,
        }
    }
}

/// Everything needed to operate update group
#[derive(Clone)]
pub struct UpdateGroupEntry {
    /// Custom schedule for this update group
    pub update_schedule: Option<Schedule>,
    /// Handle for operating the group
    pub group: SyncUpdateGroup,
    /// Members that are enabled in the charts config
    pub enabled_members: HashSet<ChartKey>,
}

pub struct RuntimeSetup {
    pub lines_layout: Vec<LineChartCategory>,
    pub counters_layout: Vec<String>,
    pub update_groups: BTreeMap<String, UpdateGroupEntry>,
    /// chart name -> entry
    pub charts_info: BTreeMap<String, EnabledChartEntry>,
}

/// Combine 2 disjoint (by key) maps into a single map.
///
/// `Err(k)` when maps are not disjoint (`k` - key present both in `a` and `b`).
fn combine_disjoint_maps<K: Ord + Clone, V>(
    mut a: BTreeMap<K, V>,
    b: BTreeMap<K, V>,
) -> Result<BTreeMap<K, V>, K> {
    for (k, v) in b {
        match a.entry(k) {
            Entry::Vacant(en) => en.insert(v),
            Entry::Occupied(en) => return Err(en.key().clone()),
        };
    }
    Ok(a)
}

struct AllChartsInfo {
    counters: BTreeMap<String, EnabledChartEntry>,
    line_charts: BTreeMap<String, EnabledChartEntry>,
}

impl RuntimeSetup {
    pub fn new(
        charts: config::charts::Config<AllChartSettings>,
        layout: config::layout::Config,
        update_groups: config::update_groups::Config,
    ) -> anyhow::Result<Self> {
        Self::validated_and_initialized(charts, layout, update_groups)
    }

    fn validated_and_initialized(
        charts: config::charts::Config<AllChartSettings>,
        layout: config::layout::Config,
        update_groups: config::update_groups::Config,
    ) -> anyhow::Result<Self> {
        let charts_info = Self::build_charts_info(charts)?;
        Self::check_all_enabled_charts_have_endpoints(charts_info.keys().collect(), &layout);
        let update_groups = Self::init_update_groups(update_groups, &charts_info)?;
        Ok(Self {
            lines_layout: layout.line_chart_categories,
            counters_layout: layout.counters_order,
            update_groups,
            charts_info,
        })
    }

    /// Build charts info from settings for one type of charts.
    ///
    /// `Err(Vec<ChartKey>)` - some unknown charts+resolutions are present in settings
    fn charts_info_from_settings(
        available_resolutions: &mut BTreeMap<ChartKey, ChartObject>,
        charts_settings: BTreeMap<String, AllChartSettings>,
        settings_chart_type: ChartType,
    ) -> Result<BTreeMap<String, EnabledChartEntry>, Vec<ChartKey>> {
        let mut unknown_charts = vec![];

        let mut charts_info = BTreeMap::new();

        for (name, settings) in charts_settings {
            if let Some(enabled_chart_settings) = settings.clone().into_enabled() {
                let mut enabled_resolutions_properties = HashMap::new();
                for (resolution, resolution_setting) in settings.resolutions.into_list() {
                    let key = ChartKey::new(name.clone(), resolution);
                    let resolution_properties = match available_resolutions.entry(key.clone()) {
                        Entry::Occupied(o)
                            if o.get().type_specifics.as_chart_type() == settings_chart_type =>
                        {
                            Some(o.remove())
                        }
                        _ => None,
                    };
                    match (resolution_setting, resolution_properties) {
                        // enabled
                        (Some(true), Some(enabled_props)) | (None, Some(enabled_props)) => {
                            enabled_resolutions_properties
                                .insert(*key.resolution(), enabled_props.into());
                        }
                        // disabled, everything correct
                        (Some(false), Some(_)) | (None, None) => (),
                        // setting is set but chart is unknown
                        (Some(true), None) | (Some(false), None) => unknown_charts.push(key),
                    }
                }
                charts_info.insert(
                    name,
                    EnabledChartEntry {
                        settings: enabled_chart_settings,
                        resolutions: enabled_resolutions_properties,
                    },
                );
            }
        }

        if !unknown_charts.is_empty() {
            Err(unknown_charts)
        } else {
            Ok(charts_info)
        }
    }

    fn all_charts_info_from_settings(
        counters_settings: BTreeMap<String, AllChartSettings>,
        line_charts_settings: BTreeMap<String, AllChartSettings>,
    ) -> Result<AllChartsInfo, Vec<ChartKey>> {
        let mut available_resolutions = Self::all_members();
        let counters_info = Self::charts_info_from_settings(
            &mut available_resolutions,
            counters_settings,
            ChartType::Counter,
        );
        let lines_info = Self::charts_info_from_settings(
            &mut available_resolutions,
            line_charts_settings,
            ChartType::Line,
        );
        match (counters_info, lines_info) {
            (Ok(c), Ok(l)) => Ok(AllChartsInfo {
                counters: c,
                line_charts: l,
            }),
            (counters_result, lines_result) => {
                let mut unknown_charts = vec![];
                if let Err(c) = counters_result {
                    unknown_charts.extend(c);
                }
                if let Err(l) = lines_result {
                    unknown_charts.extend(l);
                }
                Err(unknown_charts)
            }
        }
    }

    fn build_charts_info(
        charts_config: config::charts::Config<AllChartSettings>,
    ) -> anyhow::Result<BTreeMap<String, EnabledChartEntry>> {
        let AllChartsInfo {
            counters,
            line_charts,
        } = Self::all_charts_info_from_settings(charts_config.counters, charts_config.lines)
            .map_err(|unknown_charts| {
                anyhow::anyhow!(
                    "non-existent charts+resolutions are present in settings: {unknown_charts:?}",
                )
            })?;

        combine_disjoint_maps(counters, line_charts)
            .map_err(|duplicate_name| anyhow::anyhow!("duplicate chart name: {duplicate_name:?}",))
    }

    /// Warns charts that are both enabled and will be updated,
    /// but which are not returned by any endpoint (making the updates
    /// very likely useless).
    ///
    /// In other words, any enabled chart should be accessible from
    /// the outside.
    fn check_all_enabled_charts_have_endpoints<'a>(
        mut enabled_names: HashSet<&'a String>,
        layout: &'a config::layout::Config,
    ) {
        // general stats handles
        for counter in &layout.counters_order {
            enabled_names.remove(&counter);
        }
        for line_chart in layout
            .line_chart_categories
            .iter()
            .flat_map(|cat| cat.charts_order.iter())
        {
            enabled_names.remove(&line_chart);
        }
        // pages
        let charts_in_pages = [
            ReadService::main_page_charts(),
            ReadService::contracts_page_charts(),
            ReadService::transactions_page_charts(),
        ]
        .concat();
        for chart in charts_in_pages {
            enabled_names.remove(&chart);
        }

        if !enabled_names.is_empty() {
            tracing::warn!(
                endpointless_charts =? enabled_names,
                "Some charts are updated but not returned \
                by any endpoint. This is likely a bug, please report it."
            );
        }
    }

    fn all_update_groups() -> Vec<ArcUpdateGroup> {
        use stats::update_groups::*;

        vec![
            // actual singletons
            Arc::new(ActiveAccountsGroup),
            Arc::new(AverageBlockTimeGroup),
            Arc::new(CompletedTxnsGroup),
            Arc::new(PendingTxnsGroup),
            Arc::new(TotalAddressesGroup),
            Arc::new(TotalBlocksGroup),
            Arc::new(TotalTokensGroup),
            Arc::new(TotalTxnsGroup),
            Arc::new(TotalOperationalTxnsGroup),
            Arc::new(YesterdayTxnsGroup),
            Arc::new(ActiveRecurringAccountsDailyRecurrence60DaysGroup),
            Arc::new(ActiveRecurringAccountsMonthlyRecurrence60DaysGroup),
            Arc::new(ActiveRecurringAccountsWeeklyRecurrence60DaysGroup),
            Arc::new(ActiveRecurringAccountsYearlyRecurrence60DaysGroup),
            Arc::new(ActiveRecurringAccountsDailyRecurrence90DaysGroup),
            Arc::new(ActiveRecurringAccountsMonthlyRecurrence90DaysGroup),
            Arc::new(ActiveRecurringAccountsWeeklyRecurrence90DaysGroup),
            Arc::new(ActiveRecurringAccountsYearlyRecurrence90DaysGroup),
            Arc::new(ActiveRecurringAccountsDailyRecurrence120DaysGroup),
            Arc::new(ActiveRecurringAccountsMonthlyRecurrence120DaysGroup),
            Arc::new(ActiveRecurringAccountsWeeklyRecurrence120DaysGroup),
            Arc::new(ActiveRecurringAccountsYearlyRecurrence120DaysGroup),
            Arc::new(NewTxnsWindowGroup),
            // singletons but not really (include all resolutions of the same chart)
            Arc::new(AverageBlockRewardsGroup),
            Arc::new(AverageBlockSizeGroup),
            Arc::new(AverageGasLimitGroup),
            Arc::new(AverageGasPriceGroup),
            Arc::new(AverageTxnFeeGroup),
            Arc::new(GasUsedGrowthGroup),
            Arc::new(NativeCoinSupplyGroup),
            Arc::new(NewBlocksGroup),
            Arc::new(TxnsFeeGroup),
            Arc::new(TxnsSuccessRateGroup),
            // complex groups
            Arc::new(NewAccountsGroup),
            Arc::new(NewContractsGroup),
            Arc::new(NewTxnsGroup),
            Arc::new(NewVerifiedContractsGroup),
            Arc::new(NativeCoinHoldersGrowthGroup),
            Arc::new(NewNativeCoinTransfersGroup),
            Arc::new(TxnsStats24hGroup),
            Arc::new(VerifiedContractsPageGroup),
        ]
    }

    fn create_all_dependencies_mutexes(
        groups: impl IntoIterator<Item = ArcUpdateGroup>,
    ) -> BTreeMap<String, Arc<Mutex<()>>> {
        let mut mutexes = BTreeMap::new();
        for g in groups.into_iter() {
            let dependencies = g.list_dependency_mutex_ids();
            for d in dependencies {
                if !mutexes.contains_key(&d) {
                    mutexes.insert(d.to_owned(), Arc::new(Mutex::new(())));
                }
            }
        }
        mutexes
    }

    /// Returns more user-friendly errors
    fn verify_groups_config(
        update_groups: &BTreeMap<String, ArcUpdateGroup>,
        update_groups_config: &config::update_groups::Config,
    ) -> anyhow::Result<()> {
        let all_names: HashSet<_> = update_groups.keys().collect();
        let config_names: HashSet<_> = update_groups_config.schedules.keys().collect();
        let unknown_group_settings = config_names.difference(&all_names).collect_vec();
        if !unknown_group_settings.is_empty() {
            return Err(anyhow::anyhow!(
                "Unknown groups in update groups config: {:?}",
                unknown_group_settings
            ));
        }
        Ok(())
    }

    /// Check if some dependencies are not present in their respective groups
    /// and make corresponding warn
    fn warn_non_member_charts(groups: &BTreeMap<String, ArcUpdateGroup>) {
        // Average charts have their 'weight' dependencies absent from
        // the group because it doesn't make sense to update the weights
        // if all averages are disabled (for some reason).
        //
        // The weights themselves (e.g. `newBlocks`) have their own groups
        // for this
        let missing_members_allowed: HashMap<String, HashSet<String>> = [
            // no `MONTH` because the month one is not stored in DB
            // (in other words, not a chart (in other words, doesn't have mutex))
            ("AverageBlockRewardsGroup", vec!["newBlockRewards_DAY"]),
            (
                "AverageBlockSizeGroup",
                vec!["newBlocks_MONTH", "newBlocks_DAY"],
            ),
            (
                "AverageGasLimitGroup",
                vec!["newBlocks_DAY", "newBlocks_MONTH"],
            ),
            ("AverageGasPriceGroup", vec!["newTxns_DAY", "newTxns_MONTH"]),
            ("AverageTxnFeeGroup", vec!["newTxns_DAY", "newTxns_MONTH"]),
            ("TxnsSuccessRateGroup", vec!["newTxns_DAY", "newTxns_MONTH"]),
            // total blocks and total txns have their own respective groups
            (
                "TotalOperationalTxnsGroup",
                vec!["totalBlocks_DAY", "totalTxns_DAY"],
            ),
            // the operational txns charts that depend on `newTxns_DAY` are
            // rarely turned on, also `newTxns_DAY` is not that expensive to
            // compute, therefore this solution is ok (to not introduce
            // more update groups if not necessary)
            ("NewBlocksGroup", vec!["newTxns_DAY"]),
        ]
        .map(|(group_name, allowed_missing)| {
            (
                group_name.to_owned(),
                allowed_missing.into_iter().map(|s| s.to_string()).collect(),
            )
        })
        .into();

        for (name, group) in groups {
            let sync_dependencies: HashSet<String> = group
                .list_dependency_mutex_ids()
                .into_iter()
                .map(|s| s.to_owned())
                .collect();
            // we rely on the fact that:
            // chart key == their mutex ids
            let members: HashSet<String> = group
                .list_charts()
                .into_iter()
                .map(|c| c.properties.key.as_string())
                .collect();
            let missing_members: HashSet<String> =
                sync_dependencies.difference(&members).cloned().collect();
            let empty_set = HashSet::new();
            let group_missing_allowed = missing_members_allowed.get(name).unwrap_or(&empty_set);
            let missing_members = missing_members
                .difference(group_missing_allowed)
                .collect_vec();
            if !missing_members.is_empty() {
                tracing::warn!(
                    update_group = name,
                    "Group has dependencies that are not members. In most scenarios it makes sense to include all dependencies, \
                    because all deps are updated with the group in any case. Turning off their 'parents' may lead to these members \
                    getting stalled: {:?}", missing_members
                )
            }
        }
    }

    /// Make map & check for duplicate names
    fn build_group_map(
        groups: Vec<ArcUpdateGroup>,
    ) -> anyhow::Result<BTreeMap<String, ArcUpdateGroup>> {
        let mut map = BTreeMap::new();
        for g in groups.into_iter() {
            if let Some(duplicate_named) = map.insert(g.name(), g) {
                return Err(anyhow::anyhow!(
                    "Non-unique group name: {:?}",
                    duplicate_named.name()
                ));
            }
        }
        Ok(map)
    }

    /// All initialization of update groups happens here
    fn init_update_groups(
        groups_config: config::update_groups::Config,
        charts_info: &BTreeMap<String, EnabledChartEntry>,
    ) -> anyhow::Result<BTreeMap<String, UpdateGroupEntry>> {
        let update_groups = Self::all_update_groups();
        let dep_mutexes = Self::create_all_dependencies_mutexes(update_groups.clone());
        let update_groups = Self::build_group_map(update_groups)?;
        let mut result = BTreeMap::new();

        // checks that all groups are present in config.
        Self::verify_groups_config(&update_groups, &groups_config)?;
        Self::warn_non_member_charts(&update_groups);

        for (name, group) in update_groups {
            let update_schedule = groups_config
                .schedules
                .get(&name)
                .map(|e| e.update_schedule.clone());
            let enabled_members = group
                .list_charts()
                .into_iter()
                .filter(|m| {
                    charts_info
                        .get(m.properties.key.name())
                        .is_some_and(|a| a.resolutions.contains_key(m.properties.key.resolution()))
                })
                .map(|m| m.properties.key)
                .collect();
            let sync_group = SyncUpdateGroup::new(&dep_mutexes, group)?;
            result.insert(
                name,
                UpdateGroupEntry {
                    update_schedule,
                    group: sync_group,
                    enabled_members,
                },
            );
        }
        Ok(result)
    }

    /// List all charts+resolutions that are members of at least 1 group.
    fn all_members() -> BTreeMap<ChartKey, ChartObject> {
        let members_with_duplicates = Self::all_update_groups()
            .into_iter()
            .flat_map(|g| g.list_charts())
            .collect_vec();
        let mut members = BTreeMap::new();
        for member in members_with_duplicates {
            match members.entry(member.properties.key.clone()) {
                Entry::Vacant(v) => {
                    v.insert(member);
                }
                Entry::Occupied(o) => {
                    // note that it's still possible to have equal `ChartPropertiesObject`s
                    // but different (static) types underneath.
                    //
                    // i.e. this check does not guarantee that same mutex will not be
                    // used for 2 different charts (although it shouldn't lead to logical
                    // errors)
                    assert_eq!(
                        o.get().properties,
                        member.properties,
                        "duplicate member key '{}'",
                        o.get().properties.key
                    );
                }
            }
        }
        members
    }
}
