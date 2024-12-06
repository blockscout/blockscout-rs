use std::collections::BTreeMap;

use crate::config::{json, types::LineChartCategory};
use convert_case::{Case, Casing};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub counters_order: Vec<String>,
    pub line_chart_categories: Vec<LineChartCategory>,
}

impl From<json::layout::Config> for Config {
    fn from(value: json::layout::Config) -> Self {
        let mut line_chart_categories = value.line_chart_categories;
        for cat in line_chart_categories.iter_mut() {
            for chart_name in cat.charts_order.iter_mut() {
                *chart_name = chart_name.from_case(Case::Snake).to_case(Case::Camel)
            }
        }
        let counters_order = value
            .counters_order
            .into_iter()
            .map(|id| id.from_case(Case::Snake).to_case(Case::Camel))
            .collect();
        Self {
            counters_order,
            line_chart_categories,
        }
    }
}

/// Arranges the items `to_sort` according to order in `layout`.
///
/// `push_missing_items_back`:
/// - if `true`, then items not present in
///     `layout` are placed at the end of the vector in their original
///     relative order.
/// - if `false` - at the beginning with the same logic.
pub fn sorted_items_according_to_layout<Item, Key, F>(
    to_sort: Vec<Item>,
    layout: &[Key],
    get_key: F,
    push_missing_items_back: bool,
) -> Vec<Item>
where
    Key: Ord,
    F: Fn(&Item) -> &Key,
{
    let assigned_positions: BTreeMap<_, _> = layout
        .iter()
        .enumerate()
        .map(|(pos, key)| (key, pos))
        .collect();
    let mut to_sort_with_new_positions: Vec<_> = to_sort
        .into_iter()
        .map(|item| {
            let mut key = assigned_positions.get(get_key(&item)).copied();
            if push_missing_items_back {
                key.get_or_insert(usize::MAX);
            }
            (item, key)
        })
        .collect();
    to_sort_with_new_positions.sort_by_key(|p| p.1);
    to_sort_with_new_positions
        .into_iter()
        .map(|p| p.0)
        .collect()
}
