use super::macros;
use crate::logic::config::{ParsedVariable, UserVariable};

macros::single_string_env_var!(chain_id, backend, "CHAIN_ID", None, {
    fn validate(v: String) -> Result<(), anyhow::Error> {
        v.parse::<u64>()
            .map_err(|_| anyhow::anyhow!("invalid chain_id: '{}'", v))?;
        Ok(())
    }
});
