use crate::{
    config::{
        self,
        charts::LinesInfo,
        types::{
            AllChartSettings, CounterInfo, EnabledChartSettings, LineChartCategory, LineChartInfo,
        },
    },
    update_groups,
};
use cron::Schedule;
use itertools::Itertools;
use stats::{
    entity::sea_orm_active_enums::ChartType,
    update_group::{ArcUpdateGroup, SyncUpdateGroup},
    ChartDynamic,
};
use std::{
    collections::{btree_map::Entry, BTreeMap, HashSet},
    hash::Hash,
    sync::Arc,
};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct EnabledChartEntry {
    pub settings: EnabledChartSettings,
    /// Static information presented as dynamic object
    pub static_info: ChartDynamic,
}

/// Everything needed to operate update group
#[derive(Clone)]
pub struct UpdateGroupEntry {
    /// Custom schedule for this update group
    pub update_schedule: Option<Schedule>,
    /// Handle for operating the group
    pub group: SyncUpdateGroup,
    /// Members that are enabled both
    /// - in the charts config
    /// - in update group config (=not disabled there)
    pub enabled_members: HashSet<String>,
}

pub struct RuntimeSetup {
    pub lines_layout: LinesInfo<EnabledChartSettings>,
    pub update_groups: BTreeMap<String, UpdateGroupEntry>,
    pub charts_info: BTreeMap<String, EnabledChartEntry>,
}

fn new_set_check_duplicates<T: Hash + Eq, I: IntoIterator<Item = T>>(
    iter: I,
) -> Result<HashSet<T>, T> {
    let mut result = HashSet::default();
    for item in iter.into_iter() {
        let prev_value = result.replace(item);
        if let Some(prev_value) = prev_value {
            return Err(prev_value);
        }
    }
    Ok(result)
}

impl RuntimeSetup {
    pub fn new(
        charts: config::charts::Config<AllChartSettings>,
        update_schedule: config::update_schedule::Config,
    ) -> anyhow::Result<Self> {
        Self::validated_and_initialized(charts, update_schedule)
    }

    fn validated_and_initialized(
        charts: config::charts::Config<AllChartSettings>,
        update_schedule: config::update_schedule::Config,
    ) -> anyhow::Result<Self> {
        let enabled_charts_config = Self::remove_disabled_charts(charts);
        let enabled_counters = enabled_charts_config
            .counters
            .iter()
            .map(|counter| counter.id.clone());
        let enabled_counters = new_set_check_duplicates(enabled_counters)
            .map_err(|id| anyhow::anyhow!("encountered same id twice: {}", id))?;

        let enabled_lines = enabled_charts_config
            .lines
            .0
            .iter()
            .flat_map(|section| section.charts.iter().map(|chart| chart.id.clone()));
        let enabled_lines = new_set_check_duplicates(enabled_lines)
            .map_err(|id| anyhow::anyhow!("encountered same id twice: {}", id))?;

        let mut counters_unknown = enabled_counters.clone();
        let mut lines_unknown = enabled_lines.clone();
        let settings = Self::extract_united_chart_settings(&enabled_charts_config);
        let charts_info: BTreeMap<String, EnabledChartEntry> = Self::all_member_charts()
            .into_iter()
            .filter(|(name, chart)| match chart.chart_type {
                ChartType::Counter => counters_unknown.remove(name),
                ChartType::Line => lines_unknown.remove(name),
            })
            .filter_map(|(name, chart)| {
                settings.get(&name).map(|settings| {
                    let info = EnabledChartEntry {
                        settings: settings.to_owned(),
                        static_info: chart,
                    };
                    (name, info.clone())
                })
            })
            .collect();

        if !counters_unknown.is_empty() || !lines_unknown.is_empty() {
            return Err(anyhow::anyhow!(
                "found unknown charts: {:?}",
                counters_unknown.union(&lines_unknown)
            ));
        }

        let update_groups = Self::init_update_groups(update_schedule)?;

        Ok(Self {
            lines_layout: enabled_charts_config.lines,
            update_groups,
            charts_info,
        })
    }

    fn remove_disabled_charts(
        charts: config::charts::Config<AllChartSettings>,
    ) -> config::charts::Config<EnabledChartSettings> {
        // no need to filter `update_schedule` list because extra names
        // will be ignored anyway
        let counters = charts
            .counters
            .into_iter()
            .filter_map(|info| {
                Some(CounterInfo::<EnabledChartSettings> {
                    id: info.id,
                    settings: EnabledChartSettings::from_all(info.settings)?,
                })
            })
            .collect();
        let lines = charts
            .lines
            .0
            .into_iter()
            .map(|sec| LineChartCategory {
                id: sec.id,
                title: sec.title,
                charts: sec
                    .charts
                    .into_iter()
                    .filter_map(|info| {
                        Some(LineChartInfo::<EnabledChartSettings> {
                            id: info.id,
                            settings: EnabledChartSettings::from_all(info.settings)?,
                        })
                    })
                    .collect(),
            })
            .filter(|sec| !sec.charts.is_empty())
            .collect::<Vec<_>>()
            .into();
        config::charts::Config { counters, lines }
    }

