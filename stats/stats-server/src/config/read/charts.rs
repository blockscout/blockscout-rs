use crate::config::{
    chart_info::{AllChartSettings, CounterInfo, LineChartCategory, LineChartInfo},
    json,
};
use serde::Deserialize;
use stats_proto::blockscout::stats::v1 as proto;

impl From<LineChartInfo<AllChartSettings>> for proto::LineChartInfo {
    fn from(value: LineChartInfo<AllChartSettings>) -> Self {
        Self {
            id: value.id,
            title: value.title,
            description: value.description,
            units: value.settings.units,
        }
    }
}

impl From<LineChartCategory<AllChartSettings>> for proto::LineChartSection {
    fn from(value: LineChartCategory<AllChartSettings>) -> Self {
        Self {
            id: value.id,
            title: value.title,
            charts: value.charts.into_iter().map(|x| x.into()).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config<ChartSettings> {
    pub counters: Vec<CounterInfo<ChartSettings>>,
    pub lines: Vec<LineChartCategory<ChartSettings>>,
}

impl From<json::charts::Config> for Config<AllChartSettings> {
    fn from(value: json::charts::Config) -> Self {
        Self {
            counters: value.counters,
            lines: value.line_categories,
        }
    }
}
