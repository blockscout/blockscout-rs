// SPDX-License-Identifier: LicenseRef-Blockscout

//! Setting up the runtime according to provided configs.
//!
//! ## Adding new charts
//!
//! 1. Create charts & update group(-s) (if necessary) according to
//!    documentation in [`stats::update_group`] (steps 1-2).
//! 2. If new groups were added:
//!    2.1. Add new update groups to [`RuntimeSetup::all_update_groups`] (if any)
//!    2.2. Configure the group update schedule in `update_groups.json` config
//! 3. Add the new charts to `charts.json` and `layout.json` (if needed)
//! 4. If some were added in the previous step, also consider adding the
//!    new charts to integration tests (`tests` folder).
//!

use crate::{
    ReadService,
    config::{
        self,
        types::{AllChartSettings, EnabledChartSettings, LineChartCategory},
    },
};
use cron::Schedule;
use itertools::Itertools;
use stats::{
    ChartKey, ChartObject, IndexingStatus, ResolutionKind,
    entity::sea_orm_active_enums::ChartType,
    query_dispatch::ChartTypeSpecifics,
    update_group::{ArcUpdateGroup, SyncUpdateGroup},
};
use std::{
    collections::{BTreeMap, HashMap, HashSet, btree_map::Entry, hash_map},
    sync::Arc,
};
use tokio::sync::Mutex;

/// Chart enabled by config
#[derive(Debug, Clone)]
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

    /// Returns a vector of `ChartKey`'s for all resolutions of the chart.
    pub fn get_keys(&self) -> Vec<ChartKey> {
        self.resolutions
            .iter()
            .map(|(res, entry)| ChartKey::new(entry.name.clone(), *res))
            .collect()
    }
}

#[derive(Debug, Clone)]
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

