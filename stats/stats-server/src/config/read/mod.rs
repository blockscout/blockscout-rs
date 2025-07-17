//! Actually reading configs.
//!
//! Currently the configs are read from json files. Values can be overridden with env variables
//! for convenience.

use anyhow::Context;
use merge::{override_charts, override_layout, override_update_groups};
use serde::{Serialize, de::DeserializeOwned};

use super::{env, json, types::AllChartSettings};
use std::path::PathBuf;

pub mod charts;
pub mod layout;
mod merge;
pub mod update_groups;

fn read_json_override_from_env_config<JsonConfig, EnvConfig>(
    json_paths: &[PathBuf],
    env_prefix: &'static str,
    override_fn: impl FnOnce(&mut JsonConfig, EnvConfig) -> Result<(), anyhow::Error>,
) -> Result<JsonConfig, anyhow::Error>
where
    JsonConfig: Serialize + DeserializeOwned,
    EnvConfig: Serialize + DeserializeOwned,
{
    let mut builder = config::Config::builder();
    for json_path in json_paths {
        let extension = json_path.extension();
        if extension == Some(std::ffi::OsStr::new("json")) {
            builder = builder.add_source(config::File::from(json_path.as_path()));
        } else {
            return Err(anyhow::anyhow!(
                "expected `.json`, got invalid config extension: {extension:?}"
            ));
        };
    }
    let mut json_config: JsonConfig = builder
        .build()
        .context("json config read")?
        .try_deserialize()
        .context("json parse")?;
    let env_config: EnvConfig = config::Config::builder()
        .add_source(
            config::Environment::with_prefix(env_prefix)
                .separator("__")
                .try_parsing(true),
        )
        .build()
        .context("envs read")?
        .try_deserialize()
        .context("envs parse")?;
    override_fn(&mut json_config, env_config).context("overriding values")?;
    Ok(json_config)
}

pub fn read_charts_config(
    paths: &[PathBuf],
) -> Result<charts::Config<AllChartSettings>, anyhow::Error> {
    let overridden_json_config = read_json_override_from_env_config::<
        json::charts::Config,
        env::charts::Config,
    >(paths, "STATS_CHARTS", override_charts)
    .context("charts config")?;
    let rendered_config = overridden_json_config
        .render_with_template_values()
        .context("rendering charts config")?;
    Ok(rendered_config.into())
}

pub fn read_layout_config(paths: &[PathBuf]) -> Result<layout::Config, anyhow::Error> {
    let overridden_json_config = read_json_override_from_env_config::<
        json::layout::Config,
        env::layout::Config,
    >(paths, "STATS_LAYOUT", override_layout)
    .context("layout config")?;
    Ok(overridden_json_config.into())
}

pub fn read_update_groups_config(
    paths: &[PathBuf],
) -> Result<update_groups::Config, anyhow::Error> {
    let overridden_json_config = read_json_override_from_env_config::<
        json::update_groups::Config,
        env::update_groups::Config,
    >(paths, "STATS_UPDATE_GROUPS", override_update_groups)
    .context("update groups config")?;
    Ok(overridden_json_config.into())
}
