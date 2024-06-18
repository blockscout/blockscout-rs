use crate::config::types::LineChartCategory;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub counters_order: Vec<String>,
    pub line_chart_categories: Vec<LineChartCategory>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const EXAMPLE_CONFIG: &str = r#"{
        "counters_order": [
            "total_blocks",
            "total_txns"
        ],
        "line_chart_categories": [
            {
                "id": "accounts",
                "title": "Accounts",
                "charts_order": [
                    "average_txn_fee",
                    "txns_fee"
                ]
            }
        ]
    }"#;

    #[test]
    fn config_parses_correctly() {
        let c: Config = serde_json::from_str(EXAMPLE_CONFIG).expect("should be valid config");
        assert_eq!(
            c,
            Config {
                counters_order: ["total_blocks", "total_txns"].map(|s| s.to_owned()).into(),
                line_chart_categories: vec![LineChartCategory {
                    id: "accounts".into(),
                    title: "Accounts".into(),
                    charts_order: ["average_txn_fee", "txns_fee"].map(|s| s.to_owned()).into()
                }]
            }
        )
    }
}
