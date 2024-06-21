use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::config::types::UpdateSchedule;

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub schedules: BTreeMap<String, UpdateSchedule>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use cron::Schedule;

    use super::*;
    use crate::config::env::test_utils::check_envs_parsed_to;

    #[test]
    fn single_attribute_overwrite_works() {
        check_envs_parsed_to(
            [(
                "STATS_CHARTS__SCHEDULES__AVERAGE_BLOCK_TIME".to_owned(),
                "0 0 15 * * * *".to_owned(),
            )]
            .into(),
            Config {
                schedules: BTreeMap::from([(
                    "average_block_time".to_owned(),
                    UpdateSchedule {
                        update_schedule: Schedule::from_str("0 0 15 * * * *").unwrap(),
                    },
                )]),
            },
        )
        .unwrap();
    }
}
