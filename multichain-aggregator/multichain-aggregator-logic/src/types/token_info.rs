use super::ChainId;
use crate::{clients, error::ParseError, proto};

#[derive(Debug)]
pub struct Token {
    pub address: alloy_primitives::Address,
    pub icon_url: String,
    pub name: String,
    pub symbol: String,
    pub chain_id: ChainId,
    pub is_verified_contract: bool,
}

impl TryFrom<clients::token_info::search_token_infos::TokenInfo> for Token {
    type Error = ParseError;

    fn try_from(
        v: clients::token_info::search_token_infos::TokenInfo,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            address: v.token_address.parse().map_err(ParseError::from)?,
            icon_url: v.icon_url,
            name: v
                .token_name
                .ok_or_else(|| ParseError::Custom("token name is required".to_string()))?,
            symbol: v
                .token_symbol
                .ok_or_else(|| ParseError::Custom("token symbol is required".to_string()))?,
            chain_id: v.chain_id.parse().map_err(ParseError::from)?,
            is_verified_contract: false,
        })
    }
}

impl From<Token> for proto::Token {
    fn from(v: Token) -> Self {
        Self {
            address: v.address.to_string(),
            name: v.name,
            symbol: v.symbol,
            icon_url: v.icon_url,
            chain_id: v.chain_id.to_string(),
            is_verified_contract: v.is_verified_contract,
        }
    }
}
