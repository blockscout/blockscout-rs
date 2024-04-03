use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SmartContractId {
    /// id of the chain the contract is deployed at
    pub chain_id: String,
    /// address of a contract for the given chain
    /// (e.g., 0xc3279442a5acacf0a2ecb015d1cddbb3e0f3f775)
    pub address: ethers_core::types::Address,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SmartContract {
    pub id: SmartContractId,
    /// url which leads to the contract on th corresponding blocksocut instance
    /// (e.g., https://blockscout.com/address/0xc3279442a5acacf0a2ecb015d1cddbb3e0f3f775)
    pub blockscout_url: url::Url,
    /// contract source code stored as a mapping from file name to the content
    pub sources: BTreeMap<String, String>,
}

#[derive(thiserror::Error, Debug)]
pub enum ContractParsingError {
    #[error("contract must be included into request")]
    MissingContract,
}

impl TryFrom<basic_cache_proto::blockscout::basic_cache::v1::CreateSmartContractRequestInternal>
    for SmartContract
{
    type Error = ContractParsingError;

    fn try_from(
        value: basic_cache_proto::blockscout::basic_cache::v1::CreateSmartContractRequestInternal,
    ) -> Result<Self, Self::Error> {
        let contract = value
            .smart_contract
            .ok_or(ContractParsingError::MissingContract)?;
        Ok(SmartContract {
            id: SmartContractId {
                chain_id: value.chain_id,
                address: value.address,
            },
            blockscout_url: contract.url,
            sources: contract
                .sources
                .into_iter()
                .map(|f| (f.name, f.content))
                .collect(),
        })
    }
}

impl From<SmartContract> for basic_cache_proto::blockscout::basic_cache::v1::SmartContract {
    fn from(value: SmartContract) -> Self {
        Self {
            url: value.blockscout_url.to_string(),
            sources: value
                .sources
                .into_iter()
                .map(
                    |(name, content)| basic_cache_proto::blockscout::basic_cache::v1::SourceFile {
                        name,
                        content,
                    },
                )
                .collect(),
        }
    }
}
