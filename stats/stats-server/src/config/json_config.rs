use super::{
    chart_info::{CounterInfo, LineChartInfo},
    toml_config,
};
use convert_case::{Case, Casing};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Default, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LineChartSection {
    pub title: String,
    pub charts: BTreeMap<String, LineChartInfo>,
}

#[derive(Default, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LineCharts {
    pub sections: BTreeMap<String, LineChartSection>,
}

#[derive(Default, Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub counters: BTreeMap<String, CounterInfo>,
    pub lines: LineCharts,
}

impl From<Config> for toml_config::Config {
    fn from(value: Config) -> Self {
        Self {
            counters: value
                .counters
                .into_iter()
                .map(toml_config::CounterInfo::from)
                .collect(),
            lines: value.lines.into(),
        }
    }
}

impl From<LineCharts> for toml_config::LineCharts {
    fn from(value: LineCharts) -> Self {
        Self {
            sections: value
                .sections
                .into_iter()
                .map(toml_config::LineChartSection::from)
                .collect(),
        }
    }
}

impl From<(String, LineChartSection)> for toml_config::LineChartSection {
    fn from((id, section): (String, LineChartSection)) -> Self {
        Self {
            id,
            title: section.title,
            charts: section
                .charts
                .into_iter()
                .map(toml_config::LineChartInfo::from)
                .collect(),
        }
    }
}

impl From<(String, LineChartInfo)> for toml_config::LineChartInfo {
    fn from((id, info): (String, LineChartInfo)) -> Self {
        Self {
            id: id.to_case(Case::Camel),
            title: info.title,
            description: info.description,
            settings: info.settings,
        }
    }
}

impl From<(String, CounterInfo)> for toml_config::CounterInfo {
    fn from((id, info): (String, CounterInfo)) -> Self {
        Self {
            id: id.to_case(Case::Camel),
            title: info.title,
            settings: info.settings,
        }
    }
}
