use crate::config::{json, types::LineChartCategory};
use convert_case::{Case, Casing};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub line_chart_categories: Vec<LineChartCategory>,
}

impl From<json::layout::Config> for Config {
    fn from(value: json::layout::Config) -> Self {
        let mut line_chart_categories = value.line_chart_categories;
        for cat in line_chart_categories.iter_mut() {
            for chart_name in cat.charts_order.iter_mut() {
                *chart_name = chart_name.from_case(Case::Snake).to_case(Case::Camel)
            }
        }
        Self {
            line_chart_categories,
        }
    }
}
