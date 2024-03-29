use crate::logic::config::{macros, Error};
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

macros::custom_env_var!(
    InstanceUrl,
    String,
    config,
    "blockscout.ingress.hostname",
    None,
    {
        fn new(v: String) -> Result<Self, Error> {
            if v.parse::<url::Url>().is_ok() {
                Ok(Self::Host(v))
            } else {
                Ok(Self::Prefix(v))
            }
        }
    }
);
