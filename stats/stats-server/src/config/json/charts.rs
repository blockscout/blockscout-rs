use crate::config::types::{AllChartSettings, CounterInfo, LineChartCategory};
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

    const EXAMPLE_CONFIG: &str = r#"{
        "counters": [
            {
                "id": "total_blocks",
                "title": "Total Blocks",
                "description": "Number of all blocks in the network",
                "units": "blocks"
            },
            {
                "id": "total_txns",
                "enabled": false,
                "title": "Total txns",
                "description": "All transactions including pending, dropped, replaced, failed transactions"
            }
        ],
        "line_categories": [
            {
                "id": "accounts",
                "title": "Accounts",
                "charts": [
                    {
                        "id": "average_txn_fee",
                        "enabled": false,
                        "title": "Average transaction fee",
                        "description": "The average amount in {{native_coin_symbol}} spent per transaction",
                        "units": "{{native_coin_symbol}}"
                    },
                    {
                        "id": "txns_fee",
                        "title": "Transactions fees",
                        "description": "Amount of tokens paid as fees",
                        "units": "{{native_coin_symbol}}"
                    }
                ]
            }
        ],
        "template_values": {
            "native_coin_symbol": "USDT"
        }
    }"#;

    #[test]
    fn config_parses() {
        let _: Config = serde_json::from_str(EXAMPLE_CONFIG).expect("should be valid config");
    }

    #[test]
    fn render_works() {
        let config: Config = serde_json::from_str(EXAMPLE_CONFIG).expect("should be valid config");

        let config = serde_json::to_value(
            config
                .render_with_template_values()
                .expect("failed to render"),
        )
        .expect("failed to serialize");

        let expected_config_str = &EXAMPLE_CONFIG.replace("{{native_coin_symbol}}", "USDT");
        let expected_config: Config =
            serde_json::from_str(expected_config_str).expect("should be valid config");
        let expected_config = serde_json::to_value(expected_config).unwrap();

        assert_eq!(config, expected_config);
    }
}
