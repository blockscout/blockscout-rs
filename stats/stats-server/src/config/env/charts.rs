use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::config::types::{AllChartSettings, ResolutionsSettings};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub counters: BTreeMap<String, ChartSettingsOverwrite>,
    pub line_charts: BTreeMap<String, ChartSettingsOverwrite>,
    pub template_values: BTreeMap<String, serde_json::Value>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ResolutionsEnabledOverwrite {
    day: Option<bool>,
    week: Option<bool>,
    month: Option<bool>,
    year: Option<bool>,
}

impl From<ResolutionsEnabledOverwrite> for ResolutionsSettings {
    fn from(value: ResolutionsEnabledOverwrite) -> Self {
        ResolutionsSettings {
            day: value.day,
            week: value.week,
            month: value.month,
            year: value.year,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ChartSettingsOverwrite {
    pub enabled: Option<bool>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub units: Option<String>,
    pub resolutions: ResolutionsEnabledOverwrite,
}

macro_rules! overwrite_fields {
    (
        target: $target:expr,
        with: $source:expr,
        fields: {
            $($field_name:ident),+ $(,)?
        }
    ) => {
        $(
            if let Some($field_name) = $source.$field_name {
                $target.$field_name = $field_name;
            }
        )+
    };
}

impl ChartSettingsOverwrite {
    pub fn apply_to(self, target: &mut AllChartSettings) {
        overwrite_fields!(
            target: target,
            with: self,
            fields: {
                enabled,
                title,
                description,
            }
        );
        target.units = self.units.or(target.units.take());
        target.resolutions.day = self.resolutions.day.or(target.resolutions.day);
        target.resolutions.week = self.resolutions.week.or(target.resolutions.week);
        target.resolutions.month = self.resolutions.month.or(target.resolutions.month);
        target.resolutions.year = self.resolutions.year.or(target.resolutions.year);
    }
}

impl TryFrom<ChartSettingsOverwrite> for AllChartSettings {
    type Error = anyhow::Error;

    fn try_from(value: ChartSettingsOverwrite) -> Result<Self, Self::Error> {
        match value {
            ChartSettingsOverwrite {
                enabled: Some(enabled),
                title: Some(title),
                description: Some(description),
                units,
                resolutions,
            } => Ok(AllChartSettings {
                enabled,
                title,
                description,
                units,
                resolutions: resolutions.into(),
            }),
            _ => {
                let mut missing_fields = vec![];
                if value.enabled.is_none() {
                    missing_fields.push("enabled");
                }
                if value.title.is_none() {
                    missing_fields.push("title");
                }
                if value.description.is_none() {
                    missing_fields.push("description");
                }
                Err(anyhow::anyhow!(
                    "Cannot overwrite missing item properties. Missing required fields: {:?}",
                    missing_fields
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::config::env::test_utils::check_envs_parsed_to;

    use super::*;

    // purpose of env is to overwrite some attributes in existing configurations
    // therefore it should be possible to do it a granular manner

    #[test]
    fn single_attribute_overwrite_works_for_template() {
        // purpose of env is to overwrite some attributes in existing configurations
        // therefore it should be possible to do it a granular manner
        check_envs_parsed_to(
            "STATS_CHARTS",
            [(
                "STATS_CHARTS__TEMPLATE_VALUES__NATIVE_COIN_SYMBOL".to_owned(),
                "USDT".to_owned(),
            )]
            .into(),
            Config {
                counters: BTreeMap::new(),
                line_charts: BTreeMap::new(),
                template_values: BTreeMap::from([(
                    "native_coin_symbol".to_owned(),
                    serde_json::Value::String("USDT".to_owned()),
                )]),
            },
        )
        .unwrap();
    }

    #[test]
    fn single_attribute_overwrite_works_for_line_charts() {
        check_envs_parsed_to(
            "STATS_CHARTS",
            [(
                "STATS_CHARTS__LINE_CHARTS__AVERAGE_TXN_FEE__DESCRIPTION".to_owned(),
                "Some runtime-overwritten description".to_owned(),
            )]
            .into(),
            Config {
                counters: BTreeMap::new(),
                line_charts: BTreeMap::from([(
                    "average_txn_fee".to_owned(),
                    ChartSettingsOverwrite {
                        enabled: None,
                        title: None,
                        description: Some("Some runtime-overwritten description".to_owned()),
                        units: None,
                        resolutions: ResolutionsEnabledOverwrite::default(),
                    },
                )]),
                template_values: BTreeMap::new(),
            },
        )
        .unwrap();

        check_envs_parsed_to(
            "STATS_CHARTS",
            [(
                "STATS_CHARTS__LINE_CHARTS__AVERAGE_TXN_FEE__ENABLED".to_owned(),
                "true".to_owned(),
            )]
            .into(),
            Config {
                counters: BTreeMap::new(),
                line_charts: BTreeMap::from([(
                    "average_txn_fee".to_owned(),
                    ChartSettingsOverwrite {
                        enabled: Some(true),
                        title: None,
                        description: None,
                        units: None,
                        resolutions: ResolutionsEnabledOverwrite::default(),
                    },
                )]),
                template_values: BTreeMap::new(),
            },
        )
        .unwrap();

        check_envs_parsed_to(
            "STATS_CHARTS",
            [(
                "STATS_CHARTS__LINE_CHARTS__AVERAGE_TXN_FEE__RESOLUTIONS__DAY".to_owned(),
                "true".to_owned(),
            )]
            .into(),
            Config {
                counters: BTreeMap::new(),
                line_charts: BTreeMap::from([(
                    "average_txn_fee".to_owned(),
                    ChartSettingsOverwrite {
                        enabled: None,
                        title: None,
                        description: None,
                        units: None,
                        resolutions: ResolutionsEnabledOverwrite {
                            day: Some(true),
                            week: None,
                            month: None,
                            year: None,
                        },
                    },
                )]),
                template_values: BTreeMap::new(),
            },
        )
        .unwrap();
    }

    #[test]
    fn single_attribute_overwrite_works_for_counters() {
        check_envs_parsed_to(
            "STATS_CHARTS",
            [(
                "STATS_CHARTS__COUNTERS__AVERAGE_BLOCK_TIME__ENABLED".to_owned(),
                "true".to_owned(),
            )]
            .into(),
            Config {
                counters: BTreeMap::from([(
                    "average_block_time".to_owned(),
                    ChartSettingsOverwrite {
                        enabled: Some(true),
                        title: None,
                        description: None,
                        units: None,
                        resolutions: ResolutionsEnabledOverwrite::default(),
                    },
                )]),
                line_charts: BTreeMap::new(),
                template_values: BTreeMap::new(),
            },
        )
        .unwrap();
    }

    #[test]
    fn arbitrary_envs_parsed_correctly() {
        let envs: HashMap<_, _> = [
            ("STATS_CHARTS__TEMPLATE_VALUES__NATIVE_COIN_SYMBOL", "USDC"),
            (
                "STATS_CHARTS__COUNTERS__AVERAGE_BLOCK_TIME__TITLE",
                "Average block time",
            ),
            (
                "STATS_CHARTS__COUNTERS__AVERAGE_BLOCK_TIME__ENABLED",
                "true",
            ),
            (
                "STATS_CHARTS__COUNTERS__AVERAGE_BLOCK_TIME__DESCRIPTION",
                "Some description kek",
            ),
            ("STATS_CHARTS__COUNTERS__AVERAGE_BLOCK_TIME__UNITS", "s"),
            (
                "STATS_CHARTS__LINE_CHARTS__AVERAGE_TXN_FEE__DESCRIPTION",
                "Some runtime-overwritten description",
            ),
            (
                "STATS_CHARTS__LINE_CHARTS__AVERAGE_TXN_FEE__ENABLED",
                "false",
            ),
        ]
        .map(|(s1, s2)| (s1.to_owned(), s2.to_owned()))
        .into();

        let expected_counter = ChartSettingsOverwrite {
            enabled: Some(true),
            title: Some("Average block time".to_owned()),
            description: Some("Some description kek".to_owned()),
            units: Some("s".to_owned()),
            resolutions: ResolutionsEnabledOverwrite::default(),
        };
        let expected_line_category = ChartSettingsOverwrite {
            enabled: Some(false),
            title: None,
            description: Some("Some runtime-overwritten description".to_owned()),
            units: None,

            resolutions: ResolutionsEnabledOverwrite::default(),
        };

        check_envs_parsed_to(
            "STATS_CHARTS",
            envs,
            Config {
                counters: BTreeMap::from([("average_block_time".to_owned(), expected_counter)]),
                line_charts: BTreeMap::from([(
                    "average_txn_fee".to_owned(),
                    expected_line_category,
                )]),
                template_values: BTreeMap::from([(
                    "native_coin_symbol".to_owned(),
                    serde_json::Value::String("USDC".to_owned()),
                )]),
            },
        )
        .unwrap();
    }
}
