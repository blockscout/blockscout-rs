use super::{
    addresses::{proto_token_type_to_db_token_type, Address},
    block_ranges::BlockRange,
    hashes::{proto_hash_type_to_db_hash_type, Hash},
};
use crate::{
    error::{ParseError, ServiceError},
    proto,
};

#[derive(Debug, Clone)]
pub struct BatchImportRequest {
    pub block_ranges: Vec<BlockRange>,
    pub hashes: Vec<Hash>,
    pub addresses: Vec<Address>,
}

impl TryFrom<proto::BatchImportRequest> for BatchImportRequest {
    type Error = ServiceError;

    fn try_from(value: proto::BatchImportRequest) -> Result<Self, Self::Error> {
        let chain_id = value.chain_id.parse().map_err(ParseError::from)?;
        Ok(Self {
            block_ranges: value
                .block_ranges
                .into_iter()
                .map(|br| BlockRange {
                    chain_id,
                    min_block_number: br.min_block_number,
                    max_block_number: br.max_block_number,
                })
                .collect(),
            hashes: value
                .hashes
                .into_iter()
                .map(|h| {
                    let hash = h.hash.parse().map_err(ParseError::from)?;
                    let hash_type = proto_hash_type_to_db_hash_type(h.hash_type());
                    Ok(Hash {
                        chain_id,
                        hash,
                        hash_type,
                    })
                })
                .collect::<Result<Vec<_>, Self::Error>>()?,
            addresses: value
                .addresses
                .into_iter()
                .map(|a| {
                    let hash = a.hash.parse().map_err(ParseError::from)?;
                    let token_type = proto_token_type_to_db_token_type(a.token_type());

                    Ok(Address {
                        chain_id,
                        hash,
                        ens_name: a.ens_name,
                        contract_name: a.contract_name,
                        token_name: a.token_name,
                        token_type,
                        is_contract: a.is_contract.unwrap_or(false),
                        is_verified_contract: a.is_verified_contract.unwrap_or(false),
                        is_token: a.is_token.unwrap_or(false),
                    })
                })
                .collect::<Result<Vec<_>, Self::Error>>()?,
        })
    }
}