    /// Get settings for both counters and line charts in single data structure.
    /// Assumes that config is valid.
    fn extract_united_chart_settings(
        config: &config::charts::Config<EnabledChartSettings>,
    ) -> BTreeMap<String, EnabledChartSettings> {
        config
            .counters
            .iter()
            .map(|counter| (counter.id.clone(), counter.settings.clone()))
            .chain(config.lines.0.iter().flat_map(|section| {
                section
                    .charts
                    .iter()
                    .map(|chart| (chart.id.clone(), chart.settings.clone()))
            }))
            .collect()
    }

    fn all_update_groups() -> Vec<ArcUpdateGroup> {
        use update_groups::*;
        vec![
            // singletons
            Arc::new(ActiveAccountsGroup),
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
            Arc::new(AverageBlockTimeGroup),
            Arc::new(CompletedTxnsGroup),
            Arc::new(TotalAddressesGroup),
            Arc::new(TotalBlocksGroup),
            Arc::new(TotalContractsGroup),
            Arc::new(TotalTokensGroup),
            // complex groups
            Arc::new(AccountsIncrease),
            Arc::new(ContractsIncrease),
            Arc::new(TransactionsIncrease),
            Arc::new(VerifiedContracts),
            Arc::new(NativeCoinHolders),
            Arc::new(NativeCoinTransfers),
        ]
    }

    fn create_all_dependencies_mutexes(
        groups: impl IntoIterator<Item = ArcUpdateGroup>,
    ) -> BTreeMap<String, Arc<Mutex<()>>> {
        let mut mutexes = BTreeMap::new();
        for g in groups.into_iter() {
            let dependencies = g.list_dependency_mutex_ids();
            for d in dependencies {
                if !mutexes.contains_key(d) {
                    mutexes.insert(d.to_owned(), Arc::new(Mutex::new(())));
                }
            }
        }
        mutexes
    }

    /// Returns more user-friendly errors
    fn verify_schedule_config(
        update_groups: &BTreeMap<String, ArcUpdateGroup>,
        schedule_config: &config::update_schedule::Config,
    ) -> anyhow::Result<()> {
        let all_names: HashSet<_> = update_groups.keys().collect();
        let config_names: HashSet<_> = schedule_config.update_groups.keys().collect();
        let missing_group_settings = all_names.difference(&config_names).collect_vec();
        let unknown_group_settings = config_names.difference(&all_names).collect_vec();
        let mut error_messages = Vec::new();
        if !missing_group_settings.is_empty() {
            error_messages.push(format!("Missing groups: {:?}", missing_group_settings));
        }
        if !unknown_group_settings.is_empty() {
            error_messages.push(format!("Unknown groups: {:?}", unknown_group_settings))
        }
        if !error_messages.is_empty() {
            return Err(anyhow::anyhow!(
                "Failed to parse update schedule config: {}",
                error_messages.join(", ")
            ));
        }
        Ok(())
    }

    /// Check if some dependencies are not present in their respective groups
    /// and make corresponding warn
    fn warn_non_member_charts(groups: &BTreeMap<String, ArcUpdateGroup>) {
        for (name, group) in groups {
            let sync_dependencies: HashSet<String> = group
                .list_dependency_mutex_ids()
                .into_iter()
                .map(|s| s.to_owned())
                .collect();
            // we rely on the fact that:
            // chart names == their mutex ids
            let members: HashSet<String> =
                group.list_charts().into_iter().map(|c| c.name).collect();
            let missing_members = sync_dependencies.difference(&members).collect_vec();
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
    fn construct_group_map(
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
        schedule_config: config::update_schedule::Config,
    ) -> anyhow::Result<BTreeMap<String, UpdateGroupEntry>> {
        let update_groups = Self::all_update_groups();
        let dep_mutexes = Self::create_all_dependencies_mutexes(update_groups.clone());
        let update_groups = Self::construct_group_map(update_groups)?;
        let mut result = BTreeMap::new();

        // checks that all groups are present in config.
        Self::verify_schedule_config(&update_groups, &schedule_config)?;
        Self::warn_non_member_charts(&update_groups);

        for (name, group) in update_groups {
            let group_config = schedule_config
                .update_groups
                .get(&name)
                .expect("config verification did not catch missing group config");
            let disabled_members: HashSet<String> =
                group_config.ignore_charts.iter().cloned().collect();
            let enabled_members = group
                .list_charts()
                .into_iter()
                .map(|m| m.name)
                .filter(|member| !disabled_members.contains(member))
                .collect();
            let sync_group = SyncUpdateGroup::new(&dep_mutexes, group)?;
            result.insert(
                name,
                UpdateGroupEntry {
                    update_schedule: group_config.update_schedule.clone(),
                    group: sync_group,
                    enabled_members,
                },
            );
        }
        Ok(result)
    }

    /// List all charts that are members of at least 1 group.
    fn all_member_charts() -> BTreeMap<String, ChartDynamic> {
        let charts_with_duplicates = Self::all_update_groups()
            .into_iter()
            .flat_map(|g| g.list_charts())
            .collect_vec();
        let mut charts = BTreeMap::new();
        for chart in charts_with_duplicates {
            match charts.entry(chart.name.clone()) {
                Entry::Vacant(v) => {
                    v.insert(chart);
                }
                Entry::Occupied(o) => {
                    // note that it's still possible to have equal `ChartDynamic`s
                    // but different (static) types underneath.
                    //
                    // i.e. this check does not guarantee that same mutex will not be
                    // used for 2 different charts (although it shouldn't lead to logical
                    // errors)
                    assert_eq!(o.get(), &chart, "duplicate chart name '{}'", o.get().name);
                }
            }
        }
        charts
    }
}
