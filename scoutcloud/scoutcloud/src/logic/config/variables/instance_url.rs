use crate::logic::{
    config::{ParsedVariable, UserVariable},
    ParsedVariableKey,
};
use anyhow::Error;
use std::fmt::{Display, Formatter};

pub enum InstanceUrl {
    Host(String),
    Prefix(String),
}

impl Display for InstanceUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            InstanceUrl::Host(v) => write!(f, "{v}"),
            InstanceUrl::Prefix(v) => write!(f, "{v}.k8s-dev.blockscout.com"),
        }
    }
}

#[async_trait::async_trait]
impl UserVariable<String> for InstanceUrl {
    async fn build_config_vars(v: String) -> Result<Vec<ParsedVariable>, Error> {
        let v = if v.parse::<url::Url>().is_ok() {
            InstanceUrl::Host(v)
        } else {
            InstanceUrl::Prefix(v)
        };

        Ok(vec![(
            ParsedVariableKey::ConfigPath("blockscout.ingress.hostname".to_string()),
            serde_json::json!(v.to_string()),
        )])
    }
}
