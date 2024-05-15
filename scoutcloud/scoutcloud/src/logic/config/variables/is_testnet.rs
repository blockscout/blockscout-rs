use crate::logic::{config::macros, ConfigValidationContext};

macros::simple_env_var!(IsTestnet, bool, ConfigPath, "config.testnet", {
    fn maybe_default(_context: &ConfigValidationContext) -> Option<Self::SourceType> {
        Some(false)
    }
});
