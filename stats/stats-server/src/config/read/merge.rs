//! Logic for overwriting config values from some env variables.

use anyhow::Context;
use itertools::{Either, Itertools};

use crate::config::{
    env::{
        self,
        charts::{CounterSettingsOverwrite, LineSettingsOverwrite},
        layout::LineChartCategoryOrdered,
    },
    json,
    types::{AllCounterSettings, AllLineSettings, CounterInfo, LineChartCategory, LineChartInfo},
};
use std::collections::{btree_map::Entry, BTreeMap};

/// Prioritize values from environment
pub fn override_charts(
    target: &mut json::charts::Config,
    source: env::charts::Config,
) -> Result<(), anyhow::Error> {
    override_counter_settings(&mut target.counters, source.counters)
        .context("updating counters")?;
    override_line_settings(&mut target.line_charts, source.line_charts)
        .context("updating line categories")?;
    target.template_values.extend(source.template_values);
    Ok(())
}

/// Prioritize values from environment
pub fn override_layout(
    target: &mut json::layout::Config,
    source: env::layout::Config,
) -> Result<(), anyhow::Error> {
    override_ordered(&mut target.counters_order, source.counters_order, |_, _| {
        Ok(())
    })?;
    override_ordered(
        &mut target.line_chart_categories,
        source.line_chart_categories,
        override_field::line_categories,
    )?;
    Ok(())
}

/// Prioritize values from environment
pub fn override_update_groups(
    target: &mut json::update_groups::Config,
    source: env::update_groups::Config,
) -> Result<(), anyhow::Error> {
    for (group_name, added_settings) in source.schedules {
        match target.schedules.entry(group_name) {
            Entry::Vacant(v) => {
                v.insert(added_settings);
            }
            Entry::Occupied(mut o) => {
                let target_group = o.get_mut();
                target_group.update_schedule = added_settings.update_schedule;
            }
        }
    }
    Ok(())
}

trait GetOrder {
    fn order(&self) -> Option<usize>;
}

impl GetOrder for usize {
    fn order(&self) -> Option<usize> {
        Some(*self)
    }
}

impl GetOrder for LineChartCategoryOrdered {
    fn order(&self) -> Option<usize> {
        self.order
    }
}

trait GetKey {
    fn key(&self) -> &str;
}

impl GetKey for String {
    fn key(&self) -> &str {
        self
    }
}

macro_rules! impl_get_key {
    ($type:ident) => {
        impl GetKey for $type {
            fn key(&self) -> &str {
                &self.id
            }
        }
    };
    ($type:ident<$($generic_param:ident),+ $(,)?>) => {
        impl<$($generic_param),+> GetKey for $type<$($generic_param),+> {
            fn key(&self) -> &str {
                &self.id
            }
        }
    };
}

impl_get_key!(CounterInfo<C>);
impl_get_key!(LineChartInfo<C>);
impl_get_key!(LineChartCategory);

/// Returns `target` with updated order of elements.
///
/// Each element with key in `new_order` is placed on `new_order.get(key)`'th position
/// in `target` (or at the end, if the position exceeds target's length).
fn with_updated_order<T: GetKey>(target: Vec<T>, new_order: BTreeMap<String, usize>) -> Vec<T> {
    let (moved_elements, mut other_elements): (BTreeMap<_, _>, Vec<_>) =
        target.into_iter().partition_map(|t| {
            if let Some(move_to) = new_order.get(t.key()) {
                Either::Left((move_to, t))
            } else {
                Either::Right(t)
            }
        });
    // it's important to iterate in ascending order to not mess up
    // ordering of previous elements
    for (&new_idx, element) in moved_elements {
        if new_idx <= other_elements.len() {
            other_elements.insert(new_idx, element)
        } else {
            other_elements.push(element)
        }
    }
    other_elements
}

/// `update_t` should update 1st parameter with values from the 2nd
fn override_ordered<T, S, F>(
    target: &mut Vec<T>,
    mut source: BTreeMap<String, S>,
    update_t: F,
) -> Result<(), anyhow::Error>
where
    T: GetKey + Clone,
    S: GetOrder,
    F: Fn(&mut T, S) -> Result<(), anyhow::Error>,
{
    // elements with overwritten order (collect to safely consume `source`)
    let orders_overwrite: BTreeMap<_, _> = source
        .iter()
        .filter_map(|(key, val)| Some((key.clone(), val.order()?)))
        .collect();

    // first overwrite only contents (not order)
    for element in target.iter_mut() {
        if let Some(overwrite) = source.remove(element.key()) {
            update_t(element, overwrite)?
        }
    }
    // overwrite order
    *target = with_updated_order(std::mem::take(target), orders_overwrite);
    Ok(())
}

mod override_field {
    use super::*;

    use crate::config::{env::layout::LineChartCategoryOrdered, types::LineChartCategory};

