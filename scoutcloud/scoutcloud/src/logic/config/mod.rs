pub mod default;
pub mod variables;

pub use variables::{ParsedVariable, ParsedVariableKey, UserVariable};

use anyhow::Context;
use scoutcloud_proto::blockscout::scoutcloud::v1::{
    DeployConfigInternal, DeployConfigPartialInternal,
};
use std::collections::BTreeMap;

macro_rules! parse_config_var {
    ($config:ident, $validated_vars:ident, $is_partial_config:ident, { $($var:ident),* $(,)? }) => {
        paste::item! {
            $({
                let value: Option<_> = $config.[<$var:snake>].into();
                let maybe_value = match ($is_partial_config, value) {
                    (_, Some(value)) => Some(value),
                    (false, None) => <variables::[<$var:camel>] as UserVariable<_>>::maybe_default(),
                    (true, None) => None,
                };
                if let Some(value) = maybe_value {
                    variables::[<$var:camel>]::validate(value.clone()).with_context(|| {
                        format!("failed to validate {}", std::stringify!($var))
                    })?;
                    let parsed_vars = variables::[<$var:camel>]::parse_from_value(value).await.with_context(|| {
                        format!("failed to parse {}", std::stringify!($var))
                    })?;
                    $validated_vars.0.extend(parsed_vars);
                }
            })*
        }
    }
}

macro_rules! parse_config_all_vars {
    ($config:ident, $validated_vars:ident, $is_partial:ident) => {
        parse_config_var!($config, $validated_vars, $is_partial, {
            RpcUrl,
            ServerSize,
        });
    };
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct ValidatedInstanceConfig(BTreeMap<ParsedVariableKey, serde_yaml::Value>);

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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use scoutcloud_proto::blockscout::scoutcloud::v1::DeployConfigInternal;

    #[tokio::test]
    async fn config_parse_works() {
        let config = DeployConfigInternal {
            rpc_url: "http://localhost:8545".parse().unwrap(),
            server_size: "small".to_string(),
            node_type: Some("full".to_string()),
            chain_type: Some("".to_string()),
            chain_id: Some("".to_string()),
            token_symbol: Some("".to_string()),
            instance_url: Some("".to_string()),
            logo_link: Some("".parse().unwrap()),
            chain_name: Some("".to_string()),
            icon_link: Some("".parse().unwrap()),
            homeplate_backgroup: Some("".to_string()),
            homeplace_text_color: Some("".to_string()),
        };

        let validated = ValidatedInstanceConfig::try_from_config(config)
            .await
            .unwrap();
        assert_eq!(
            validated.0.len(),
            2,
            "invalid parsed config: {:?}",
            validated
        );
    }
}
