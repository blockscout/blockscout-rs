use crate::config::{json, types::UpdateGroup};
use convert_case::{Case, Casing};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub update_groups: BTreeMap<String, UpdateGroup>,
}

impl From<json::update_schedule::Config> for Config {
    fn from(value: json::update_schedule::Config) -> Self {
        let update_groups_fixed_case = value
            .update_groups
            .into_iter()
            .map(|(name, group)| (name.from_case(Case::Snake).to_case(Case::Pascal), group))
            .collect();
        Self {
            update_groups: update_groups_fixed_case,
        }
    }
}
