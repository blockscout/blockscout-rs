//! Actually reading configs.
//!
//! Currently the configs are read from json files. Values can be overridden with env variables
//! for convenience.

use merge::{override_charts, override_update_schedule};
use serde::{de::DeserializeOwned, Serialize};

use super::{env, json, types::AllChartSettings};
use std::path::Path;

pub mod charts;
mod merge;
pub mod update_schedule;

fn read_json_override_from_env_config<JsonConfig, EnvConfig>(
    json_path: &Path,
    env_prefix: &'static str,
    override_fn: impl FnOnce(&mut JsonConfig, EnvConfig) -> Result<(), anyhow::Error>,
) -> Result<JsonConfig, anyhow::Error>
where
    JsonConfig: Serialize + DeserializeOwned,
    EnvConfig: Serialize + DeserializeOwned,
{
    let extension = json_path.extension();
    if extension == Some(std::ffi::OsStr::new("json")) {
        let mut json_config: JsonConfig = config::Config::builder()
            .add_source(config::File::from(json_path))
            .build()?
            .try_deserialize()?;
        let env_config: EnvConfig = config::Config::builder()
            .add_source(
                config::Environment::with_prefix(env_prefix)
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()?;
        override_fn(&mut json_config, env_config)?;
        Ok(json_config)
    } else {
        Err(anyhow::anyhow!(
            "expected `.json`, got invalid config extension: {extension:?}"
        ))
    }
}

pub fn read_charts_config(path: &Path) -> Result<charts::Config<AllChartSettings>, anyhow::Error> {
    let overridden_json_config = read_json_override_from_env_config::<
        json::charts::Config,
        env::charts::Config,
    >(path, "STATS_CHARTS", override_charts)?;
    let rendered_config = overridden_json_config.render_with_template_values()?;
    Ok(rendered_config.into())
}

pub fn read_update_schedule_config(path: &Path) -> Result<update_schedule::Config, anyhow::Error> {
    let overridden_json_config = read_json_override_from_env_config::<
        json::update_schedule::Config,
        env::update_schedule::Config,
    >(path, "STATS_CHARTS", override_update_schedule)?;
    Ok(overridden_json_config.into())
}
