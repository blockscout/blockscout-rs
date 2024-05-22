use anyhow::Context;
use itertools::Itertools;

use crate::config::{
    chart_info::{CounterInfo, LineChartCategory, LineChartInfo},
    env::{
        self,
        charts::{CounterInfoOrdered, LineChartCategoryOrdered, LineChartInfoOrdered},
    },
    json,
};
use std::collections::{btree_map::Entry, BTreeMap};

trait GetOrder {
    fn order(&self) -> Option<usize>;
}

macro_rules! impl_get_order {
    ($type:ident) => {
        impl GetOrder for $type {
            fn order(&self) -> Option<usize> {
                self.order
            }
        }
    };
}

impl_get_order!(CounterInfoOrdered);
impl_get_order!(LineChartInfoOrdered);
impl_get_order!(LineChartCategoryOrdered);

trait GetKey {
    fn key(&self) -> &str;
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
impl_get_key!(LineChartCategory<C>);

/// `overwrite_f` should update 1st parameter with values from the 2nd
fn override_ordered<T, S, F>(
    target: &mut Vec<T>,
    source: BTreeMap<String, S>,
    update_t: F,
) -> Result<(), anyhow::Error>
where
    T: GetKey,
    S: GetOrder,
    F: Fn(&mut T, S) -> Result<(), anyhow::Error>,
{
    let mut target_with_order: BTreeMap<String, (usize, T)> = std::mem::take(target)
        .into_iter()
        .enumerate()
        .map(|(i, t)| (t.key().to_owned(), (i, t)))
        .collect();
    for (key, val_with_order) in source {
        let Some((target_order, target_val)) = target_with_order.get_mut(&key) else {
            return Err(anyhow::anyhow!("Unknown key: {}", key));
        };
        if let Some(order_override) = val_with_order.order() {
            *target_order = order_override;
        }
        update_t(target_val, val_with_order)
            .context(format!("updating values for key: {}", key))?;
    }
    let mut target_with_order = target_with_order.into_values().collect_vec();
    target_with_order.sort_by_key(|t| t.0);
    *target = target_with_order.into_iter().map(|t| t.1).collect();
    Ok(())
}

mod override_field {
    use super::*;

    use crate::config::{
        chart_info::{AllChartSettings, CounterInfo, LineChartCategory, LineChartInfo},
        env::charts::{CounterInfoOrdered, LineChartCategoryOrdered, LineChartInfoOrdered},
    };

    pub fn counter(
        target: &mut CounterInfo<AllChartSettings>,
        source: CounterInfoOrdered,
    ) -> Result<(), anyhow::Error> {
        source.settings.apply_to(&mut target.settings);
        Ok(())
    }

    pub fn line_chart_info(
        target: &mut LineChartInfo<AllChartSettings>,
        source: LineChartInfoOrdered,
    ) -> Result<(), anyhow::Error> {
        source.settings.apply_to(&mut target.settings);
        Ok(())
    }

