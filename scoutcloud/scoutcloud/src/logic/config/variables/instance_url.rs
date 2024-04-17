use crate::logic::{
    config::{macros, ConfigError},
    ConfigValidationContext,
};
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
    [
        (ConfigPath, "blockscout.ingress.hostname"),
        (ConfigPath, "frontend.ingress.hostname")
    ],
    {
        fn new(v: String, _context: &ConfigValidationContext) -> Result<Self, ConfigError> {
            if v.contains('.') {
                Ok(Self::Host(v))
            } else {
                Ok(Self::Prefix(v))
            }
        }

        fn maybe_default(context: &ConfigValidationContext) -> Option<String> {
            Some(format!("{}.blockscout.com", context.client_name))
        }
    }
);
