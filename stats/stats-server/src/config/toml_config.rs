use super::chart_info::ChartSettings;
use serde::Deserialize;
use stats_proto::blockscout::stats::v1 as proto;

#[derive(Debug, Clone, Deserialize)]
pub struct CounterInfo {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(flatten)]
    pub settings: ChartSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LineChartInfo {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(flatten)]
    pub settings: ChartSettings,
}

impl From<LineChartInfo> for proto::LineChartInfo {
    fn from(value: LineChartInfo) -> Self {
        Self {
            id: value.id,
            title: value.title,
            description: value.description,
            units: value.settings.units,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LineChartSection {
    pub id: String,
    pub title: String,
    pub charts: Vec<LineChartInfo>,
}

impl From<Vec<LineChartSection>> for LineCharts {
    fn from(sections: Vec<LineChartSection>) -> Self {
        Self { sections }
    }
}

impl From<LineChartSection> for proto::LineChartSection {
    fn from(value: LineChartSection) -> Self {
        Self {
            id: value.id,
            title: value.title,
            charts: value.charts.into_iter().map(|x| x.into()).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LineCharts {
    pub sections: Vec<LineChartSection>,
}

impl From<LineCharts> for proto::LineCharts {
    fn from(value: LineCharts) -> Self {
        Self {
            sections: value.sections.into_iter().map(|x| x.into()).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub counters: Vec<CounterInfo>,
    pub lines: LineCharts,
}
