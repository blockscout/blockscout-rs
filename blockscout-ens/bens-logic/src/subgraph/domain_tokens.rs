// SPDX-License-Identifier: LicenseRef-Blockscout

use super::{DomainToken, DomainTokenType};
use crate::{
    entity::subgraph::domain::DetailedDomain,
    protocols::{
        D3ConnectProtocol, DomainNameOnProtocol, EnsLikeProtocol, InfinityNameProtocol,
        ProtocolSpecific,
    },
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
    match &name.deployed_protocol.protocol.info.protocol_specific {
        ProtocolSpecific::EnsLike(ens_like) => extract_tokens_for_ens_like(domain, name, ens_like),
        ProtocolSpecific::D3Connect(d3_connect) => {
            extract_tokens_for_d3_connect(domain, name, d3_connect)
        }
        ProtocolSpecific::InfinityName(infinity_name) => {
            extract_tokens_for_infinity_name(domain, name, infinity_name)
        }
    }
}

fn extract_tokens_for_ens_like(
    domain: &DetailedDomain,
    name: &DomainNameOnProtocol<'_>,
    ens_like: &EnsLikeProtocol,
) -> Result<Vec<DomainToken>, anyhow::Error> {
    let mut tokens = vec![];
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
        // The registry owner of a wrapped name is the NameWrapper contract.
        // `domain.owner` can be the holder for ENSv2-migrated names, so the
        // raw registry owner is used here.
        let registry_owner = if domain.registry_owner.is_empty() {
            &domain.owner
        } else {
            &domain.registry_owner
        };
        let contract = Address::from_str(registry_owner).context("parse owner as address")?;
        // A migrated name's v1 wrapper token no longer represents the name:
        // an unlocked migration unwraps it into a migration contract (then
        // the registry owner is one too), a locked migration leaves the
        // name wrapped under the NameWrapper and parks the ERC-1155 token in
        // the Graveyard, which only the ENSv2 slot's migration flag reveals.
        let retired = domain.ensv2_migrated
            || ens_like
                .ensv2
                .as_ref()
                .is_some_and(|v2| v2.migration_contracts.contains(&contract));
        if !retired {
            tokens.push(DomainToken {
                id,
                contract,
                _type: DomainTokenType::Wrapped,
            });
        }
    };

    Ok(tokens)
}

fn extract_tokens_for_d3_connect(
    domain: &DetailedDomain,
    _name: &DomainNameOnProtocol<'_>,
    d3_connect: &D3ConnectProtocol,
) -> Result<Vec<DomainToken>, anyhow::Error> {
    let id = token_id(&domain.id)?;
    let contract = d3_connect.native_token_contract;
    Ok(vec![DomainToken {
        id,
        contract,
        _type: DomainTokenType::Native,
    }])
}

