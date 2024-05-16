use crate::config::chart_info::{AllChartSettings, CounterInfo, LineChartCategory};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub counters: Vec<CounterInfo<AllChartSettings>>,
    pub line_categories: Vec<LineChartCategory<AllChartSettings>>,
    pub template_values: BTreeMap<String, serde_json::Value>,
}

impl Config {
    pub fn render_with_template_values(self) -> Result<Self, anyhow::Error> {
        let context = serde_json::to_value(self.template_values.clone())?;
        let template_json = serde_json::to_value(self)?;
        let actual = liquid_json::LiquidJson::new(template_json).render(&context)?;
        let new_config = serde_json::from_value(actual)?;
        Ok(new_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

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

        let config = serde_json::to_value(
            config
                .render_with_template_values()
                .expect("failed to render"),
        )
        .expect("failed to serialize");

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
