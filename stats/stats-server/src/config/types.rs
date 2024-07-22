//! Common types for the configs

use std::collections::BTreeMap;

use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use stats::ResolutionKind;
use stats_proto::blockscout::stats::v1 as proto_v1;

use crate::runtime_setup::EnabledChartEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResolutionsEnabled {
    #[serde(default = "enabled_default")]
    pub day: bool,
    #[serde(default = "enabled_default")]
    pub week: bool,
    #[serde(default = "enabled_default")]
    pub month: bool,
    #[serde(default = "enabled_default")]
    pub year: bool,
}

impl Default for ResolutionsEnabled {
    fn default() -> Self {
        Self {
            day: enabled_default(),
            week: enabled_default(),
            month: enabled_default(),
            year: enabled_default(),
        }
    }
}

impl ResolutionsEnabled {
    pub fn into_enabled(self) -> Vec<ResolutionKind> {
        [
            ResolutionKind::Day,
            ResolutionKind::Week,
            ResolutionKind::Month,
            ResolutionKind::Year,
        ]
        .into_iter()
        .filter(|res| match res {
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
    #[serde(default = "ResolutionsEnabled::default")]
    pub resolutions: ResolutionsEnabled,
}

fn enabled_default() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnabledChartSettings {
    pub title: String,
    pub description: String,
    pub units: Option<String>,
    pub resolutions: ResolutionsEnabled,
}

impl EnabledChartSettings {
    pub fn from_all(value: AllChartSettings) -> Option<Self> {
        if value.enabled {
            Some(EnabledChartSettings {
                units: value.units,
                title: value.title,
                description: value.description,
                resolutions: value.resolutions,
            })
        } else {
            None
        }
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
    pub fn intersect_settings(
        self,
        settings: &BTreeMap<String, EnabledChartEntry>,
    ) -> proto_v1::LineChartSection {
        let charts: Vec<_> = self
            .charts_order
            .into_iter()
            .flat_map(|c: String| {
                settings.get(&c).map(|e| {
                    LineChartInfo {
                        id: c,
                        settings: e.settings.clone(),
                    }
                    .into()
                })
            })
            .collect();
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
