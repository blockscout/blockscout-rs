use crate::config::{json, types::UpdateSchedule};
use convert_case::{Case, Casing};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub schedules: BTreeMap<String, UpdateSchedule>,
}

impl From<json::update_groups::Config> for Config {
    fn from(value: json::update_groups::Config) -> Self {
        let schedules_fixed_case = value
            .schedules
            .into_iter()
            .map(|(name, group)| (name.from_case(Case::Snake).to_case(Case::Pascal), group))
            .collect();
        Self {
            schedules: schedules_fixed_case,
        }
    }
}
