//! Basically the same as normal config but without lists.
//! Lists are currently not supported by `config` crate with environmental vars.
//!
//! Instead, we have the same items but with `order` field that defines relative position between them.

pub mod charts;
pub mod update_schedule;

/// env prefix "STATS_CHARTS" is assumed
#[cfg(test)]
fn assert_envs_parsed_to<T>(env_values: std::collections::HashMap<String, String>, expected: T)
where
    T: serde::de::DeserializeOwned + std::fmt::Debug + PartialEq,
{
    let env_source = config::Environment::with_prefix("STATS_CHARTS")
        .separator("__")
        .try_parsing(true)
        .source(Some(env_values));
    let config: T = config::Config::builder()
        .add_source(env_source)
        .build()
        .unwrap()
        .try_deserialize()
        .unwrap();
    assert_eq!(config, expected)
}
