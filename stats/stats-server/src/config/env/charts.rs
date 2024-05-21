use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ChartSettingsOverwrite {
    pub enabled: Option<bool>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub units: Option<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct CounterInfoOrdered {
    pub order: Option<i64>,
    #[serde(flatten)]
    pub settings: ChartSettingsOverwrite,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct LineChartInfoOrdered {
    pub order: Option<i64>,
    #[serde(flatten)]
    pub settings: ChartSettingsOverwrite,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct LineChartCategoryOrdered {
    pub order: Option<i64>,
    pub title: Option<String>,
    pub charts: BTreeMap<String, LineChartInfoOrdered>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub counters: BTreeMap<String, CounterInfoOrdered>,
    pub line_categories: BTreeMap<String, LineChartCategoryOrdered>,
    pub template_values: BTreeMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::config::env::assert_envs_parsed_to;

    // purpose of env is to overwrite some attributes in existing configurations
    // therefore it should be possible to do it a granular manner

    #[test]
    fn single_attribute_overwrite_works_for_template() {
        // purpose of env is to overwrite some attributes in existing configurations
        // therefore it should be possible to do it a granular manner
        assert_envs_parsed_to(
            [(
                "STATS_CHARTS__TEMPLATE_VALUES__NATIVE_COIN_SYMBOL".to_owned(),
                "USDT".to_owned(),
            )]
            .into(),
            Config {
                counters: BTreeMap::new(),
                line_categories: BTreeMap::new(),
                template_values: BTreeMap::from_iter([(
                    "native_coin_symbol".to_owned(),
                    serde_json::Value::String("USDT".to_owned()),
                )]),
            },
        );
    }

    #[test]
    fn single_attribute_overwrite_works_for_line() {
        assert_envs_parsed_to(
            [(
                "STATS_CHARTS__LINE_CATEGORIES__TRANSACTIONS__CHARTS__AVERAGE_TXN_FEE__DESCRIPTION"
                    .to_owned(),
                "Some runtime-overwritten description".to_owned(),
            )]
            .into(),
            Config {
                counters: BTreeMap::new(),
                line_categories: BTreeMap::from_iter([(
                    "transactions".to_owned(),
                    LineChartCategoryOrdered {
                        order: None,
                        title: None,
                        charts: BTreeMap::from_iter([(
                            "average_txn_fee".to_owned(),
                            LineChartInfoOrdered {
                                order: None,
                                settings: ChartSettingsOverwrite {
                                    enabled: None,
                                    title: None,
                                    description: Some(
                                        "Some runtime-overwritten description".to_owned(),
                                    ),
                                    units: None,
                                },
                            },
                        )]),
                    },
                )]),
                template_values: BTreeMap::new(),
            },
        );

        assert_envs_parsed_to(
            [(
                "STATS_CHARTS__LINE_CATEGORIES__TRANSACTIONS__CHARTS__AVERAGE_TXN_FEE__ORDER"
                    .to_owned(),
                "1".to_owned(),
            )]
            .into(),
            Config {
                counters: BTreeMap::new(),
                line_categories: BTreeMap::from_iter([(
                    "transactions".to_owned(),
                    LineChartCategoryOrdered {
                        order: None,
                        title: None,
                        charts: BTreeMap::from_iter([(
                            "average_txn_fee".to_owned(),
                            LineChartInfoOrdered {
                                order: Some(1),
                                settings: ChartSettingsOverwrite {
                                    enabled: None,
                                    title: None,
                                    description: None,
                                    units: None,
                                },
                            },
                        )]),
                    },
                )]),
                template_values: BTreeMap::new(),
            },
        );

        assert_envs_parsed_to(
            [(
                "STATS_CHARTS__LINE_CATEGORIES__TRANSACTIONS__TITLE".to_owned(),
                "Trans chart".to_owned(),
            )]
            .into(),
            Config {
                counters: BTreeMap::new(),
                line_categories: BTreeMap::from_iter([(
                    "transactions".to_owned(),
                    LineChartCategoryOrdered {
                        order: None,
                        title: Some("Trans chart".to_owned()),
                        charts: BTreeMap::new(),
                    },
                )]),
                template_values: BTreeMap::new(),
            },
        );
    }

    #[test]
    fn single_attribute_overwrite_works_for_counters() {
        assert_envs_parsed_to(
            [(
                "STATS_CHARTS__COUNTERS__AVERAGE_BLOCK_TIME__ORDER".to_owned(),
                "1".to_owned(),
            )]
            .into(),
            Config {
                counters: BTreeMap::from_iter([(
                    "average_block_time".to_owned(),
                    CounterInfoOrdered {
                        order: Some(1),
                        settings: ChartSettingsOverwrite {
                            enabled: None,
                            title: None,
                            description: None,
                            units: None,
                        },
                    },
                )]),
                line_categories: BTreeMap::new(),
                template_values: BTreeMap::new(),
            },
        );

        assert_envs_parsed_to(
            [(
                "STATS_CHARTS__COUNTERS__AVERAGE_BLOCK_TIME__ENABLED".to_owned(),
                "true".to_owned(),
            )]
            .into(),
            Config {
                counters: BTreeMap::from_iter([(
                    "average_block_time".to_owned(),
                    CounterInfoOrdered {
                        order: None,
                        settings: ChartSettingsOverwrite {
                            enabled: Some(true),
                            title: None,
                            description: None,
                            units: None,
                        },
                    },
                )]),
                line_categories: BTreeMap::new(),
                template_values: BTreeMap::new(),
            },
        );
    }

    #[test]
    fn arbitrary_env_parsed_correctly() {
        let envs: HashMap<_, _> = [
            ("STATS_CHARTS__TEMPLATE_VALUES__NATIVE_COIN_SYMBOL", "USDC"),
            (
                "STATS_CHARTS__COUNTERS__AVERAGE_BLOCK_TIME__TITLE",
                "Average block time",
            ),
            (
                "STATS_CHARTS__COUNTERS__AVERAGE_BLOCK_TIME__ENABLED",
                "true",
            ),
            (
                "STATS_CHARTS__COUNTERS__AVERAGE_BLOCK_TIME__DESCRIPTION",
                "Some description kek",
            ),
            ("STATS_CHARTS__COUNTERS__AVERAGE_BLOCK_TIME__UNITS", "s"),
            ("STATS_CHARTS__LINE_CATEGORIES__TRANSACTIONS__ORDER", "123"),
            (
                "STATS_CHARTS__LINE_CATEGORIES__TRANSACTIONS__TITLE",
                "Transactions",
            ),
            (
                "STATS_CHARTS__LINE_CATEGORIES__TRANSACTIONS__CHARTS__AVERAGE_TXN_FEE__DESCRIPTION",
                "Some runtime-overwritten description",
            ),
            (
                "STATS_CHARTS__LINE_CATEGORIES__TRANSACTIONS__CHARTS__AVERAGE_TXN_FEE__ENABLED",
                "false",
            ),
        ]
        .map(|(s1, s2)| (s1.to_owned(), s2.to_owned()))
        .into();

        let expected_counter = CounterInfoOrdered {
            settings: ChartSettingsOverwrite {
                enabled: Some(true),
                title: Some("Average block time".to_owned()),
                description: Some("Some description kek".to_owned()),
                units: Some("s".to_owned()),
            },
            order: None,
        };
        let expected_line_category = LineChartCategoryOrdered {
            order: Some(123),
            title: Some("Transactions".to_owned()),
            charts: BTreeMap::from_iter([(
                "average_txn_fee".to_owned(),
                LineChartInfoOrdered {
                    order: None,
                    settings: ChartSettingsOverwrite {
                        enabled: Some(false),
                        title: None,
                        description: Some("Some runtime-overwritten description".to_owned()),
                        units: None,
                    },
                },
            )]),
        };

        assert_envs_parsed_to(
            envs,
            Config {
                counters: BTreeMap::from_iter([(
                    "average_block_time".to_owned(),
                    expected_counter,
                )]),
                line_categories: BTreeMap::from_iter([(
                    "transactions".to_owned(),
                    expected_line_category,
                )]),
                template_values: BTreeMap::from_iter([(
                    "native_coin_symbol".to_owned(),
                    serde_json::Value::String("USDC".to_owned()),
                )]),
            },
        );
    }
}
