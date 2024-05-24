use crate::config::{
    json,
    types::{
        AllChartSettings, CounterInfo, EnabledChartSettings, LineChartCategory, LineChartInfo,
    },
};
use serde::Deserialize;
use stats_proto::blockscout::stats::v1 as proto;

impl From<LineChartInfo<EnabledChartSettings>> for proto::LineChartInfo {
    fn from(value: LineChartInfo<EnabledChartSettings>) -> Self {
        Self {
            id: value.id,
            title: value.settings.title,
            description: value.settings.description,
            units: value.settings.units,
        }
    }
}

impl From<LineChartCategory<EnabledChartSettings>> for proto::LineChartSection {
    fn from(value: LineChartCategory<EnabledChartSettings>) -> Self {
        Self {
            id: value.id,
            title: value.title,
            charts: value.charts.into_iter().map(|x| x.into()).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinesInfo<ChartSettings>(pub Vec<LineChartCategory<ChartSettings>>);

impl From<LinesInfo<EnabledChartSettings>> for proto::LineCharts {
    fn from(value: LinesInfo<EnabledChartSettings>) -> Self {
        Self {
            sections: value.0.into_iter().map(|x| x.into()).collect(),
        }
    }
}

impl<S> From<Vec<LineChartCategory<S>>> for LinesInfo<S> {
    fn from(value: Vec<LineChartCategory<S>>) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config<ChartSettings> {
    pub counters: Vec<CounterInfo<ChartSettings>>,
    pub lines: LinesInfo<ChartSettings>,
}

impl From<json::charts::Config> for Config<AllChartSettings> {
    fn from(value: json::charts::Config) -> Self {
        Self {
            counters: value.counters,
            lines: LinesInfo(value.line_categories),
        }
    }
}
