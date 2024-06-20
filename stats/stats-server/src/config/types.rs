//! Common types for the configs

use std::collections::BTreeMap;

use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use stats_proto::blockscout::stats::v1 as proto_v1;

use crate::runtime_setup::EnabledChartEntry;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LineChartCategory {
    pub id: String,
    pub title: String,
    pub charts_order: Vec<String>,
}

impl LineChartCategory {
    pub fn insert_settings(
        self,
        settings: &BTreeMap<String, EnabledChartEntry>,
    ) -> Option<proto_v1::LineChartSection> {
        let charts: Result<Vec<_>, String> = self
            .charts_order
            .into_iter()
            .map(|c: String| {
                if let Some(e) = settings.get(&c) {
                    Ok(LineChartInfo {
                        id: c,
                        settings: e.settings.clone(),
                    }
                    .into())
                } else {
                    Err(c)
                }
            })
            .collect();
        let charts = match charts {
            Ok(c) => c,
            Err(missing_settings) => {
                tracing::error!("Did not find settings for chart {missing_settings}");
                return None;
            }
        };
        Some(proto_v1::LineChartSection {
            id: self.id,
            title: self.title,
            charts,
        })
    }
}

#[serde_as]
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct UpdateGroup {
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub update_schedule: Option<Schedule>,
}
