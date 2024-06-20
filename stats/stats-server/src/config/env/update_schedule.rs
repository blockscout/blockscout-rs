use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::collections::BTreeMap;

#[serde_as]
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct UpdateGroup {
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub update_schedule: Option<Schedule>,
}

impl From<UpdateGroup> for crate::config::types::UpdateGroup {
    fn from(value: UpdateGroup) -> Self {
        Self {
            update_schedule: value.update_schedule,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub update_groups: BTreeMap<String, UpdateGroup>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::config::env::test_utils::check_envs_parsed_to;

    #[test]
    fn single_attribute_overwrite_works() {
        check_envs_parsed_to(
            [(
                "STATS_CHARTS__UPDATE_GROUPS__AVERAGE_BLOCK_TIME__UPDATE_SCHEDULE".to_owned(),
                "0 0 15 * * * *".to_owned(),
            )]
            .into(),
            Config {
                update_groups: BTreeMap::from([(
                    "average_block_time".to_owned(),
                    UpdateGroup {
                        update_schedule: Some(Schedule::from_str("0 0 15 * * * *").unwrap()),
                    },
                )]),
            },
        )
        .unwrap();
    }
}
