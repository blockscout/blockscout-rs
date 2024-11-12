use super::{DomainToken, DomainTokenType};
use crate::{
    entity::subgraph::domain::DetailedDomain,
    protocols::{D3ConnectProtocol, DomainNameOnProtocol, EnsLikeProtocol, ProtocolSpecific},
};
use alloy::primitives::Address;
use anyhow::Context;
use bigdecimal::{num_bigint::BigInt, Num};
use std::str::FromStr;

#[tracing::instrument(
    level = "info",
    skip_all,
    fields(
        domain_name = domain.name,
        protocol_type = ?name.deployed_protocol.protocol.info.protocol_specific,
    ),
    err,
)]
pub fn extract_tokens_from_domain(
    domain: &DetailedDomain,
    name: &DomainNameOnProtocol<'_>,
) -> Result<Vec<DomainToken>, anyhow::Error> {
    let mut tokens = vec![];

    match &name.deployed_protocol.protocol.info.protocol_specific {
        ProtocolSpecific::EnsLike(ens_like) => {
            extract_tokens_for_ens_like(&mut tokens, domain, name, ens_like)?;
        }
        ProtocolSpecific::D3Connect(d3_connect) => {
            extract_tokens_for_d3_connect(&mut tokens, domain, name, d3_connect)?;
        }
    }

    Ok(tokens)
}

fn extract_tokens_for_ens_like(
    tokens: &mut Vec<DomainToken>,
    domain: &DetailedDomain,
    name: &DomainNameOnProtocol<'_>,
    ens_like: &EnsLikeProtocol,
) -> Result<(), anyhow::Error> {
    if let Some(contract) = ens_like.native_token_contract {
        let is_second_level_domain = name.inner.level() == 2;
        let is_native_domain = name.tld_is_native();

        // native NFT exists only if domain is second level (like abc.eth and not abc.abc.eth)
        // and if tld is native (like .eth and not .xyz)
        if is_second_level_domain && is_native_domain {
            let labelhash = domain
                .labelhash
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("no labelhash in database"))?;

            let id = token_id(&hex::encode(labelhash))?;
            tokens.push(DomainToken {
                id,
                contract,
                _type: DomainTokenType::Native,
            });
        }
    };

    if domain.wrapped_owner.is_some() {
        let id = token_id(&domain.id)?;
        let contract = Address::from_str(&domain.owner).context("parse owner as address")?;
        tokens.push(DomainToken {
            id,
            contract,
            _type: DomainTokenType::Wrapped,
        });
    };

    Ok(())
}

fn extract_tokens_for_d3_connect(
    tokens: &mut Vec<DomainToken>,
    domain: &DetailedDomain,
    _name: &DomainNameOnProtocol<'_>,
    d3_connect: &D3ConnectProtocol,
) -> Result<(), anyhow::Error> {
    let id = token_id(&domain.id)?;
    let contract = d3_connect.native_token_contract;
    tokens.push(DomainToken {
        id,
        contract,
        _type: DomainTokenType::Native,
    });
    Ok(())
}

