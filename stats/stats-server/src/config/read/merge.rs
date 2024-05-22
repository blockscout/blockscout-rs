use anyhow::Context;

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
                target_group.update_schedule = target_group
                    .update_schedule
                    .take()
                    .or(added_settings.update_schedule);

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
