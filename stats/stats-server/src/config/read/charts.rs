use crate::config::{
    json,
    types::{
        AllChartSettings, CounterInfo, EnabledChartSettings, LineChartCategory, LineChartInfo,
    },
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
        let counters = value
            .counters
            .into_iter()
            .map(|c| CounterInfo {
                id: c.id.from_case(Case::Snake).to_case(Case::Camel),
                ..c
            })
            .collect();
        let lines = value
            .line_categories
            .into_iter()
            .map(|l| LineChartCategory {
                id: l.id.from_case(Case::Snake).to_case(Case::Camel),
                ..l
            })
            .collect();
        Self {
            counters,
            lines: LinesInfo(lines),
        }
    }
}