fn token_id(hexed_id: &str) -> Result<String, anyhow::Error> {
    let id = BigInt::from_str_radix(hexed_id.trim_start_matches("0x"), 16)
        .context("convert token_id to number")?;
    Ok(id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        blockscout::BlockscoutClient,
        protocols::{DeployedProtocol, EnsLikeProtocol, Network, Protocol, Tld},
    };
    use nonempty::nonempty;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;

    #[inline]
    fn domain(
        name: &str,
        id: &str,
        labelhash: &str,
        owner: &str,
        maybe_wrapped_owner: Option<&str>,
    ) -> DetailedDomain {
        DetailedDomain {
            id: id.to_string(),
            name: Some(name.to_string()),
            labelhash: Some(
                hex::decode(labelhash.trim_start_matches("0x"))
                    .expect("invalid labelhash provided"),
            ),
            owner: owner.to_string(),
            wrapped_owner: maybe_wrapped_owner.map(str::to_string),
            ..Default::default()
        }
    }

    #[inline]
    fn addr(a: &str) -> Option<Address> {
        Address::from_str(a).ok()
    }

    #[test]
    fn it_works() {
        let native_contract = "0x1234567890123456789012345678901234567890";
        let wrapped_contract = "0x0987654321098765432109876543210987654321";
        let owner = "0x1111111111111111111111111111111111111111";
        for (domain, native_token_contract, expected_tokens) in [
            // No native contract provided
            (
                domain("levvv.eth", "0x0200", "0x0100", owner, None),
                None,
                vec![],
            ),
            // Native contract provided, but domain is third level
            (
                domain(
                    "this_is_third_level_domain.levvv.eth",
                    "0x0200",
                    "0x0100",
                    owner,
                    None,
                ),
                addr(native_contract),
                vec![],
            ),
            // Native contract provided, no wrapped owner
            (
                domain("levvv.eth", "0x0200", "0x0100", owner, None),
                addr(native_contract),
                vec![DomainToken {
                    id: "256".to_string(),
                    contract: Address::from_str(native_contract)
                        .expect("invalid native_contract provided"),
                    _type: DomainTokenType::Native,
                }],
            ),
            // Native contract provided, wrapped owner provided, but third level domain, so only wrapped token
            (
                domain(
                    "this_is_third_level_domain.levvv.eth",
                    "0x0200",
                    "0x0100",
                    wrapped_contract,
                    Some(owner),
                ),
                addr(native_contract),
                vec![DomainToken {
                    id: "512".to_string(),
                    contract: Address::from_str(wrapped_contract)
                        .expect("invalid wrapped_contract provided"),
                    _type: DomainTokenType::Wrapped,
                }],
            ),
            // Everything is provided
            (
                domain(
                    "levvv.eth",
                    "0x38a7804a53792b0cdefe3e7271b0b85422d620ea4a82df7b7bf750a6d4b297a4",
                    "0x1a8247ca2a4190d90c748b31fa6517e5560c1b7a680f03ff73dbbc3ed2c0ed66",
                    wrapped_contract,
                    Some(owner),
                ),
                addr(native_contract),
                vec![
                    DomainToken {
                        id: "11990319655936053415661126359086567018700354293176496925267203544835860524390".to_string(),
                        contract: Address::from_str(native_contract)
                            .expect("invalid native_contract provided"),
                        _type: DomainTokenType::Native,
                    },
                    DomainToken {
                        id: "25625468407840116393736812939389551247551040926951238633020744494000165263268".to_string(),
                        contract: Address::from_str(wrapped_contract)
                            .expect("invalid wrapped_contract provided"),
                        _type: DomainTokenType::Wrapped,
                    },
                ],
            ),
            // tld is not native
            (
                domain("levvv.xyz", "0x0200", "0x0100", owner, None),
                addr(native_contract),
                vec![],
            )
        ] {
            let mut protocol = Protocol::default();
            protocol.info.tld_list = nonempty![Tld::new("eth")];
            protocol.info.protocol_specific = ProtocolSpecific::EnsLike(EnsLikeProtocol { native_token_contract, ..Default::default() });
            let deployed_protocol = DeployedProtocol {
                protocol: &protocol,
                deployment_network: &Network {
                    blockscout_client: Arc::new(BlockscoutClient::new("http://localhost:8545".parse().unwrap(), 1, 1)),
                    rpc_url: None,
                    use_protocols: vec![],
                },
            };
            let name = DomainNameOnProtocol::from_str(domain.name.as_ref().unwrap(), deployed_protocol).unwrap();
            let tokens = extract_tokens_from_domain(&domain, &name)
                .expect("failed to extract tokens from domain");

            assert_eq!(tokens, expected_tokens, "failed for domain: {}", name.inner.name);
        }
    }
}
