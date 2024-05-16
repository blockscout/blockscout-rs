use crate::config::chart_info::UpdateGroup;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub update_groups: BTreeMap<String, UpdateGroup>,
}
