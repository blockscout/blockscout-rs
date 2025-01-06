use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub counters_order: BTreeMap<String, usize>,
    pub line_chart_categories: BTreeMap<String, LineChartCategoryOrdered>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct LineChartCategoryOrdered {
    pub order: Option<usize>,
    pub title: Option<String>,
    pub charts_order: BTreeMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::config::env::test_utils::check_envs_parsed_to;

    use super::*;

    // purpose of env is to overwrite some attributes in existing configurations
    // therefore it should be possible to do it a granular manner

    #[test]
    fn single_attribute_overwrite_works_for_line() {
        check_envs_parsed_to(
            "STATS_LAYOUT",
            [(
                "STATS_LAYOUT__LINE_CHART_CATEGORIES__TRANSACTIONS__CHARTS_ORDER__AVERAGE_TXN_FEE"
                    .to_owned(),
                "1".to_owned(),
            )]
            .into(),
            Config {
                counters_order: BTreeMap::new(),
                line_chart_categories: BTreeMap::from([(
                    "transactions".to_owned(),
                    LineChartCategoryOrdered {
                        order: None,
                        title: None,
                        charts_order: BTreeMap::from([("average_txn_fee".to_owned(), 1)]),
                    },
                )]),
            },
        )
        .unwrap();

        check_envs_parsed_to(
            "STATS_LAYOUT",
            [(
                "STATS_LAYOUT__LINE_CHART_CATEGORIES__TRANSACTIONS__TITLE".to_owned(),
                "Trans chart".to_owned(),
            )]
            .into(),
            Config {
                counters_order: BTreeMap::new(),
                line_chart_categories: BTreeMap::from([(
                    "transactions".to_owned(),
                    LineChartCategoryOrdered {
                        order: None,
                        title: Some("Trans chart".to_owned()),
                        charts_order: BTreeMap::new(),
                    },
                )]),
            },
        )
        .unwrap();
    }

    #[test]
    fn single_attribute_overwrite_works_for_counters() {
        check_envs_parsed_to(
            "STATS_LAYOUT",
            [(
                "STATS_LAYOUT__COUNTERS_ORDER__AVERAGE_BLOCK_TIME".to_owned(),
                "1".to_owned(),
            )]
            .into(),
            Config {
                counters_order: BTreeMap::from([("average_block_time".to_owned(), 1)]),
                line_chart_categories: BTreeMap::new(),
            },
        )
        .unwrap();
    }

    #[test]
    fn arbitrary_envs_parsed_correctly() {
        let envs: HashMap<_, _> = [
            ("STATS_LAYOUT__COUNTERS_ORDER__AVERAGE_BLOCK_TIME", "1"),
            (
                "STATS_LAYOUT__LINE_CHART_CATEGORIES__TRANSACTIONS__ORDER",
                "123",
            ),
            (
                "STATS_LAYOUT__LINE_CHART_CATEGORIES__TRANSACTIONS__TITLE",
                "Transactions",
            ),
            (
                "STATS_LAYOUT__LINE_CHART_CATEGORIES__TRANSACTIONS__CHARTS_ORDER__AVERAGE_TXN_FEE",
                "321",
            ),
        ]
        .map(|(s1, s2)| (s1.to_owned(), s2.to_owned()))
        .into();

        let expected_line_category = LineChartCategoryOrdered {
            order: Some(123),
            title: Some("Transactions".to_owned()),
            charts_order: BTreeMap::from([("average_txn_fee".to_owned(), 321)]),
        };

        check_envs_parsed_to(
            "STATS_LAYOUT",
            envs,
            Config {
                counters_order: BTreeMap::from([("average_block_time".to_owned(), 1)]),
                line_chart_categories: BTreeMap::from([(
                    "transactions".to_owned(),
                    expected_line_category,
                )]),
            },
        )
        .unwrap();
    }
}
