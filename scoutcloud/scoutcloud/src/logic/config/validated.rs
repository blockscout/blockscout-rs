use crate::logic::config::{variables, ParsedVariableKey, UserVariable};
use anyhow::Context;
use scoutcloud_proto::blockscout::scoutcloud::v1::{
    DeployConfigInternal, DeployConfigPartialInternal,
};
use std::collections::BTreeMap;

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct ValidatedInstanceConfig {
    pub vars: BTreeMap<ParsedVariableKey, serde_json::Value>,
}

macro_rules! parse_config_var {
    ($config:ident, $validated_config:ident, $is_partial_config:ident, { $($var:ident),* $(,)? }) => {
        paste::item! {
            $({
                let value: Option<_> = $config.[<$var:snake>].into();
                let maybe_value = match ($is_partial_config, value) {
                    (_, Some(value)) => Some(value),
                    (false, None) => <variables::[<$var:snake>]::[<$var:camel>] as UserVariable<_>>::maybe_default(),
                    (true, None) => None,
                };
                if let Some(value) = maybe_value {
                    variables::[<$var:snake>]::[<$var:camel>]::validate(value.clone()).with_context(|| {
                        format!("failed to validate {}", std::stringify!($var))
                    })?;
                    let parsed_vars = variables::[<$var:snake>]::[<$var:camel>]::build_config_vars(value).await.with_context(|| {
                        format!("failed to build config variable for '{}'", std::stringify!([<$var:snake>]))
                    })?;
                    $validated_config.vars.extend(parsed_vars);
                }
            })*
        }
    }
}

macro_rules! parse_config_all_vars {
    ($config:ident, $validated_config:ident, $is_partial:ident) => {
        parse_config_var!($config, $validated_config, $is_partial, {
            RpcUrl,
            ServerSize,
            NodeType,
            ChainId,
        });
    };
}

impl ValidatedInstanceConfig {
    pub async fn try_from_config_partial(
        config: DeployConfigPartialInternal,
    ) -> Result<Self, anyhow::Error> {
        let mut this = Self::default();
        let is_partial = true;
        parse_config_all_vars!(config, this, is_partial);
        Ok(this)
    }

    pub async fn try_from_config(config: DeployConfigInternal) -> Result<Self, anyhow::Error> {
        let mut this = Self::default();
        let is_partial = false;
        parse_config_all_vars!(config, this, is_partial);
        Ok(this)
    }
}
