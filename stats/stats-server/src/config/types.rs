//! Common types for the configs

use std::collections::{BTreeMap, HashSet};

use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use stats::ResolutionKind;
use stats_proto::blockscout::stats::v1 as proto_v1;

use crate::runtime_setup::EnabledChartEntry;

use super::layout::sorted_items_according_to_layout;

/// `None` means 'enable if present'
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResolutionsSettings {
    pub day: Option<bool>,
    pub week: Option<bool>,
    pub month: Option<bool>,
    pub year: Option<bool>,
}

impl ResolutionsSettings {
    pub fn into_list(self) -> Vec<(ResolutionKind, Option<bool>)> {
        vec![
            (ResolutionKind::Day, self.day),
            (ResolutionKind::Week, self.week),
            (ResolutionKind::Month, self.month),
            (ResolutionKind::Year, self.year),
        ]
    }

    fn into_enabled_field(
        setting: Option<bool>,
        kind: ResolutionKind,
        available_resolutions: &HashSet<ResolutionKind>,
    ) -> Result<bool, ResolutionKind> {
        let is_available = available_resolutions.contains(&kind);
        if let Some(setting) = setting {
            if is_available {
                Ok(setting)
            } else {
                Err(kind)
            }
        } else {
            Ok(is_available)
        }
    }

    pub fn into_enabled(
        self,
        available_resolutions: &HashSet<ResolutionKind>,
    ) -> Result<ResolutionsEnabled, Vec<ResolutionKind>> {
        let day = Self::into_enabled_field(self.day, ResolutionKind::Day, available_resolutions);
        let week = Self::into_enabled_field(self.week, ResolutionKind::Week, available_resolutions);
        let month =
            Self::into_enabled_field(self.month, ResolutionKind::Month, available_resolutions);
        let year = Self::into_enabled_field(self.year, ResolutionKind::Year, available_resolutions);
        match (day, week, month, year) {
            (Ok(day), Ok(week), Ok(month), Ok(year)) => Ok(ResolutionsEnabled {
                day,
                week,
                month,
                year,
            }),
            (d, w, m, y) => Err([d, w, m, y]
                .into_iter()
                .filter_map(|res| res.err())
                .collect()),
        }
    }
}

/// `true` if the resolution is active for the chart
#[derive(Debug, Clone)]
pub struct ResolutionsEnabled {
    pub day: bool,
    pub week: bool,
    pub month: bool,
    pub year: bool,
}

impl ResolutionsEnabled {
    pub fn into_list(self) -> Vec<ResolutionKind> {
        [
            ResolutionKind::Day,
            ResolutionKind::Week,
            ResolutionKind::Month,
            ResolutionKind::Year,
        ]
        .into_iter()
        .filter(|res| match res {
            // add new corresponding resolution
            // to the array above
            ResolutionKind::Day => self.day,
            ResolutionKind::Week => self.week,
            ResolutionKind::Month => self.month,
            ResolutionKind::Year => self.year,
        })
        .collect()
    }

    pub fn is_enabled(&self, resolution: &ResolutionKind) -> bool {
        match resolution {
            ResolutionKind::Day => self.day,
            ResolutionKind::Week => self.week,
            ResolutionKind::Month => self.month,
            ResolutionKind::Year => self.year,
        }
    }
}

/// Includes disabled charts
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AllChartSettings {
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    pub title: String,
    pub description: String,
    pub units: Option<String>,
    #[serde(default = "Default::default")]
    pub resolutions: ResolutionsSettings,
}

fn enabled_default() -> bool {
    true
}

impl AllChartSettings {
    /// `Some(_)` - The chart is enabled.
    ///
    /// `None` - The chart is disabled.
    pub fn into_enabled(self) -> Option<EnabledChartSettings> {
        if self.enabled {
            Some(EnabledChartSettings {
                units: self.units,
                title: self.title,
                description: self.description,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnabledChartSettings {
    pub title: String,
    pub description: String,
    pub units: Option<String>,
}

impl EnabledChartSettings {
    /// * `Some(_)` - The chart is enabled
    /// * `None` - The chart is disabled
    pub fn from_all(value: AllChartSettings) -> Option<Self> {
        value.into_enabled()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterInfo<ChartSettings> {
    pub id: String,
    #[serde(flatten)]
    pub settings: ChartSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineChartInfo<ChartSettings> {
    pub id: String,
    #[serde(flatten)]
    pub settings: ChartSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LineChartCategory {
    pub id: String,
    pub title: String,
    pub charts_order: Vec<String>,
}

impl LineChartCategory {
    /// Add settings to the charts within category.
    ///
    /// If the settings are not present - remove the chart (i.e. remove disabled
    /// or nonexistent charts)
    pub fn intersect_info(
        self,
        info: &BTreeMap<String, EnabledChartEntry>,
    ) -> proto_v1::LineChartSection {
        let category_charts = HashSet::<_>::from_iter(self.charts_order.iter());
        let category_infos_alphabetic_order: Vec<_> = info
            .iter()
            .filter(|(id, _)| category_charts.contains(id))
            .map(|(id, e)| e.build_proto_line_chart_info(id.to_string()))
            .collect();
        let charts = sorted_items_according_to_layout(
            category_infos_alphabetic_order,
            &self.charts_order,
            |chart_info| &chart_info.id,
            true,
        );
        proto_v1::LineChartSection {
            id: self.id,
            title: self.title,
            charts,
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, transparent)]
pub struct UpdateSchedule {
    #[serde_as(as = "DisplayFromStr")]
    pub update_schedule: Schedule,
}