fn extract_tokens_for_infinity_name(
    domain: &DetailedDomain,
    _name: &DomainNameOnProtocol<'_>,
    infinity_name: &InfinityNameProtocol,
) -> Result<Vec<DomainToken>, anyhow::Error> {
    match domain.token_id.clone() {
        Some(token_id) => Ok(vec![DomainToken {
            id: token_id,
            contract: infinity_name.main_contract,
            _type: DomainTokenType::Native,
        }]),
        None => Ok(vec![]),
    }
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
        protocols::{DeployedProtocol, EnsLikeProtocol, EnsV2Settings, Network, Protocol, Tld},
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
            registry_owner: owner.to_string(),
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
                    "this-is-third-level-domain.levvv.eth",
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
                    "this-is-third-level-domain.levvv.eth",
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
            check(&protocol, domain, expected_tokens);
        }
    }

    // An ENSv2-migrated locked name whose registry owner moved to the
    // Graveyard; the reported owner is the holder (wrapped owner). The v1
    // NameWrapper token is retired, so only the native token is linked, and
    // never against the holder.
    #[test]
    fn migrated_wrapped_name_links_no_retired_wrapper_token() {
        let native_contract = "0x1234567890123456789012345678901234567890";
        let graveyard = "0xda021a2c1c2f99574d861ef8c6fa745851beb7f6";
        let holder = "0x1111111111111111111111111111111111111111";
        let mut migrated = domain("levvv.eth", "0x0200", "0x0100", holder, Some(holder));
        migrated.registry_owner = graveyard.to_string();

        let mut protocol = Protocol::default();
        protocol.info.tld_list = nonempty![Tld::new("eth")];
        protocol.info.protocol_specific = ProtocolSpecific::EnsLike(EnsLikeProtocol {
            native_token_contract: addr(native_contract),
            ensv2: Some(EnsV2Settings {
                migration_contracts: vec![Address::from_str(graveyard).unwrap()],
            }),
            ..Default::default()
        });
        check(
            &protocol,
            migrated.clone(),
            vec![DomainToken {
                id: "256".to_string(),
                contract: Address::from_str(native_contract).unwrap(),
                _type: DomainTokenType::Native,
            }],
        );

        // Same name indexed by a subgraph without ENSv2 config: the wrapper
        // token is still linked, against the registry owner, not the holder.
        protocol.info.protocol_specific = ProtocolSpecific::EnsLike(EnsLikeProtocol {
            native_token_contract: addr(native_contract),
            ..Default::default()
        });
        check(
            &protocol,
            migrated,
            vec![
                DomainToken {
                    id: "256".to_string(),
                    contract: Address::from_str(native_contract).unwrap(),
                    _type: DomainTokenType::Native,
                },
                DomainToken {
                    id: "512".to_string(),
                    contract: Address::from_str(graveyard).unwrap(),
                    _type: DomainTokenType::Wrapped,
                },
            ],
        );
    }

    fn check(protocol: &Protocol, domain: DetailedDomain, expected_tokens: Vec<DomainToken>) {
        let deployed_protocol = DeployedProtocol {
            protocol,
            deployment_network: &Network {
                network_id: 1,
                blockscout_client: Arc::new(BlockscoutClient::new(
                    "http://localhost:8545".parse().unwrap(),
                    1,
                    1,
                )),
                rpc_url: None,
                use_protocols: vec![],
            },
        };
        let name = DomainNameOnProtocol::from_str(domain.name.as_ref().unwrap(), deployed_protocol)
            .unwrap();
        let tokens = extract_tokens_from_domain(&domain, &name)
            .expect("failed to extract tokens from domain");

        assert_eq!(
            tokens,
            expected_tokens,
            "failed for domain: {}",
            name.inner.name()
        );
    }

    // A locked migration on mainnet usually leaves the registry owner as the
    // NameWrapper (24 of ~27,800 migrated names at 2026-11-09) while the
    // ERC-1155 token sits in the Graveyard. The address check cannot see that;
    // the ENSv2 slot's migration flag retires the token. A plain wrapped v1
    // name, not migrated, still links its wrapper token.
    #[test]
    fn locked_migrated_name_under_name_wrapper_links_no_wrapper_token() {
        let native_contract = "0x1234567890123456789012345678901234567890";
        let name_wrapper = "0xd4416b13d2b3a9abae7acd5d6c2bbdbe25686401";
        let graveyard = "0xda021a2c1c2f99574d861ef8c6fa745851beb7f6";
        let holder = "0x1111111111111111111111111111111111111111";
        let mut protocol = Protocol::default();
        protocol.info.tld_list = nonempty![Tld::new("eth")];
        protocol.info.protocol_specific = ProtocolSpecific::EnsLike(EnsLikeProtocol {
            native_token_contract: addr(native_contract),
            ensv2: Some(EnsV2Settings {
                migration_contracts: vec![Address::from_str(graveyard).unwrap()],
            }),
            ..Default::default()
        });
        let native = DomainToken {
            id: "256".to_string(),
            contract: Address::from_str(native_contract).unwrap(),
            _type: DomainTokenType::Native,
        };
        let wrapped = DomainToken {
            id: "512".to_string(),
            contract: Address::from_str(name_wrapper).unwrap(),
            _type: DomainTokenType::Wrapped,
        };

        let mut migrated = domain("levvv.eth", "0x0200", "0x0100", holder, Some(holder));
        migrated.registry_owner = name_wrapper.to_string();
        migrated.ensv2_migrated = true;
        check(&protocol, migrated, vec![native.clone()]);

        let mut wrapped_v1 = domain("levvv.eth", "0x0200", "0x0100", name_wrapper, Some(holder));
        wrapped_v1.registry_owner = name_wrapper.to_string();
        check(&protocol, wrapped_v1, vec![native, wrapped]);
    }
}
