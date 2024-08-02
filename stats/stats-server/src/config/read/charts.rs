use std::collections::BTreeMap;

use crate::config::{
    json,
    types::{AllChartSettings, EnabledChartSettings, LineChartInfo},
};
use convert_case::{Case, Casing};
use serde::Deserialize;
use stats_proto::blockscout::stats::v1 as proto;

impl From<LineChartInfo<EnabledChartSettings>> for proto::LineChartInfo {
    fn from(value: LineChartInfo<EnabledChartSettings>) -> Self {
        Self {
            id: value.id,
            title: value.settings.title,
            description: value.settings.description,
            units: value.settings.units,
            resolutions: value
                .settings
                .resolutions
                .into_list()
                .into_iter()
                .map(|r| String::from(r))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config<ChartSettings> {
    pub counters: BTreeMap<String, ChartSettings>,
    pub lines: BTreeMap<String, ChartSettings>,
}

impl From<json::charts::Config> for Config<AllChartSettings> {
    fn from(value: json::charts::Config) -> Self {
        let counters = value
            .counters
            .into_iter()
            .map(|(id, s)| (id.from_case(Case::Snake).to_case(Case::Camel), s))
            .collect();
        let lines = value
            .line_charts
            .into_iter()
            .map(|(id, s)| (id.from_case(Case::Snake).to_case(Case::Camel), s))
            .collect();
        Self { counters, lines }
    }
}
