use super::{
    chart_info::{CounterInfo, LineChartInfo},
    toml_config,
};
use convert_case::{Case, Casing};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LineChartSection {
    pub title: String,
    pub order: Option<i64>,
    pub charts: BTreeMap<String, LineChartInfo>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub counters: BTreeMap<String, CounterInfo>,
    pub lines: BTreeMap<String, LineChartSection>,
    pub template_values: BTreeMap<String, serde_json::Value>,
}

impl From<Config> for toml_config::Config {
    fn from(value: Config) -> Self {
        Self {
            counters: value
                .counters
                .into_iter()
                .map(toml_config::CounterInfo::from)
                .collect(),
            lines: value
                .lines
                .into_iter()
                .sorted_by_key(|(_, section)| section.order)
                .map(toml_config::LineChartSection::from)
                .collect::<Vec<_>>()
                .into(),
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
            id: id.from_case(Case::Snake).to_case(Case::Camel),
            title: info.title,
            description: info.description,
            settings: info.settings,
        }
    }
}

impl From<(String, CounterInfo)> for toml_config::CounterInfo {
    fn from((id, info): (String, CounterInfo)) -> Self {
        Self {
            id: id.from_case(Case::Snake).to_case(Case::Camel),
            title: info.title,
            description: info.description,
            settings: info.settings,
        }
    }
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
