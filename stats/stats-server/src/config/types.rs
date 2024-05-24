//! Common types for the configs

use std::collections::HashSet;

use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
/// includes disabled charts
pub struct AllChartSettings {
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    pub title: String,
    pub description: String,
    pub units: Option<String>,
}

fn enabled_default() -> bool {
    true
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineChartCategory<ChartSettings> {
    pub id: String,
    pub title: String,
    pub charts: Vec<LineChartInfo<ChartSettings>>,
}

#[serde_as]
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct UpdateGroup {
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub update_schedule: Option<Schedule>,
    /// Dynamically disable some group members.
    /// These charts won't get directly updated by the group
    /// (they can still get updated if they are depended upon)
    pub ignore_charts: HashSet<String>,
}
