use crate::config::types::UpdateSchedule;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub schedules: BTreeMap<String, UpdateSchedule>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use cron::Schedule;
    use pretty_assertions::assert_eq;

    const EXAMPLE_CONFIG: &str = r#"{
        "schedules": {
            "average_block_time": "0 0 15 * * * *",
            "transactions":"0 0 7 * * * *",
            "new_transactions_only":"0 10 */3 * * * *"
        }
    }"#;

    #[test]
    fn config_parses_correctly() {
        let config: Config = serde_json::from_str(EXAMPLE_CONFIG).expect("should be valid config");

        assert_eq!(
            config,
            Config {
                schedules: BTreeMap::from([
                    (
                        "average_block_time".to_owned(),
                        UpdateSchedule {
                            update_schedule: Schedule::from_str("0 0 15 * * * *").unwrap()
                        }
                    ),
                    (
                        "transactions".to_owned(),
                        UpdateSchedule {
                            update_schedule: Schedule::from_str("0 0 7 * * * *").unwrap()
                        }
                    ),
                    (
                        "new_transactions_only".to_owned(),
                        UpdateSchedule {
                            update_schedule: Schedule::from_str("0 10 */3 * * * *").unwrap()
                        }
                    ),
                ])
            }
        )
    }
}
