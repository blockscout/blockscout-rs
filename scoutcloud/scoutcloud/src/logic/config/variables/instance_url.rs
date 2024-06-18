use crate::logic::{
    config::{macros, ConfigError},
    ConfigValidationContext,
};
use serde_plain::derive_serialize_from_display;
use std::fmt::{Display, Formatter};

pub enum InstanceUrl {
    Host(String),
    Prefix(String),
}
derive_serialize_from_display!(InstanceUrl);

impl Display for InstanceUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            InstanceUrl::Host(v) => write!(f, "{v}"),
            InstanceUrl::Prefix(v) => write!(f, "{v}.cloud.blockscout.com"),
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
            // default should be <client-name>.k8s-dev.blockscout.com
            Some(context.client_name.clone())
        }
    }
);

pub fn hostname_to_url(hostname: &str) -> Result<url::Url, ConfigError> {
    let instance_url = if hostname.starts_with("http") {
        hostname.to_string()
    } else {
        format!("https://{}", hostname)
    };

    url::Url::parse(&instance_url).map_err(|e| ConfigError::Validation(e.to_string()))
}
