use super::{addresses::Address, block_ranges::BlockRange, hashes::Hash};
use entity::sea_orm_active_enums as db_enum;
use multichain_aggregator_proto::blockscout::multichain_aggregator::v1::{
    self, address_upsert as proto_address, hash_upsert as proto_hash,
};

#[derive(Debug, Clone)]
pub struct BatchImportRequest {
    pub block_ranges: Vec<BlockRange>,
    pub hashes: Vec<Hash>,
    pub addresses: Vec<Address>,
}

impl TryFrom<v1::BatchImportRequest> for BatchImportRequest {
    type Error = anyhow::Error;

    fn try_from(value: v1::BatchImportRequest) -> Result<Self, Self::Error> {
        let chain_id = value.chain_id.parse()?;
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
                    let hash = h.hash.parse()?;
                    let hash_type = proto_hash_type_to_db_hash_type(h.hash_type());
                    Ok(Hash {
                        chain_id,
                        hash,
                        hash_type,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?,
            addresses: value
                .addresses
                .into_iter()
                .map(|a| {
                    let hash = a.hash.parse()?;
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
                .collect::<anyhow::Result<Vec<_>>>()?,
        })
    }
}

fn proto_hash_type_to_db_hash_type(hash_type: proto_hash::HashType) -> db_enum::HashType {
    match hash_type {
        proto_hash::HashType::Block => db_enum::HashType::Block,
        proto_hash::HashType::Transaction => db_enum::HashType::Transaction,
    }
}

fn proto_token_type_to_db_token_type(
    token_type: proto_address::TokenType,
) -> Option<db_enum::TokenType> {
    match token_type {
        proto_address::TokenType::Erc20 => Some(db_enum::TokenType::Erc20),
        proto_address::TokenType::Erc1155 => Some(db_enum::TokenType::Erc1155),
        proto_address::TokenType::Erc721 => Some(db_enum::TokenType::Erc721),
        proto_address::TokenType::Erc404 => Some(db_enum::TokenType::Erc404),
        proto_address::TokenType::Unspecified => None,
    }
}
