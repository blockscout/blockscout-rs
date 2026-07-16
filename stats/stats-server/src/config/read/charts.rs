// SPDX-License-Identifier: LicenseRef-Blockscout

use std::collections::BTreeMap;

use crate::config::{json, types::AllChartSettings};
use convert_case::{Case, Casing};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config<ChartSettings> {
    pub counters: BTreeMap<String, ChartSettings>,
    pub lines: BTreeMap<String, ChartSettings>,
}

impl From<json::charts::Config> for Config<AllChartSettings> {
    fn from(value: json::charts::Config) -> Self {
        let to_camel_case = |(id, mut settings): (String, AllChartSettings)| {
            settings.implementation = settings
                .implementation
                .map(|name| name.from_case(Case::Snake).to_case(Case::Camel));
            (id.from_case(Case::Snake).to_case(Case::Camel), settings)
        };
        let counters = value.counters.into_iter().map(to_camel_case).collect();
        let lines = value.line_charts.into_iter().map(to_camel_case).collect();
        Self { counters, lines }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implementation_value_is_converted_to_camel_case() {
        let json_config: json::charts::Config = serde_json::from_str(
            r#"{
                "counters": {
                    "some_counter": {
                        "title": "Some counter",
                        "description": "Some description",
                        "implementation": "another_counter"
                    }
                },
                "line_charts": {
                    "txns_fee": {
                        "title": "Transactions fees",
                        "description": "Amount of tokens paid as fees",
                        "implementation": "filecoin_new_chain_fees"
                    }
                }
            }"#,
        )
        .unwrap();

        let config: Config<AllChartSettings> = json_config.into();

        assert_eq!(
            config.counters["someCounter"].implementation.as_deref(),
            Some("anotherCounter")
        );
        assert_eq!(
            config.lines["txnsFee"].implementation.as_deref(),
            Some("filecoinNewChainFees")
        );
    }
}
