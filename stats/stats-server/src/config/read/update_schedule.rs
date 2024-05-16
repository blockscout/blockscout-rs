use crate::config::{chart_info::UpdateGroup, json};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub update_groups: BTreeMap<String, UpdateGroup>,
}

impl From<json::update_schedule::Config> for Config {
    fn from(value: json::update_schedule::Config) -> Self {
        Self {
            update_groups: value.update_groups,
        }
    }
}
