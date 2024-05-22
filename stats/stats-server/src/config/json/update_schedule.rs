use crate::config::chart_info::UpdateGroup;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub update_groups: BTreeMap<String, UpdateGroup>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use cron::Schedule;
    use pretty_assertions::assert_eq;

    const EXAMPLE_CONFIG: &'static str = r#"{
        "update_groups": {
            "average_block_time": {
                "update_schedule": "0 0 15 * * * *"
            },
            "transactions": {
                "update_schedule": "0 0 7 * * * *",
                "ignore_charts": [
                    "total_txns"
                ]
            },
            "new_transactions_only": {
                "update_schedule": "0 10 */3 * * * *",
                "ignore_charts": [ ]
            }
        }
    }"#;

    #[test]
    fn config_parses_correctly() {
        let config: Config = serde_json::from_str(&EXAMPLE_CONFIG).expect("should be valid config");

        assert_eq!(
            config,
            Config {
                update_groups: BTreeMap::from_iter([
                    (
                        "average_block_time".to_owned(),
                        UpdateGroup {
                            update_schedule: Some(Schedule::from_str("0 0 15 * * * *").unwrap()),
                            ignore_charts: vec![]
                        }
                    ),
                    (
                        "transactions".to_owned(),
                        UpdateGroup {
                            update_schedule: Some(Schedule::from_str("0 0 7 * * * *").unwrap()),
                            ignore_charts: vec!["total_txns".to_owned()]
                        }
                    ),
                    (
                        "new_transactions_only".to_owned(),
                        UpdateGroup {
                            update_schedule: Some(Schedule::from_str("0 10 */3 * * * *").unwrap()),
                            ignore_charts: vec![]
                        }
                    ),
                ])
            }
        )
    }
}
