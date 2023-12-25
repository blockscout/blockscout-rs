use std::str::FromStr;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Config {
    url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ConfigBuilder {
    url: String,
}

impl Config {
    pub fn builder(url: String) -> ConfigBuilder {
        ConfigBuilder { url }
    }
}

impl ConfigBuilder {
    pub fn build(self) -> Config {
        Config { url: self.url }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct ValidatedConfig {
    pub url: url::Url,
}

impl TryFrom<Config> for ValidatedConfig {
    type Error = BoxError;

    fn try_from(value: Config) -> std::result::Result<Self, Self::Error> {
        let url = url::Url::from_str(&value.url)?;

        Ok(Self { url })
    }
}
