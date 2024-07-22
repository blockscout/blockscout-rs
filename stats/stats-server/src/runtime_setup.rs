use crate::config::{
    self,
    types::{AllChartSettings, EnabledChartSettings, LineChartCategory},
};
use cron::Schedule;
use itertools::Itertools;
use stats::{
    entity::sea_orm_active_enums::ChartType,
    update_group::{ArcUpdateGroup, SyncUpdateGroup},
    ChartKey, ChartPropertiesObject, ResolutionKind,
};
use std::{
    collections::{btree_map::Entry, BTreeMap, HashMap, HashSet},
    hash::Hash,
    sync::Arc,
};
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct EnabledChartEntry {
    pub settings: EnabledChartSettings,
    /// Static information presented as dynamic object
    pub enabled_resolutions: HashMap<ResolutionKind, ChartPropertiesObject>,
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
        let enabled_charts_config = Self::remove_disabled_charts(charts);
        let enabled_counters = Self::enabled_keys(enabled_charts_config.counters.clone());
        let enabled_counters = new_set_check_duplicates(enabled_counters)
            .map_err(|id| anyhow::anyhow!("encountered same id twice: {}", id))?;
        if enabled_counters
            .iter()
            .find(|a| a.resolution() != &ResolutionKind::Day)
            .is_some()
        {
            return Err(anyhow::anyhow!(
                "Resolutions other than 'Day' are not supported for counters"
            ));
        }

        let enabled_lines = Self::enabled_keys(enabled_charts_config.lines.clone());
        let enabled_lines = new_set_check_duplicates(enabled_lines)
            .map_err(|id| anyhow::anyhow!("encountered same id twice: {}", id))?;

        let mut counters_unknown = enabled_counters.clone();
        let mut lines_unknown = enabled_lines.clone();
        let settings = Self::extract_united_chart_settings(&enabled_charts_config);
        let enabled_resolutions: BTreeMap<ChartKey, ChartPropertiesObject> = Self::all_members()
            .into_iter()
            .filter(|(key, chart)| match chart.chart_type {
                ChartType::Counter => counters_unknown.remove(key),
                ChartType::Line => lines_unknown.remove(key),
            })
            .filter_map(|(key, chart)| {
                settings.get(key.name()).and_then(|settings| {
                    if settings.resolutions.is_enabled(key.resolution()) {
                        Some((key, chart))
                    } else {
                        None
                    }
                })
            })
            .collect();

        let charts_info: BTreeMap<String, EnabledChartEntry> =
            Self::combine_resolutions(enabled_resolutions, &settings)?;

        if !counters_unknown.is_empty() || !lines_unknown.is_empty() {
            return Err(anyhow::anyhow!(
                "found unknown charts: {:?}",
                counters_unknown.union(&lines_unknown)
            ));
        }

        let update_groups = Self::init_update_groups(update_groups, &charts_info)?;

        Ok(Self {
            lines_layout: layout.line_chart_categories,
            update_groups,
            charts_info,
        })
    }

    fn enabled_keys(charts: BTreeMap<String, EnabledChartSettings>) -> Vec<ChartKey> {
        charts
            .into_iter()
            .flat_map(|(name, settings)| {
                settings
                    .resolutions
                    .into_enabled()
                    .into_iter()
                    .map(move |r| ChartKey::new(name.clone(), r))
            })
            .collect()
    }

    fn combine_resolutions(
        enabled_resolutions: BTreeMap<ChartKey, ChartPropertiesObject>,
        settings: &BTreeMap<String, EnabledChartSettings>,
    ) -> anyhow::Result<BTreeMap<String, EnabledChartEntry>> {
        let mut result: BTreeMap<String, EnabledChartEntry> = BTreeMap::new();
        for (key, properties) in enabled_resolutions {
            match result.entry(key.name().to_string()) {
                Entry::Vacant(v) => {
                    let settings = settings
                        .get(key.name())
                        .ok_or(anyhow::anyhow!(
                            // must've checked presence of the settings when checking status
                            // of resolution (on/off)
                            "No settings for an enabled resolution. Likely a bug in the code"
                        ))?
                        .clone();
                    v.insert(EnabledChartEntry {
                        settings,
                        enabled_resolutions: HashMap::from([(*key.resolution(), properties)]),
                    });
                }
                Entry::Occupied(mut o) => {
                    if let Some(_) = o
                        .get_mut()
                        .enabled_resolutions
                        .insert(*key.resolution(), properties)
                    {
                        // in `enabled_resolutions`
                        panic!("Duplicate keys in BTreeMap");
                    };
                }
            }
        }
        Ok(result)
    }

    fn remove_disabled_charts(
        charts: config::charts::Config<AllChartSettings>,
    ) -> config::charts::Config<EnabledChartSettings> {
        // no need to filter `update_schedule` list because extra names
        // will be ignored anyway
        let counters = charts
            .counters
            .into_iter()
            .filter_map(|(id, settings)| Some((id, EnabledChartSettings::from_all(settings)?)))
            .collect();
        let lines = charts
            .lines
            .into_iter()
            .filter_map(|(id, settings)| Some((id, EnabledChartSettings::from_all(settings)?)))
            .collect();
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
            .map(|(id, settings)| (id.clone(), settings.clone()))
            .chain(
                config
                    .lines
                    .iter()
                    .map(|(id, settings)| (id.clone(), settings.clone())),
            )
            .collect()
    }

    fn all_update_groups() -> Vec<ArcUpdateGroup> {
        use stats::update_groups::*;

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
            Arc::new(TotalTokensGroup),
            // complex groups
            Arc::new(NewAccountsGroup),
            Arc::new(NewContractsGroup),
            Arc::new(NewTxnsGroup),
            Arc::new(NewVerifiedContractsGroup),
            Arc::new(NativeCoinHoldersGrowthGroup),
            Arc::new(NewNativeCoinTransfersGroup),
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
        groups_config: config::update_groups::Config,
        charts_info: &BTreeMap<String, EnabledChartEntry>,
    ) -> anyhow::Result<BTreeMap<String, UpdateGroupEntry>> {
        let update_groups = Self::all_update_groups();
        let dep_mutexes = Self::create_all_dependencies_mutexes(update_groups.clone());
        let update_groups = Self::construct_group_map(update_groups)?;
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
                        .get(m.key.name())
                        .is_some_and(|a| a.enabled_resolutions.contains_key(m.key.resolution()))
                })
                .map(|m| m.key)
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
    fn all_members() -> BTreeMap<ChartKey, ChartPropertiesObject> {
        let members_with_duplicates = Self::all_update_groups()
            .into_iter()
            .flat_map(|g| g.list_charts())
            .collect_vec();
        let mut members = BTreeMap::new();
        for member in members_with_duplicates {
            match members.entry(member.key.clone()) {
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
                    assert_eq!(o.get(), &member, "duplicate member key '{}'", o.get().key);
                }
            }
        }
        members
    }
}