    pub fn line_categories(
        target: &mut LineChartCategory<AllChartSettings>,
        source: LineChartCategoryOrdered,
    ) -> Result<(), anyhow::Error> {
        if let Some(title) = source.title {
            *(&mut target.title) = title;
        }
        override_ordered(&mut target.charts, source.charts, line_chart_info)
            .context("updating charts in category")
    }
}

/// Prioritize values from environment
pub fn override_charts(
    target: &mut json::charts::Config,
    source: env::charts::Config,
) -> Result<(), anyhow::Error> {
    override_ordered(
        &mut target.counters,
        source.counters,
        override_field::counter,
    )
    .context("updating counters")?;
    override_ordered(
        &mut target.line_categories,
        source.line_categories,
        override_field::line_categories,
    )
    .context("updating line categories")?;
    target
        .template_values
        .extend(source.template_values.into_iter());
    Ok(())
}

/// Prioritize values from environment
pub fn override_update_schedule(
    target: &mut json::update_schedule::Config,
    source: env::update_schedule::Config,
) -> Result<(), anyhow::Error> {
    for (group_name, added_settings) in source.update_groups {
        match target.update_groups.entry(group_name) {
            Entry::Vacant(v) => {
                v.insert(added_settings.into());
            }
            Entry::Occupied(mut o) => {
                let target_group = o.get_mut();
                target_group.update_schedule = added_settings
                    .update_schedule
                    .or(target_group.update_schedule.take());

                for (chart_name, new_ignore_status) in added_settings.ignore_charts {
                    if new_ignore_status == true {
                        target_group.ignore_charts.insert(chart_name);
                    } else {
                        target_group.ignore_charts.remove(&chart_name);
                    }
                }
            }
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

    const EXAMPLE_CHART_CONFIG: &'static str = r#"{
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
    fn charts_overridden_correctly() {
        let mut json_config: json::charts::Config =
            serde_json::from_str(&EXAMPLE_CHART_CONFIG).unwrap();

        let env_override: env::charts::Config = config_from_env(HashMap::from_iter(
            [
                ("STATS_CHARTS__TEMPLATE_VALUES__NATIVE_COIN_SYMBOL", "USDC"),
                ("STATS_CHARTS__COUNTERS__TOTAL_BLOCKS__ENABLED", "false"),
                ("STATS_CHARTS__COUNTERS__TOTAL_TXNS__ENABLED", "true"),
                (
                    "STATS_CHARTS__LINE_CATEGORIES__ACCOUNTS__CHARTS__TXNS_FEE__UNITS",
                    "k USDC",
                ),
            ]
            .map(|(a, b)| (a.to_owned(), b.to_owned())),
        ))
        .unwrap();

        override_charts(&mut json_config, env_override).unwrap();
        let overridden_config = serde_json::to_value(json_config).unwrap();

        let expected_config: json::charts::Config = serde_json::from_str(r#"{
            "counters": [
                {
                    "id": "total_blocks",
                    "title": "Total Blocks",
                    "description": "Number of all blocks in the network",
                    "units": "blocks",
                    "enabled": false
                },
                {
                    "id": "total_txns",
                    "enabled": true,
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
                            "units": "k USDC"
                        }
                    ]
                }
            ],
            "template_values": {
                "native_coin_symbol": "USDC"
            }
        }"#).unwrap();
        let expected_config = serde_json::to_value(expected_config).unwrap();

        assert_eq!(overridden_config, expected_config)
    }

    const EXAMPLE_SCHEDULE_CONFIG: &'static str = r#"{
        "update_groups": {
            "average_block_time": {
                "update_schedule": "0 0 15 * * * *"
            },
            "transactions": {
                "update_schedule": "0 0 7 * * * *",
                "ignore_charts": [
                    "total_txns",
                    "average_txn_fee"
                ]
            },
            "new_transactions_only": {
                "update_schedule": "0 10 */3 * * * *",
                "ignore_charts": [ ]
            }
        }
    }"#;

    #[test]
    fn schedule_overridden_correctly() {
        let mut json_config: json::update_schedule::Config =
            serde_json::from_str(&EXAMPLE_SCHEDULE_CONFIG).unwrap();

        let env_override: env::update_schedule::Config = config_from_env(HashMap::from_iter(
            [
                ("STATS_CHARTS__UPDATE_GROUPS__AVERAGE_BLOCK_TIME__IGNORE_CHARTS__AVERAGE_BLOCK_TIME", "true"),
                ("STATS_CHARTS__UPDATE_GROUPS__TRANSACTIONS__IGNORE_CHARTS__AVERAGE_TXN_FEE", "false"),
                ("STATS_CHARTS__UPDATE_GROUPS__TRANSACTIONS__UPDATE_SCHEDULE", "0 0 3 * * * *"),
                ("STATS_CHARTS__UPDATE_GROUPS__NEW_CONTRACTS__UPDATE_SCHEDULE", "0 0 1 * * * *"),
            ]
            .map(|(a, b)| (a.to_owned(), b.to_owned())),
        ))
        .unwrap();

        override_update_schedule(&mut json_config, env_override).unwrap();
        let overridden_config = serde_json::to_value(json_config).unwrap();

        let expected_config: json::update_schedule::Config = serde_json::from_str(
            r#"{
            "update_groups": {
                "average_block_time": {
                    "update_schedule": "0 0 15 * * * *",
                    "ignore_charts": [
                        "average_block_time"
                    ]
                },
                "transactions": {
                    "update_schedule": "0 0 3 * * * *",
                    "ignore_charts": [
                        "total_txns"
                    ]
                },
                "new_transactions_only": {
                    "update_schedule": "0 10 */3 * * * *",
                    "ignore_charts": [ ]
                },
                "new_contracts": {
                    "update_schedule": "0 0 1 * * * *",
                    "ignore_charts": [ ]
                }
            }
        }"#,
        )
        .unwrap();
        let expected_config = serde_json::to_value(expected_config).unwrap();

        assert_eq!(overridden_config, expected_config)
    }
}