    pub fn line_categories(
        target: &mut LineChartCategory,
        source: LineChartCategoryOrdered,
    ) -> Result<(), anyhow::Error> {
        if let Some(title) = source.title {
            target.title = title;
        }
        override_ordered(&mut target.charts_order, source.charts_order, |_, _| Ok(()))
            .context("updating charts in category")
    }
}

fn override_counter_settings(
    target: &mut BTreeMap<String, AllCounterSettings>,
    source: BTreeMap<String, CounterSettingsOverwrite>,
) -> Result<(), anyhow::Error> {
    for (id, settings) in source {
        match target.entry(id) {
            Entry::Vacant(v) => {
                v.insert(settings.try_into()?);
            }
            Entry::Occupied(mut o) => settings.apply_to(o.get_mut()),
        }
    }
    Ok(())
}

fn override_line_settings(
    target: &mut BTreeMap<String, AllLineSettings>,
    source: BTreeMap<String, LineSettingsOverwrite>,
) -> Result<(), anyhow::Error> {
    for (id, settings) in source {
        match target.entry(id) {
            Entry::Vacant(v) => {
                v.insert(settings.try_into()?);
            }
            Entry::Occupied(mut o) => settings.apply_to(o.get_mut()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::config::{
        env::{self, test_utils::config_from_env},
        json,
    };
    use pretty_assertions::assert_eq;

    use super::*;

    const EXAMPLE_CHART_CONFIG: &str = r#"{
        "counters": {
            "total_blocks": {
                "title": "Total Blocks",
                "description": "Number of all blocks in the network",
                "units": "blocks"
            },
            "total_txns": {
                "enabled": false,
                "title": "Total txns",
                "description": "All transactions including pending, dropped, replaced, failed transactions"
            }
        },
        "line_charts": {
            "average_txn_fee": {
                "enabled": false,
                "title": "Average transaction fee",
                "description": "The average amount in {{native_coin_symbol}} spent per transaction",
                "units": "{{native_coin_symbol}}"
            },
            "txns_fee": {
                "title": "Transactions fees",
                "description": "Amount of tokens paid as fees",
                "units": "{{native_coin_symbol}}"
            }
        },
        "template_values": {
            "native_coin_symbol": "USDT"
        }
    }"#;

    #[test]
    fn charts_overridden_correctly() {
        let mut json_config: json::charts::Config =
            serde_json::from_str(EXAMPLE_CHART_CONFIG).unwrap();

        let env_override: env::charts::Config = config_from_env(
            "STATS_CHARTS",
            HashMap::from_iter(
                [
                    ("STATS_CHARTS__TEMPLATE_VALUES__NATIVE_COIN_SYMBOL", "USDC"),
                    ("STATS_CHARTS__COUNTERS__TOTAL_BLOCKS__ENABLED", "false"),
                    ("STATS_CHARTS__COUNTERS__TOTAL_TXNS__ENABLED", "true"),
                    ("STATS_CHARTS__LINE_CHARTS__TXNS_FEE__UNITS", "k USDC"),
                ]
                .map(|(a, b)| (a.to_owned(), b.to_owned())),
            ),
        )
        .unwrap();

        override_charts(&mut json_config, env_override).unwrap();
        let overridden_config = serde_json::to_value(json_config).unwrap();

        let expected_config: json::charts::Config = serde_json::from_str(r#"{
            "counters": {
                "total_blocks": {
                    "title": "Total Blocks",
                    "description": "Number of all blocks in the network",
                    "units": "blocks",
                    "enabled": false
                },
                "total_txns": {
                    "enabled": true,
                    "title": "Total txns",
                    "description": "All transactions including pending, dropped, replaced, failed transactions"
                }
            },
            "line_charts": {
                "average_txn_fee": {
                    "enabled": false,
                    "title": "Average transaction fee",
                    "description": "The average amount in {{native_coin_symbol}} spent per transaction",
                    "units": "{{native_coin_symbol}}"
                },
                "txns_fee": {
                    "title": "Transactions fees",
                    "description": "Amount of tokens paid as fees",
                    "units": "k USDC"
                }
            },
            "template_values": {
                "native_coin_symbol": "USDC"
            }
        }"#).unwrap();
        let expected_config = serde_json::to_value(expected_config).unwrap();

        assert_eq!(overridden_config, expected_config)
    }

    const EXAMPLE_LAYOUT_CONFIG: &str = r#"{
        "counters_order": [
            "total_blocks",
            "total_txns"
        ],
        "line_chart_categories": [
            {
                "id": "accounts",
                "title": "Accounts",
                "charts_order": [
                    "new_accounts",
                    "accounts_growth"
                ]
            },
            {
                "id": "transactions",
                "title": "Transactions",
                "charts_order": [
                    "average_txn_fee",
                    "txns_fee"
                ]
            }
        ]
    }"#;

    #[test]
    fn layout_overridden_correctly() {
        let mut json_config: json::layout::Config =
            serde_json::from_str(EXAMPLE_LAYOUT_CONFIG).unwrap();

        let env_override: env::layout::Config = config_from_env(
            "STATS_LAYOUT",
            HashMap::from_iter(
            [
                ("STATS_LAYOUT__COUNTERS_ORDER__TOTAL_BLOCKS", "256"),
                (
                    "STATS_LAYOUT__LINE_CHART_CATEGORIES__TRANSACTIONS__ORDER",
                    "1",
                ),
                (
                    "STATS_LAYOUT__LINE_CHART_CATEGORIES__TRANSACTIONS__TITLE",
                    "CoolTransactions",
                ),
                (
                    "STATS_LAYOUT__LINE_CHART_CATEGORIES__ACCOUNTS__CHARTS_ORDER__ACCOUNTS_GROWTH",
                    "0",
                ),
            ]
            .map(|(a, b)| (a.to_owned(), b.to_owned())),
        ))
        .unwrap();

        override_layout(&mut json_config, env_override).unwrap();
        let overridden_config = serde_json::to_value(json_config).unwrap();

        let expected_config: json::layout::Config = serde_json::from_str(
            r#"{
                "counters_order": [
                    "total_txns",
                    "total_blocks"
                ],
                "line_chart_categories": [
                    {
                        "id": "accounts",
                        "title": "Accounts",
                        "charts_order": [
                            "accounts_growth",
                            "new_accounts"
                        ]
                    },
                    {
                        "id": "transactions",
                        "title": "CoolTransactions",
                        "charts_order": [
                            "average_txn_fee",
                            "txns_fee"
                        ]
                    }
                ]
            }"#,
        )
        .unwrap();
        let expected_config = serde_json::to_value(expected_config).unwrap();

        assert_eq!(overridden_config, expected_config)
    }

    const EXAMPLE_LAYOUT_CONFIG_2: &str = r#"{
        "counters_order": [
            "total_txns",
            "total_blocks",
            "total_accounts"
        ],
        "line_chart_categories": [
            {
                "id": "transactions",
                "title": "Transactions",
                "charts_order": [
                    "average_txn_fee",
                    "txns_fee"
                ]
            },
            {
                "id": "blocks",
                "title": "Blocks",
                "charts_order": [
                    "total_blocks",
                    "blocks_growth"
                ]
            },
            {
                "id": "accounts",
                "title": "Accounts",
                "charts_order": [
                    "new_accounts",
                    "accounts_growth"
                ]
            }
        ]
    }"#;

    #[test]
    fn layout_order_is_preserved() {
        let mut json_config: json::layout::Config =
            serde_json::from_str(EXAMPLE_LAYOUT_CONFIG_2).unwrap();

        let env_override: env::layout::Config =
            config_from_env("STATS_LAYOUT", HashMap::new()).unwrap();

        override_layout(&mut json_config, env_override).unwrap();
        let overridden_config = serde_json::to_value(json_config).unwrap();

        let expected_config: json::layout::Config =
            serde_json::from_str(EXAMPLE_LAYOUT_CONFIG_2).unwrap();
        let expected_config = serde_json::to_value(expected_config).unwrap();

        assert_eq!(overridden_config, expected_config)
    }

    const EXAMPLE_SCHEDULE_CONFIG: &str = r#"{
        "schedules": {
            "average_block_time": "0 0 15 * * * *",
            "transactions": "0 0 7 * * * *",
            "new_transactions_only": "0 10 */3 * * * *"
        }
    }"#;

    #[test]
    fn schedule_overridden_correctly() {
        let mut json_config: json::update_groups::Config =
            serde_json::from_str(EXAMPLE_SCHEDULE_CONFIG).unwrap();

        let env_override: env::update_groups::Config = config_from_env(
            "STATS_UPDATE_GROUPS",
            HashMap::from_iter(
                [
                    (
                        "STATS_UPDATE_GROUPS__SCHEDULES__TRANSACTIONS",
                        "0 0 3 * * * *",
                    ),
                    (
                        "STATS_UPDATE_GROUPS__SCHEDULES__NEW_CONTRACTS",
                        "0 0 1 * * * *",
                    ),
                ]
                .map(|(a, b)| (a.to_owned(), b.to_owned())),
            ),
        )
        .unwrap();

        override_update_groups(&mut json_config, env_override).unwrap();
        let overridden_config = serde_json::to_value(json_config).unwrap();

        let expected_config: json::update_groups::Config = serde_json::from_str(
            r#"{
                "schedules": {
                    "average_block_time": "0 0 15 * * * *",
                    "transactions": "0 0 3 * * * *",
                    "new_transactions_only": "0 10 */3 * * * *",
                    "new_contracts": "0 0 1 * * * *"
                }
            }"#,
        )
        .unwrap();
        let expected_config = serde_json::to_value(expected_config).unwrap();

        assert_eq!(overridden_config, expected_config)
    }
}
