use crate::logic::{json_utils, ConfigError};
use anyhow::Context;
use scoutcloud_proto::blockscout::scoutcloud::v1::{
    DeployConfigInternal, DeployConfigPartialInternal,
};

#[derive(Clone, Debug)]
pub struct UserConfig {
    pub internal: DeployConfigInternal,
}

impl From<DeployConfigInternal> for UserConfig {
    fn from(internal: DeployConfigInternal) -> Self {
        Self::new(internal)
    }
}

impl UserConfig {
    pub fn new(internal: DeployConfigInternal) -> Self {
        Self { internal }
    }

    pub fn raw(&self) -> Result<serde_json::Value, ConfigError> {
        let raw = serde_json::to_value(&self.internal).context("serializing config")?;
        Ok(raw)
    }

    pub fn parse(json: serde_json::Value) -> Result<Self, ConfigError> {
        let internal: DeployConfigInternal =
            serde_json::from_value(json).context("parsing existing config")?;
        Ok(internal.into())
    }

    pub fn with_merged_partial(
        self,
        partial: &DeployConfigPartialInternal,
    ) -> Result<Self, ConfigError> {
        let mut this = self.raw()?;
        let mut other = serde_json::to_value(partial).context("serializing partial config")?;
        json_utils::filter_null_values(&mut other);
        json_utils::merge(&mut this, &other);
        Self::parse(this)
    }
}
