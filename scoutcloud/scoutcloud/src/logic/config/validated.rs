use crate::logic::{
    config::{types::ConfigValidationContext, variables, Error},
    ParsedVariableKey, UserVariable,
};
use scoutcloud_proto::blockscout::scoutcloud::v1::{
    DeployConfigInternal, DeployConfigPartialInternal,
};
use std::collections::BTreeMap;

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct ValidatedInstanceConfig {
    pub vars: BTreeMap<ParsedVariableKey, serde_json::Value>,
}

macro_rules! parse_config_vars {
    ($config:ident, $context:ident, $validated_config:ident, $is_partial_config:ident, { $($var:ident),* $(,)? }) => {
        paste::item! {
            $({
                let value: Option<_> = $config.[<$var:snake>].into();
                let maybe_value = match ($is_partial_config, value) {
                    (_, Some(value)) => Some(value),
                    (false, None) => <variables::[<$var:snake>]::[<$var:camel>] as UserVariable<_>>::maybe_default(&$context),
                    (true, None) => None,
                };
                if let Some(value) = maybe_value {
                    let parsed_vars = variables::[<$var:snake>]::[<$var:camel>]::new(value, &$context)?
                        .build_config_vars(&$context)
                        .await?;
                    $validated_config.vars.extend(parsed_vars);
                }
            })*
        }
    }
}

macro_rules! parse_config_all_vars {
    ($config:ident, $context:ident, $validated_config:ident, $is_partial:ident) => {
        parse_config_vars!($config, $context, $validated_config, $is_partial, {
            ChainId,
            ChainName,
            ChainType,
            HomeplateBackground,
            HomeplateTextColor,
            IconUrl,
            InstanceUrl,
            NodeType,
            RpcUrl,
            ServerSize,
            TokenSymbol,
        });
    };
}

impl ValidatedInstanceConfig {
    pub async fn try_from_config_partial(
        config: DeployConfigPartialInternal,
        context: ConfigValidationContext,
    ) -> Result<Self, Error> {
        let mut this = Self::default();
        let is_partial = true;
        parse_config_all_vars!(config, context, this, is_partial);
        Ok(this)
    }

    pub async fn try_from_config(
        config: DeployConfigInternal,
        context: ConfigValidationContext,
    ) -> Result<Self, Error> {
        let mut this = Self::default();
        let is_partial = false;
        parse_config_all_vars!(config, context, this, is_partial);
        Ok(this)
    }
}
