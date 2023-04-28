use anyhow::Context;
use std::str::FromStr;
use url::Url;

pub struct Client {
    _api_url: Url,
}

impl Client {
    pub fn try_new(api_url: String) -> anyhow::Result<Self> {
        let api_url = Url::from_str(&api_url).context("invalid blockscout api url")?;
        Ok(Self { _api_url: api_url })
    }
}
