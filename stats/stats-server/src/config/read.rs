use super::{json_config, toml_config};
use std::path::Path;

pub fn read_charts_config(path: &Path) -> Result<toml_config::Config, anyhow::Error> {
    let extension = path.extension();
    if extension == Some(std::ffi::OsStr::new("json")) {
        let json_config: json_config::Config = config::Config::builder()
            .add_source(config::File::from(path))
            .add_source(
                config::Environment::with_prefix("STATS_CHARTS")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()?;
        let json_config = json_config.render_with_template_values()?;
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
