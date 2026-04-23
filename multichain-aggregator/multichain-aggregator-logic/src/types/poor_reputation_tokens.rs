use crate::{error::ParseError, proto, types::ChainId};
use entity::poor_reputation_tokens::ActiveModel;
use sea_orm::ActiveValue::Set;

pub struct PoorReputationToken {
    pub chain_id: ChainId,
    pub address_hash: alloy_primitives::Address,
}

impl TryFrom<proto::import_poor_reputation_tokens_request::PoorReputationToken>
    for PoorReputationToken
{
    type Error = ParseError;

    fn try_from(
        v: proto::import_poor_reputation_tokens_request::PoorReputationToken,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            chain_id: v.chain_id.parse()?,
            address_hash: v.address_hash.parse()?,
        })
    }
}

impl From<PoorReputationToken> for ActiveModel {
    fn from(v: PoorReputationToken) -> Self {
        Self {
            address_hash: Set(v.address_hash.to_vec()),
            chain_id: Set(v.chain_id),
            ..Default::default()
        }
    }
}
