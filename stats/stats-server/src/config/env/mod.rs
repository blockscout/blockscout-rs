//! Basically the same as normal config but without lists.
//! Lists are currently not supported by `config` crate with environmental vars.
//!
//! Instead, we have the same items but with `order` field that defines relative position between them.

pub mod charts;
pub mod update_schedule;

#[cfg(test)]
pub mod test_utils {
    use pretty_assertions::Comparison;
    use std::collections::HashMap;

    pub fn config_from_env<Config>(values: HashMap<String, String>) -> anyhow::Result<Config>
    where
        Config: serde::de::DeserializeOwned + std::fmt::Debug + PartialEq,
    {
        let env_source = config::Environment::with_prefix("STATS_CHARTS")
            .separator("__")
            .try_parsing(true)
            .source(Some(values));
        Ok(config::Config::builder()
            .add_source(env_source)
            .build()
            .unwrap()
            .try_deserialize()?)
    }

    // returns result to see the line where panic happens
    /// env prefix "STATS_CHARTS" is assumed
    pub fn check_envs_parsed_to<T>(
        env_values: std::collections::HashMap<String, String>,
        expected: T,
    ) -> anyhow::Result<()>
    where
        T: serde::de::DeserializeOwned + std::fmt::Debug + PartialEq,
    {
        let config: T = config_from_env(env_values)?;
        if config != expected {
            return Err(anyhow::anyhow!(
                "Parsed config does not match expected. Left = parsed, right = expected: {}",
                Comparison::new(&config, &expected)
            ));
        }
        Ok(())
    }
}
