use crate::{
    config::{
        self,
        chart_info::{AllChartSettings, CounterInfo, LineChartCategory, LineChartInfo},
        charts::LinesInfo,
    },
    groups,
};
use cron::Schedule;
use itertools::Itertools;
use serde::Deserialize;
use stats::{
    data_source::group::UpdateGroup, entity::sea_orm_active_enums::ChartType, ChartDynamic,
};
use std::{
    collections::{btree_map::Entry, BTreeMap, HashSet},
    hash::Hash,
    sync::Arc,
};

#[derive(Debug, Clone, Deserialize)]
pub struct EnabledChartSettings {
    pub title: String,
    pub description: String,
    pub units: Option<String>,
}

impl EnabledChartSettings {
    pub fn from_all(value: AllChartSettings) -> Option<Self> {
        if value.enabled {
            Some(EnabledChartSettings {
                units: value.units,
                title: value.title,
                description: value.description,
            })
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct EnabledChartEntry {
    pub settings: EnabledChartSettings,
    /// Static information presented as dynamic object
    pub static_info: ChartDynamic,
}

pub type ArcUpdateGroup = Arc<dyn for<'a> UpdateGroup + Send + Sync + 'static>;

#[derive(Clone)]
pub struct UpdateGroupEntry {
    /// Custom schedule for this update group
    pub update_schedule: Option<Schedule>,
    /// Handle for operating the group
    pub group: ArcUpdateGroup,
}

pub struct Charts {
    pub lines_layout: LinesInfo<EnabledChartSettings>,
    pub update_schedule_config: config::update_schedule::Config,
    pub update_groups: BTreeMap<String, ArcUpdateGroup>,
    pub charts_info: BTreeMap<String, EnabledChartEntry>,
    pub enabled_set: HashSet<String>,
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

impl Charts {
    pub fn new(
        charts: config::charts::Config<AllChartSettings>,
        update_schedule: config::update_schedule::Config,
    ) -> Result<Self, anyhow::Error> {
        Self::validated(charts, update_schedule)
    }

    fn validated(
        charts: config::charts::Config<AllChartSettings>,
        update_schedule: config::update_schedule::Config,
    ) -> Result<Self, anyhow::Error> {
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
        let settings = Self::new_settings(&enabled_charts_config);
        let update_groups = Self::all_update_groups();
        let charts_info = Self::all_charts()
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

        let enabled_charts = {
            let mut s = enabled_counters;
            s.extend(enabled_lines);
            s
        };

        Ok(Self {
            lines_layout: enabled_charts_config.lines,
            update_schedule_config: update_schedule,
            update_groups,
            charts_info,
            enabled_set: enabled_charts,
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

    // assumes that config is valid
    fn new_settings(
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

    fn all_update_groups() -> BTreeMap<String, ArcUpdateGroup> {
        let contracts = Arc::new(groups::Contracts);
        let groups: Vec<ArcUpdateGroup> = vec![contracts];
        groups.into_iter().map(|g| (g.name(), g)).collect()
    }

    fn all_charts() -> BTreeMap<String, ChartDynamic> {
        let charts_with_duplicates = Self::all_update_groups()
            .into_iter()
            .flat_map(|(_, g)| g.list_charts())
            .collect_vec();
        let mut charts = BTreeMap::new();
        for chart in charts_with_duplicates {
            match charts.entry(chart.name.clone()) {
                Entry::Vacant(v) => {
                    v.insert(chart);
                }
                Entry::Occupied(o) => {
                    assert_eq!(o.get(), &chart, "duplicate chart name '{}'", o.get().name);
                }
            }
        }
        charts
    }
}
