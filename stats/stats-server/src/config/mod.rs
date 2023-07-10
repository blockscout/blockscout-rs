mod chart_info;
pub mod json_config;
pub mod toml_config;

pub use chart_info::ChartSettings;
use std::path::PathBuf;

pub fn read_charts_config(path: PathBuf) -> Result<toml_config::Config, anyhow::Error> {
    let extension = path.extension();
    if extension == Some(std::ffi::OsStr::new("json")) {
        let json_config: json_config::Config = config::Config::builder()
            .add_source(config::File::from(path))
            .add_source(
                config::Environment::with_prefix("STATS_CFG")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()?;
        Ok(json_config.into())
    } else if extension == Some(std::ffi::OsStr::new("toml")) {
        let toml_config = std::fs::read(path)?;
        let toml_config: toml_config::Config = toml::from_slice(&toml_config)?;

        Ok(toml_config)
    } else {
        Err(anyhow::anyhow!(
            "invalid chart config extension: {extension:?}"
        ))
    }
}
