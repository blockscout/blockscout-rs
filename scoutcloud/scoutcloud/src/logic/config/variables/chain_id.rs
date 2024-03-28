use crate::logic::config::macros;

pub struct ChainId {}

macros::single_env_var!(ChainId, String, backend, "CHAIN_ID", None, {
    fn validate(v: String) -> Result<(), anyhow::Error> {
        v.parse::<u64>()
            .map_err(|_| anyhow::anyhow!("invalid chain_id: '{}'", v))?;
        Ok(())
    }
});