impl UpdateGroupEntry {
    pub fn should_skip_update(&self) -> bool {
        let should = self.enabled_members.is_empty();
        if should {
            tracing::info!(
                "update group {} does not have enabled members; should skip update",
                self.group.name()
            );
        }
        should
    }
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
                // an entry with `implementation` set is served with the handles
                // of the referenced chart, under the entry's own (public) name;
                // the mapping is checked by `validate_implementation_mappings`
                let internal_name = settings.implementation.unwrap_or_else(|| name.clone());
                let mut enabled_resolutions_properties = HashMap::new();
                for (resolution, resolution_setting) in settings.resolutions.into_list() {
                    let key = ChartKey::new(internal_name.clone(), resolution);
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

    /// Validate the `implementation` mappings of enabled entries, with errors
    /// naming both sides of a broken mapping.
    ///
    /// The matching loop in [`Self::charts_info_from_settings`] enforces the
    /// same invariants mechanically (each registered chart is consumed at most
    /// once), but failures there surface as a generic "non-existent
    /// charts+resolutions" error that never names the mapped entry.
    ///
    /// Both sections are checked in one pass because implementations are
    /// claimed from the shared registry map. Unknown-name and wrong-type
    /// checks must precede the resolution check: the latter derives the
    /// implementation's available resolutions, which look empty for a
    /// misspelled or wrong-type implementation.
    fn validate_implementation_mappings(
        counters_settings: &BTreeMap<String, AllChartSettings>,
        line_charts_settings: &BTreeMap<String, AllChartSettings>,
        all_members: &BTreeMap<ChartKey, ChartObject>,
    ) -> anyhow::Result<()> {
        let member_types: HashMap<&str, ChartType> = all_members
            .iter()
            .map(|(key, object)| (key.name(), object.type_specifics.as_chart_type()))
            .collect();
        let sections = [
            (ChartType::Counter, counters_settings),
            (ChartType::Line, line_charts_settings),
        ];
        // implementation name -> public entry that claimed it
        let mut claims: HashMap<&str, &str> = HashMap::new();
        for (entry_type, settings) in sections {
            for (public_name, entry_settings) in settings.iter() {
                // `implementation` on a disabled entry is inert, consistent
                // with the `into_enabled` gate of the matching loop
                if !entry_settings.enabled {
                    continue;
                }
                let Some(implementation) = entry_settings.implementation.as_deref() else {
                    continue;
                };
                if implementation == public_name {
                    anyhow::bail!(
                        "chart '{public_name}': `implementation` must reference another \
                        chart, not the chart itself"
                    );
                }
                let Some(implementation_type) = member_types.get(implementation) else {
                    anyhow::bail!(
                        "chart '{public_name}': `implementation` references unknown \
                        chart '{implementation}'"
                    );
                };
                if *implementation_type != entry_type {
                    anyhow::bail!(
                        "chart '{public_name}' is a {entry_type:?} chart, but its \
                        `implementation` '{implementation}' is a {implementation_type:?} chart"
                    );
                }
                if let Some(also_claimed_by) = claims.insert(implementation, public_name) {
                    anyhow::bail!(
                        "charts '{also_claimed_by}' and '{public_name}' both reference \
                        '{implementation}' in `implementation`"
                    );
                }
                let implementation_own_entry_enabled = counters_settings
                    .get(implementation)
                    .or_else(|| line_charts_settings.get(implementation))
                    .is_some_and(|settings| settings.enabled);
                if implementation_own_entry_enabled {
                    anyhow::bail!(
                        "chart '{public_name}' references '{implementation}' in \
                        `implementation`, but chart '{implementation}' is also enabled \
                        under its own name; disable one of them"
                    );
                }
                // resolution compatibility is checked only for explicitly
                // requested resolutions; a default `None` ("enable if present")
                // on a missing resolution stays silently skipped, exactly as
                // for non-remapped charts
                let implementation_resolutions: HashSet<ResolutionKind> = all_members
                    .keys()
                    .filter(|key| key.name() == implementation)
                    .map(|key| *key.resolution())
                    .collect();
                if implementation_resolutions.is_empty() {
                    // defer to the more specific errors above
                    continue;
                }
                if let Err(missing) = entry_settings
                    .resolutions
                    .clone()
                    .into_enabled(&implementation_resolutions)
                {
                    anyhow::bail!(
                        "chart '{public_name}' explicitly requests resolutions {missing:?} \
                        that its `implementation` '{implementation}' does not have"
                    );
                }
            }
        }
        Ok(())
    }

    fn all_charts_info_from_settings(
        counters_settings: BTreeMap<String, AllChartSettings>,
        line_charts_settings: BTreeMap<String, AllChartSettings>,
    ) -> anyhow::Result<AllChartsInfo> {
        let mut available_resolutions = Self::all_members();
        Self::validate_implementation_mappings(
            &counters_settings,
            &line_charts_settings,
            &available_resolutions,
        )?;
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
                Err(anyhow::anyhow!(
                    "non-existent charts+resolutions are present in settings: {unknown_charts:?}",
                ))
            }
        }
    }

    fn build_charts_info(
        charts_config: config::charts::Config<AllChartSettings>,
    ) -> anyhow::Result<BTreeMap<String, EnabledChartEntry>> {
        let AllChartsInfo {
            counters,
            line_charts,
        } = Self::all_charts_info_from_settings(charts_config.counters, charts_config.lines)?;

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
            ReadService::main_page_multichain_charts(),
            ReadService::main_page_interchain_charts(),
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
        use stats::{update_groups::*, update_groups_interchain::*, update_groups_multichain::*};

        vec![
            // actual singletons
            Arc::new(ActiveAccountsGroup),
            Arc::new(ActiveAccountsWeeklyGroup),
            Arc::new(ActiveBundlersGroup),
            Arc::new(ActivePaymastersGroup),
            Arc::new(ActiveAccountAbstractionWalletsGroup),
            Arc::new(AverageBlockTimeGroup),
            Arc::new(CompletedTxnsGroup),
            Arc::new(PendingTxns30mGroup),
            Arc::new(TotalAddressesGroup),
            Arc::new(TotalBlocksGroup),
            Arc::new(TotalTokensGroup),
            Arc::new(TotalTxnsGroup),
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
            Arc::new(NewAccountAbstractionWalletsGroup),
            Arc::new(NewContractsGroup),
            Arc::new(NewTxnsGroup),
            Arc::new(NewUserOpsGroup),
            Arc::new(NewEip7702AuthsGroup),
            Arc::new(NewVerifiedContractsGroup),
            Arc::new(NativeCoinHoldersGrowthGroup),
            Arc::new(NewNativeCoinTransfersGroup),
            Arc::new(TxnsStats24hGroup),
            Arc::new(NewBuilderAccountsGroup),
            Arc::new(VerifiedContractsPageGroup),
            // zetachain cross chain txns
            Arc::new(ZetachainCrossChainTxnsGroup),
            // filecoin chain fees
            Arc::new(FilecoinChainFeesGroup),
            // multichain: singletons
            Arc::new(TotalInteropMessagesGroup),
            Arc::new(TotalInteropTransfersGroup),
            Arc::new(TotalMultichainAddressesGroup),
            Arc::new(TotalMultichainTxnsGroup),
            Arc::new(YesterdayTxnsMultichainGroup),
            // multichain: groups
            Arc::new(NewTxnsMultichainGroup),
            Arc::new(NewTxnsMultichainWindowGroup),
            Arc::new(TxnsGrowthMultichainGroup),
            Arc::new(AccountsGrowthMultichainGroup),
            // interchain
            Arc::new(TotalInterchainMessagesGroup),
            Arc::new(TotalInterchainMessagesReceivedGroup),
            Arc::new(TotalInterchainMessagesSentGroup),
            Arc::new(NewMessagesInterchainGroup),
            Arc::new(NewMessagesSentInterchainGroup),
            Arc::new(NewMessagesReceivedInterchainGroup),
            Arc::new(TotalInterchainTransfersGroup),
            Arc::new(TotalInterchainTransfersReceivedGroup),
            Arc::new(TotalInterchainTransfersSentGroup),
            Arc::new(TotalInterchainTransferUsersGroup),
            Arc::new(NewTransfersInterchainGroup),
            Arc::new(NewTransfersSentInterchainGroup),
            Arc::new(NewTransfersReceivedInterchainGroup),
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
            // They have their own group + doesn't make sense to update
            // the dependency if `networkUtilization` is disabled
            (
                "NewBlocksGroup",
                vec![
                    "averageGasLimit_DAY",
                    "averageGasLimit_WEEK",
                    "averageGasLimit_MONTH",
                    "averageGasLimit_YEAR",
                ],
            ),
            // Same logic as above
            ("TotalBlocksGroup", vec!["totalTxns_DAY"]),
        ]
        .map(|(group_name, allowed_missing)| {
            (
                group_name.to_owned(),
                allowed_missing.into_iter().map(|s| s.to_string()).collect(),
            )
        })
        .into_iter()
        // combine sets for the same key (group name)
        .fold(HashMap::new(), |mut acc, (group_name, allowed_missing)| {
            match acc.entry(group_name) {
                hash_map::Entry::Vacant(v) => {
                    v.insert(allowed_missing);
                }
                hash_map::Entry::Occupied(mut o) => {
                    o.get_mut().extend(allowed_missing);
                }
            }
            acc
        });

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
                    getting stalled: {:?}",
                    missing_members
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

        // key by `ChartKey` (internal names) so that scheduling follows the
        // implementation identity of every enabled entry, even when it is
        // served under a different public name
        let enabled_keys: HashSet<ChartKey> = charts_info
            .values()
            .flat_map(|entry| entry.get_keys())
            .collect();

        for (name, group) in update_groups {
            let update_schedule = groups_config
                .schedules
                .get(&name)
                .map(|e| e.update_schedule.clone());
            let enabled_members = group
                .list_charts()
                .into_iter()
                .filter(|m| enabled_keys.contains(&m.properties.key))
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

    pub fn enabled_chart_keys(&self) -> Vec<ChartKey> {
        self.charts_info
            .values()
            .flat_map(|entry| entry.get_keys())
            .collect()
    }

    /// Recursive indexing status requirements for all group members.
    ///
    /// 'Recursive' means that their dependencies' requirements are also
    /// considered.
    ///
    /// Both enabled and disabled memebers are included.
    pub fn all_members_indexing_status_requirements() -> BTreeMap<ChartKey, IndexingStatus> {
        Self::all_update_groups()
            .into_iter()
            .flat_map(|g| {
                g.list_charts()
                    .into_iter()
                    .map(|c| {
                        let key = c.properties.key;
                        let enabled_key = &HashSet::from([key.clone()]);
                        (key, g.dependency_indexing_status_requirement(enabled_key))
                    })
                    .collect_vec()
            })
            .collect()
    }

    /// Recursive indexing status requirements for all enabled group members.
    /// See [`Self::all_members_indexing_status_requirements`] for details.
    pub fn all_enabled_members_indexing_status_requirements(
        &self,
    ) -> BTreeMap<ChartKey, IndexingStatus> {
        let enabled_charts: HashSet<_> = self.enabled_chart_keys().into_iter().collect();
        Self::all_members_indexing_status_requirements()
            .into_iter()
            .filter(|(chart, _req)| enabled_charts.contains(chart))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::ResolutionsSettings;
    use stats::{
        ChartProperties,
        counters::TotalTxns,
        lines::{AverageTxnFee, FilecoinNewChainFees, NewTxnsWindow, TxnsFee},
    };

    // these tests operate on post-load configs, where chart ids are already
    // camelCase, so they are always spelled via `key().name()`

    fn chart_settings(enabled: bool, implementation: Option<String>) -> AllChartSettings {
        AllChartSettings {
            enabled,
            implementation,
            ..Default::default()
        }
    }

    fn line_charts_config(
        lines: impl IntoIterator<Item = (String, AllChartSettings)>,
    ) -> config::charts::Config<AllChartSettings> {
        config::charts::Config {
            counters: BTreeMap::new(),
            lines: lines.into_iter().collect(),
        }
    }

    fn runtime_setup(
        charts: config::charts::Config<AllChartSettings>,
    ) -> anyhow::Result<RuntimeSetup> {
        RuntimeSetup::new(
            charts,
            config::layout::Config::default(),
            config::update_groups::Config::default(),
        )
    }

    fn startup_error(charts: config::charts::Config<AllChartSettings>) -> String {
        match runtime_setup(charts) {
            Ok(_) => panic!("invalid mapping must fail startup"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn remapped_chart_is_served_under_public_name_with_implementation_handles() {
        let public_name = TxnsFee::key().into_name();
        let implementation_name = FilecoinNewChainFees::key().into_name();
        // distinct sentinels on both sides pin the metadata source: the
        // public entry's settings must win for all three fields
        let public_settings = AllChartSettings {
            enabled: true,
            title: "PUBLIC_TITLE".to_owned(),
            description: "PUBLIC_DESC".to_owned(),
            units: Some("PUBLIC_UNITS".to_owned()),
            implementation: Some(implementation_name.clone()),
            ..Default::default()
        };
        let implementation_settings = AllChartSettings {
            enabled: false,
            title: "IMPL_TITLE".to_owned(),
            description: "IMPL_DESC".to_owned(),
            units: Some("IMPL_UNITS".to_owned()),
            ..Default::default()
        };
        let setup = runtime_setup(line_charts_config([
            (public_name.clone(), public_settings),
            (implementation_name.clone(), implementation_settings),
        ]))
        .expect("valid remapping must not fail startup");

        let entry = setup
            .charts_info
            .get(&public_name)
            .expect("public name must be present in charts info");
        assert!(
            !setup.charts_info.contains_key(&implementation_name),
            "implementation must not be served under its own name"
        );

        assert_eq!(entry.settings.title, "PUBLIC_TITLE");
        assert_eq!(entry.settings.description, "PUBLIC_DESC");
        assert_eq!(entry.settings.units.as_deref(), Some("PUBLIC_UNITS"));

        // per-resolution handles carry the implementation's internal name
        let expected_keys: HashSet<ChartKey> = [
            ResolutionKind::Day,
            ResolutionKind::Week,
            ResolutionKind::Month,
            ResolutionKind::Year,
        ]
        .map(|resolution| ChartKey::new(implementation_name.clone(), resolution))
        .into();
        for resolution_entry in entry.resolutions.values() {
            assert_eq!(resolution_entry.name, implementation_name);
        }
        let keys: HashSet<ChartKey> = entry.get_keys().into_iter().collect();
        assert_eq!(keys, expected_keys);

        // update scheduling follows the implementation identity (Phase 2):
        // the implementation's group updates the entry, the replaced chart's
        // group has nothing to do
        assert_eq!(
            setup.update_groups["FilecoinChainFeesGroup"].enabled_members,
            expected_keys
        );
        assert!(
            setup.update_groups["TxnsFeeGroup"]
                .enabled_members
                .is_empty()
        );
    }

    #[test]
    fn unknown_implementation_fails_startup() {
        let public_name = TxnsFee::key().into_name();
        let err = startup_error(line_charts_config([(
            public_name.clone(),
            chart_settings(true, Some("definitelyUnknownChart".to_owned())),
        )]));
        assert!(err.contains(&public_name), "{err}");
        assert!(err.contains("definitelyUnknownChart"), "{err}");
    }

    #[test]
    fn implementation_chart_type_mismatch_fails_startup() {
        let public_name = TxnsFee::key().into_name();
        let implementation_name = TotalTxns::key().into_name();
        let err = startup_error(line_charts_config([(
            public_name.clone(),
            chart_settings(true, Some(implementation_name.clone())),
        )]));
        assert!(err.contains(&public_name), "{err}");
        assert!(err.contains(&implementation_name), "{err}");
        assert!(err.contains("Counter"), "{err}");
    }

    #[test]
    fn implementation_claimed_by_two_mapped_entries_fails_startup() {
        let implementation_name = FilecoinNewChainFees::key().into_name();
        let err = startup_error(line_charts_config([
            (
                TxnsFee::key().into_name(),
                chart_settings(true, Some(implementation_name.clone())),
            ),
            (
                AverageTxnFee::key().into_name(),
                chart_settings(true, Some(implementation_name.clone())),
            ),
        ]));
        assert!(err.contains(&TxnsFee::key().into_name()), "{err}");
        assert!(err.contains(&AverageTxnFee::key().into_name()), "{err}");
        assert!(err.contains(&implementation_name), "{err}");
    }

    #[test]
    fn implementation_enabled_under_own_name_fails_startup() {
        let public_name = TxnsFee::key().into_name();
        let implementation_name = FilecoinNewChainFees::key().into_name();
        let err = startup_error(line_charts_config([
            (
                public_name.clone(),
                chart_settings(true, Some(implementation_name.clone())),
            ),
            (implementation_name.clone(), chart_settings(true, None)),
        ]));
        assert!(err.contains(&public_name), "{err}");
        assert!(err.contains(&implementation_name), "{err}");
        assert!(err.contains("also enabled"), "{err}");
    }

    #[test]
    fn self_referencing_implementation_fails_startup() {
        let public_name = TxnsFee::key().into_name();
        let err = startup_error(line_charts_config([(
            public_name.clone(),
            chart_settings(true, Some(public_name.clone())),
        )]));
        assert!(err.contains(&public_name), "{err}");
        assert!(err.contains("the chart itself"), "{err}");
    }

    #[test]
    fn explicitly_requested_resolution_missing_from_implementation_fails_startup() {
        // `newTxnsWindow` has only the day resolution
        let public_name = TxnsFee::key().into_name();
        let implementation_name = NewTxnsWindow::key().into_name();
        let mut settings = chart_settings(true, Some(implementation_name.clone()));
        settings.resolutions = ResolutionsSettings {
            week: Some(true),
            ..Default::default()
        };
        let err = startup_error(line_charts_config([(public_name.clone(), settings)]));
        assert!(err.contains(&public_name), "{err}");
        assert!(err.contains(&implementation_name), "{err}");
        assert!(err.contains("Week"), "{err}");
    }

    #[test]
    fn resolution_missing_from_implementation_with_default_setting_is_skipped() {
        // parity with non-remapped behavior: a `None` resolution setting means
        // "enable if present", so nothing fails and only day is enabled
        let public_name = TxnsFee::key().into_name();
        let implementation_name = NewTxnsWindow::key().into_name();
        let setup = runtime_setup(line_charts_config([(
            public_name.clone(),
            chart_settings(true, Some(implementation_name.clone())),
        )]))
        .expect("default resolution settings must not fail startup");
        let entry = &setup.charts_info[&public_name];
        assert_eq!(
            entry.resolutions.keys().collect_vec(),
            vec![&ResolutionKind::Day]
        );
        assert_eq!(
            entry.resolutions[&ResolutionKind::Day].name,
            implementation_name
        );
    }

    #[test]
    fn unknown_implementation_takes_precedence_over_resolution_mismatch() {
        let mut settings = chart_settings(true, Some("definitelyUnknownChart".to_owned()));
        settings.resolutions = ResolutionsSettings {
            week: Some(true),
            ..Default::default()
        };
        let err = startup_error(line_charts_config([(TxnsFee::key().into_name(), settings)]));
        assert!(err.contains("unknown chart"), "{err}");
    }

    #[test]
    fn type_mismatch_takes_precedence_over_resolution_mismatch() {
        let mut settings = chart_settings(true, Some(TotalTxns::key().into_name()));
        settings.resolutions = ResolutionsSettings {
            week: Some(true),
            ..Default::default()
        };
        let err = startup_error(line_charts_config([(TxnsFee::key().into_name(), settings)]));
        assert!(err.contains("Counter"), "{err}");
    }

    #[test]
    fn implementation_on_disabled_entry_is_inert() {
        let setup = runtime_setup(line_charts_config([(
            TxnsFee::key().into_name(),
            chart_settings(false, Some(FilecoinNewChainFees::key().into_name())),
        )]))
        .expect("disabled entry with `implementation` must not fail startup");
        assert!(setup.charts_info.is_empty());
    }
}
