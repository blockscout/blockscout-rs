use crate::logic::{
    config::{macros, Error},
    ConfigValidationContext,
};
use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use serde_plain::derive_display_from_serialize;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChainId(String);
derive_display_from_serialize!(ChainId);

macros::custom_env_var!(ChainId, String, backend, "CHAIN_ID", {
    fn new(v: String, _context: &ConfigValidationContext) -> Result<Self, Error> {
        v.parse::<BigInt>()
            .map_err(|_| Error::Validation("invalid chain_id".to_string()))?;
        Ok(Self(v))
    }
});
