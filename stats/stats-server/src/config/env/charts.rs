use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::config::chart_info::AllChartSettings;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CounterInfoOrdered {
    pub order: Option<i64>,
    pub title: String,
    pub description: String,
    #[serde(flatten)]
    pub settings: AllChartSettings,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LineChartInfoOrdered {
    pub order: Option<i64>,
    pub title: String,
    pub description: String,
    #[serde(flatten)]
    pub settings: AllChartSettings,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LineChartCategoryOrdered {
    pub order: Option<i64>,
    pub title: String,
    pub charts: BTreeMap<String, LineChartInfoOrdered>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub counters: BTreeMap<String, CounterInfoOrdered>,
    pub line_categories: BTreeMap<String, LineChartCategoryOrdered>,
    pub template_values: BTreeMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // todo: test layout is correct (via json)
    #[test]
    fn render_works() {
        let config: Config = serde_json::from_str(r#"{
            "counters": {
                "total_blocks": {
                    "title": "Total Blocks",
                    "update_schedule": "0 0 */3 * * * *"
                }
            },
            "lines": {
                "accounts": {
                    "title": "Accounts",
                    "charts": {
                        "average_txn_fee": {
                            "title": "Average transaction fee",
                            "description": "The average amount in {{native_coin_symbol}} spent per transaction",
                            "units": "{{native_coin_symbol}}",
                            "update_schedule": "0 0 6 * * * *"
                        },
                        "txns_fee": {
                            "title": "Transactions fees",
                            "description": "Amount of tokens paid as fees",
                            "units": "{{native_coin_symbol}}",
                            "update_schedule": "0 0 7 * * * *"
                        }
                    }
                }
            },
            "template_values": {
                "native_coin_symbol": "USDT"
            }
        }"#).expect("should be valid config");

        let config = serde_json::to_value(config).expect("failed to serialize");

        let expected_config: Config = serde_json::from_str(
            r#"{
            "counters": {
                "total_blocks": {
                    "title": "Total Blocks",
                    "update_schedule": "0 0 */3 * * * *"
                }
            },
            "lines": {
                "accounts": {
                    "title": "Accounts",
                    "charts": {
                        "average_txn_fee": {
                            "title": "Average transaction fee",
                            "description": "The average amount in USDT spent per transaction",
                            "units": "USDT",
                            "update_schedule": "0 0 6 * * * *"
                        },
                        "txns_fee": {
                            "title": "Transactions fees",
                            "description": "Amount of tokens paid as fees",
                            "units": "USDT",
                            "update_schedule": "0 0 7 * * * *"
                        }
                    }
                }
            },
            "template_values": {
                "native_coin_symbol": "USDT"
            }
        }"#,
        )
        .expect("should be valid config");
        let expected_config = serde_json::to_value(expected_config).unwrap();

        assert_eq!(config, expected_config);
    }
}
