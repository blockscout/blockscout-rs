use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::collections::BTreeMap;

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateGroup {
    pub title: String,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub update_schedule: Option<Schedule>,
    pub charts: BTreeMap<String, ()>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub update_groups: BTreeMap<String, UpdateGroup>,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_config() -> Result<(), config::ConfigError> {
        let group_name = "GR1";
        let chart_name = "CH1";
        let source = config::Environment::with_prefix("STATS_CHARTS")
            .separator("__")
            .try_parsing(true)
            .source(Some({
                let mut env = HashMap::new();
                env.insert(
                    format!("STATS_CHARTS__UPDATE_GROUPS__{group_name}__UPDATE_SCHEDULE").into(),
                    "0 0 15 * * * *".into(),
                );
                env.insert(
                    format!("STATS_CHARTS__UPDATE_GROUPS__{group_name}__CHARTS__{chart_name}")
                        .into(),
                    "".into(),
                );
                env
            }));

        let config: Config = config::Config::builder()
            .add_source(source)
            .build()?
            .try_deserialize()
            .unwrap();
        assert!(config
            .update_groups
            .get(group_name)
            .unwrap()
            .charts
            .contains_key(chart_name));

        Ok(())
    }
}
