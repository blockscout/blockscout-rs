use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct ChainsSettings(BTreeMap<String, ChainSettings>);

impl ChainsSettings {
    pub fn new(path: Option<PathBuf>) -> Result<Self, anyhow::Error> {
        let mut builder = config::Config::builder();
        if let Some(path) = path {
            builder = builder.add_source(config::File::from(path))
        }
        builder = builder.add_source(
            config::Environment::with_prefix("PROXY_VERIFIER_CHAINS")
                .separator("__")
                .try_parsing(true),
        );

        let settings: Self = builder.build()?.try_deserialize()?;

        settings.validate()?;

        Ok(settings)
    }

    pub fn inner(&self) -> &BTreeMap<String, ChainSettings> {
        &self.0
    }

    pub fn into_inner(self) -> BTreeMap<String, ChainSettings> {
        self.0
    }

    fn validate(&self) -> anyhow::Result<()> {
        let chains_without_sensitive_api_key: Vec<_> = self
            .0
            .values()
            .filter(|&chain| chain.sensitive_api_key.is_none())
            .collect();

        if !chains_without_sensitive_api_key.is_empty() {
            anyhow::bail!(
                "Chains configuration contains values without sensitive api key specified: {chains_without_sensitive_api_key:?}"
            );
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChainSettings {
    pub name: String,
    pub api_url: url::Url,
    pub icon_url: Option<url::Url>,
    pub sensitive_api_key: Option<String>,
}
