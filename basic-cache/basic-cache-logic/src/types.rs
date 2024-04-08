use basic_cache_proto::blockscout::basic_cache::v1::CreateSmartContractRequestInternal;
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
pub struct SmartContractValue {
    /// url which leads to the contract on th corresponding blocksocut instance
    /// (e.g., https://blockscout.com/address/0xc3279442a5acacf0a2ecb015d1cddbb3e0f3f775)
    pub blockscout_url: url::Url,
    /// contract source code stored as a mapping from file name to the content
    pub sources: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SmartContract {
    pub id: SmartContractId,

    pub value: SmartContractValue,
}

#[derive(thiserror::Error, Debug)]
pub enum ContractParsingError {
    #[error("contract must be included into request")]
    MissingContract,
    #[error("file names must be unique, {0} is repeated")]
    DuplicateFilenames(String),
}

impl TryFrom<CreateSmartContractRequestInternal> for SmartContract {
    type Error = ContractParsingError;

    fn try_from(value: CreateSmartContractRequestInternal) -> Result<Self, Self::Error> {
        let contract = value
            .smart_contract
            .ok_or(ContractParsingError::MissingContract)?;
        let mut sources = BTreeMap::new();
        for (
            name,
            basic_cache_proto::blockscout::basic_cache::v1::FileContentsInternal { content },
        ) in contract.sources.into_iter()
        {
            if sources.contains_key(&name) {
                return Err(ContractParsingError::DuplicateFilenames(name));
            }
            sources.insert(name, content);
        }
        Ok(SmartContract {
            id: SmartContractId {
                chain_id: value.chain_id,
                address: value.address,
            },
            value: SmartContractValue {
                blockscout_url: contract.url,
                sources,
            },
        })
    }
}

impl From<SmartContractValue> for basic_cache_proto::blockscout::basic_cache::v1::SmartContract {
    fn from(value: SmartContractValue) -> Self {
        Self {
            url: value.blockscout_url.to_string(),
            sources: value
                .sources
                .into_iter()
                .map(|(name, content)| {
                    (
                        name,
                        basic_cache_proto::blockscout::basic_cache::v1::FileContents { content },
                    )
                })
                .collect(),
        }
    }
}

impl From<basic_cache_proto::blockscout::basic_cache::v1::GetSmartContractRequestInternal>
    for SmartContractId
{
    fn from(
        value: basic_cache_proto::blockscout::basic_cache::v1::GetSmartContractRequestInternal,
    ) -> Self {
        Self {
            chain_id: value.chain_id,
            address: value.address,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use basic_cache_proto::blockscout::basic_cache::v1::{
        CreateSmartContractRequestInternal, FileContents, FileContentsInternal,
    };
    use convert_trait::TryConvert;
    use ethers_core::types::H160;

    #[test]
    fn from_smart_contract_value() {
        let url = url::Url::parse("https://info.cern.ch/").unwrap();
        let sources = vec![
            (
                "juju.ts".to_owned(),
                FileContents {
                    content: "const strongest = 'G'".to_owned(),
                },
            ),
            (
                "lets.go".to_owned(),
                FileContents {
                    content: "package lets".to_owned(),
                },
            ),
        ];
        let input = super::SmartContract {
            id: super::SmartContractId {
                chain_id: "".to_owned(),
                address: H160::from_str("0x0000000000000000000000000000000000000000").unwrap(),
            },
            value: super::SmartContractValue {
                blockscout_url: url.clone(),
                sources: sources
                    .clone()
                    .into_iter()
                    .map(|(name, contents)| (name, contents.content))
                    .collect(),
            },
        };
        let expected = basic_cache_proto::blockscout::basic_cache::v1::SmartContract {
            url: url.to_string(),
            sources: sources.into_iter().collect(),
        };
        assert_eq!(expected, input.value.into())
    }

    #[test]
    fn try_from_internal_create_request() {
        let chain_id = "aboba".to_owned();
        let address = H160::from_str("0x0000000000000000000000000000000000000000").unwrap();
        let url = url::Url::parse("https://info.cern.ch/").unwrap();
        let sources = vec![
            (
                "juju.ts".to_owned(),
                FileContents {
                    content: "const strongest = 'G'".to_owned(),
                },
            ),
            (
                "lets.go".to_owned(),
                FileContents {
                    content: "package lets".to_owned(),
                },
            ),
        ];
        let request = CreateSmartContractRequestInternal {
            chain_id: chain_id.clone(),
            address,
            smart_contract: Some(
                basic_cache_proto::blockscout::basic_cache::v1::SmartContractInternal {
                    url: url.clone(),
                    sources: sources
                        .clone()
                        .into_iter()
                        .map(|(a, b)| (a, FileContentsInternal::try_convert(b).unwrap()))
                        .collect(),
                },
            ),
        };
        let expected = super::SmartContract {
            id: super::SmartContractId { chain_id, address },
            value: super::SmartContractValue {
                blockscout_url: url,
                sources: sources
                    .into_iter()
                    .map(|(name, contents)| (name, contents.content))
                    .collect(),
            },
        };
        assert_eq!(expected, request.try_into().unwrap())
    }
}
