use crate::config::{json, types::LineChartCategory};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub line_chart_categories: Vec<LineChartCategory>,
}

impl From<json::layout::Config> for Config {
    fn from(value: json::layout::Config) -> Self {
        Self {
            line_chart_categories: value.line_chart_categories,
        }
    }
}
