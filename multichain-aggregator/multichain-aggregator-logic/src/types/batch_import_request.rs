use super::{
    address_coin_balances::AddressCoinBalance,
    address_token_balances::AddressTokenBalance,
    addresses::{proto_token_type_to_db_token_type, Address},
    block_ranges::BlockRange,
    hashes::{proto_hash_type_to_db_hash_type, Hash},
};
use crate::{
    error::{ParseError, ServiceError},
    proto,
};
use sea_orm::prelude::Decimal;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct BatchImportRequest {
    pub block_ranges: Vec<BlockRange>,
    pub hashes: Vec<Hash>,
    pub addresses: Vec<Address>,
    pub address_coin_balances: Vec<AddressCoinBalance>,
    pub address_token_balances: Vec<AddressTokenBalance>,
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
                        domain_info: None,
                        contract_name: a.contract_name,
                        token_name: a.token_name,
                        token_type,
                        is_contract: a.is_contract.unwrap_or(false),
                        is_verified_contract: a.is_verified_contract.unwrap_or(false),
                        is_token: a.is_token.unwrap_or(false),
                    })
                })
                .collect::<Result<Vec<_>, Self::Error>>()?,
            address_coin_balances: value
                .address_coin_balances
                .into_iter()
                .map(|ab| {
                    let address_hash = ab.address_hash.parse().map_err(ParseError::from)?;
                    let value = Decimal::from_str(&ab.value).map_err(|_| {
                        ParseError::Custom(format!("invalid decimal: {}", ab.value))
                    })?;

                    Ok(AddressCoinBalance {
                        address_hash,
                        chain_id,
                        value,
                    })
                })
                .collect::<Result<Vec<_>, Self::Error>>()?,
            address_token_balances: value
                .address_token_balances
                .into_iter()
                .map(|at| {
                    let address_hash = at.address_hash.parse().map_err(ParseError::from)?;
                    let token_address_hash =
                        at.token_address_hash.parse().map_err(ParseError::from)?;
                    let value = Decimal::from_str(&at.value).map_err(|_| {
                        ParseError::Custom(format!("invalid decimal: {}", at.value))
                    })?;

                    Ok(AddressTokenBalance {
                        address_hash,
                        chain_id,
                        value,
                        token_address_hash,
                    })
                })
                .collect::<Result<Vec<_>, Self::Error>>()?,
        })
    }
}
