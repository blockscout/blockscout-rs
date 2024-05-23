//! Actually reading configs.
//! Currently the configs are read from json files. Values can be overridden with env variables
//! for convenience.

use merge::{override_charts, override_update_schedule};

use super::{chart_info::AllChartSettings, env, json};
use std::path::Path;

pub mod charts;
mod merge;
pub mod update_schedule;

// todo: deduplicate
pub fn read_charts_config(path: &Path) -> Result<charts::Config<AllChartSettings>, anyhow::Error> {
    let extension = path.extension();
    if extension == Some(std::ffi::OsStr::new("json")) {
        let mut json_config: json::charts::Config = config::Config::builder()
            .add_source(config::File::from(path))
            .build()?
            .try_deserialize()?;
        let env_config: env::charts::Config = config::Config::builder()
            .add_source(
                config::Environment::with_prefix("STATS_CHARTS")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()?;
        override_charts(&mut json_config, env_config)?;
        let json_config = json_config.render_with_template_values()?;
        Ok(json_config.into())
    } else {
        Err(anyhow::anyhow!(
            "invalid chart config extension: {extension:?}"
        ))
    }
}

pub fn read_update_schedule_config(path: &Path) -> Result<update_schedule::Config, anyhow::Error> {
    let extension = path.extension();
    if extension == Some(std::ffi::OsStr::new("json")) {
        let mut json_config: json::update_schedule::Config = config::Config::builder()
            .add_source(config::File::from(path))
            .build()?
            .try_deserialize()?;
        let env_config: env::update_schedule::Config = config::Config::builder()
            .add_source(
                config::Environment::with_prefix("STATS_CHARTS")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()?;
        override_update_schedule(&mut json_config, env_config)?;
        Ok(json_config.into())
    } else {
        Err(anyhow::anyhow!(
            "invalid chart config extension: {extension:?}"
        ))
    }
}
