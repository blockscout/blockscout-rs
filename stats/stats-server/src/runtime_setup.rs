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
    sync::Arc,
};
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct EnabledChartEntry {
    pub settings: EnabledChartSettings,
    /// Static information presented as dynamic object
    pub enabled_resolutions: HashMap<ResolutionKind, EnabledResolutionEntry>,
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
                .enabled_resolutions
                .keys()
                .map(|r| String::from(*r))
                .collect_vec(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct EnabledResolutionEntry {
    pub name: String,
    pub chart_type: ChartType,
    pub missing_date_policy: stats::MissingDatePolicy,
    pub approximate_trailing_points: u64,
}

impl From<ChartPropertiesObject> for EnabledResolutionEntry {
    fn from(value: ChartPropertiesObject) -> Self {
        Self {
            name: value.name,
            chart_type: value.chart_type,
            missing_date_policy: value.missing_date_policy,
            approximate_trailing_points: value.approximate_trailing_points,
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
    pub update_groups: BTreeMap<String, UpdateGroupEntry>,
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
        let update_groups = Self::init_update_groups(update_groups, &charts_info)?;
        Ok(Self {
            lines_layout: layout.line_chart_categories,
            update_groups,
            charts_info,
        })
    }

    /// Build charts info from settings for one type of charts.
    ///
    /// `Err(Vec<ChartKey>)` - some unknown charts+resolutions are present in settings
    fn charts_info_from_settings(
        charts_settings: BTreeMap<String, AllChartSettings>,
        settings_chart_type: ChartType,
    ) -> Result<BTreeMap<String, EnabledChartEntry>, Vec<ChartKey>> {
        let available_resolutions = Self::all_members();
        let mut unknown_charts = vec![];

        let mut charts_info = BTreeMap::new();

        for (name, settings) in charts_settings {
            if let Some(enabled_chart_settings) = settings.clone().into_enabled() {
                let mut enabled_resolutions_properties = HashMap::new();
                for (resolution, resolution_setting) in settings.resolutions.into_list() {
                    let key = ChartKey::new(name.clone(), resolution);
                    let resolution_properties = available_resolutions
                        .get(&key)
                        .filter(|props| props.chart_type == settings_chart_type)
                        .cloned();
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
                        enabled_resolutions: enabled_resolutions_properties,
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

    fn build_charts_info(
        charts_config: config::charts::Config<AllChartSettings>,
    ) -> anyhow::Result<BTreeMap<String, EnabledChartEntry>> {
        let counters_info =
            Self::charts_info_from_settings(charts_config.counters, ChartType::Counter);
        let lines_info = Self::charts_info_from_settings(charts_config.lines, ChartType::Line);

        let (counters_info, lines_info) = match (counters_info, lines_info) {
            (Ok(c), Ok(l)) => (c, l),
            (counters_result, lines_result) => {
                let mut unknown_charts = vec![];
                if let Err(c) = counters_result {
                    unknown_charts.extend(c);
                }
                if let Err(l) = lines_result {
                    unknown_charts.extend(l);
                }
                return Err(anyhow::anyhow!(
                    "non-existent charts+resolutions are present in settings: {unknown_charts:?}",
                ));
            }
        };

        combine_disjoint_maps(counters_info, lines_info)
            .map_err(|duplicate_name| anyhow::anyhow!("duplicate chart name: {duplicate_name:?}",))
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
