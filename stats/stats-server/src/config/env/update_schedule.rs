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
    // weird design because it's env...
    /// true - ignore;
    /// false - leave as-is
    pub ignore_charts: BTreeMap<String, bool>,
}

impl From<UpdateGroup> for crate::config::chart_info::UpdateGroup {
    fn from(value: UpdateGroup) -> Self {
        Self {
            update_schedule: value.update_schedule,
            ignore_charts: value
                .ignore_charts
                .into_iter()
                .filter(|(_, ignore)| *ignore)
                .map(|(name, _)| name)
                .collect(),
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
                update_groups: BTreeMap::from_iter([(
                    "average_block_time".to_owned(),
                    UpdateGroup {
                        update_schedule: Some(Schedule::from_str("0 0 15 * * * *").unwrap()),
                        ignore_charts: BTreeMap::new(),
                    },
                )]),
            },
        )
        .unwrap();

        check_envs_parsed_to(
            [(
                "STATS_CHARTS__UPDATE_GROUPS__AVERAGE_BLOCK_TIME__IGNORE_CHARTS__SOME_POOR_CHART"
                    .to_owned(),
                "1".to_owned(),
            )]
            .into(),
            Config {
                update_groups: BTreeMap::from_iter([(
                    "average_block_time".to_owned(),
                    UpdateGroup {
                        update_schedule: None,
                        ignore_charts: BTreeMap::from_iter([("some_poor_chart".to_owned(), true)]),
                    },
                )]),
            },
        )
        .unwrap();
    }
}
