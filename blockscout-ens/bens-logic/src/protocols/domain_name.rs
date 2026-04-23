use super::{hash_name::hash_ens_domain_name, ProtocolError, Tld};
use crate::{
    hex,
    protocols::{
        hash_name::hash_infinity_domain_name, protocoler::DeployedProtocol, ProtocolSpecific,
    },
};
use alloy::primitives::{keccak256, Address, B256};
use ens_normalize_rs::EnsNameNormalizer;
use lazy_static::lazy_static;

const SEPARATOR: char = '.';

lazy_static! {
    static ref ENS_NORMALIZER: EnsNameNormalizer = EnsNameNormalizer::default();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanName {
    name: String,
    tld: Tld,
    label_name: String,
}

impl CleanName {
    pub fn new(initial_name: &str) -> Result<Self, ProtocolError> {
        let normalized = ens_normalize(initial_name)?;
        let (label_name, _) = normalized
            .split_once(SEPARATOR)
            .unwrap_or((&normalized, ""));
        let tld = Tld::from_domain_name(&normalized).ok_or_else(|| ProtocolError::InvalidName {
            name: initial_name.to_string(),
            reason: "tld not found".to_string(),
        })?;

        Ok(Self {
            label_name: label_name.to_string(),
            name: normalized,
            tld,
        })
    }

    pub fn tld(&self) -> &Tld {
        &self.tld
    }

    pub fn label_name(&self) -> &str {
        &self.label_name
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn level(&self) -> usize {
        self.name().chars().filter(|c| *c == SEPARATOR).count() + 1
    }

    pub fn iter_parts(&self) -> impl Iterator<Item = &str> {
        self.name().split(SEPARATOR)
    }

    pub fn labelhash(&self) -> B256 {
        keccak256(self.label_name().as_bytes())
    }

    pub fn append_tld(self, tld: Tld) -> Self {
        let old_name = self.name;
        let old_label_name = self.label_name;
        let new_name = format!("{}.{}", old_name, tld.0);
        Self {
            name: new_name,
            tld,
            label_name: old_label_name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainName {
    id: String,
    id_bytes: B256,
    clean: CleanName,
    empty_label_hash: Option<B256>,
    specific: Option<ProtocolSpecific>,
}

impl DomainName {
    pub fn new(
        name: &str,
        empty_label_hash: Option<B256>,
        specific: Option<ProtocolSpecific>,
    ) -> Result<Self, ProtocolError> {
        let clean = CleanName::new(name)?;
        let id_bytes = calculate_id(clean.name(), empty_label_hash, &specific);
        let id = hex(id_bytes);

        Ok(Self {
            id,
            id_bytes,
            clean,
            empty_label_hash,
            specific,
        })
    }

    pub fn new_from_name_and_protocol(
        name: &str,
        protocol: &ProtocolSpecific,
    ) -> Result<Self, ProtocolError> {
        let empty_label_hash = protocol.empty_label_hash();
        let specific = Some(protocol.clone());
        Self::new(name, empty_label_hash, specific)
    }

    pub fn addr_reverse(addr: &Address) -> Self {
        let label_name = format!("{addr:x}");
        let name = format!("{label_name}.addr.reverse");
        Self::new(&name, None, None).expect("addr.reverse is always valid")
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn id_bytes(&self) -> &B256 {
        &self.id_bytes
    }

    pub fn clean(&self) -> &CleanName {
        &self.clean
    }

    pub fn empty_label_hash(&self) -> Option<&B256> {
        self.empty_label_hash.as_ref()
    }

    /// Returns true if level of domain is greater than 1
    /// e.g. `vitalik.eth`, `test.vitalik.eth`, `test.test.vitalik.eth` are 2nd, 3rd and 4th level domains
    /// `eth` and `vitalik` are TLD
    pub fn level_gt_tld(&self) -> bool {
        self.level() > 1
    }

    pub fn level(&self) -> usize {
        self.clean.level()
    }

    pub fn iter_parts(&self) -> impl Iterator<Item = &str> {
        self.clean.iter_parts()
    }

    /// Returns an iterator over the parent names of the domain, including the domain itself
    pub fn iter_parents_with_self(&self) -> impl Iterator<Item = Self> {
        alloy_ccip_read::utils::iter_parent_names(self.clean.name())
            .into_iter()
            .map(|name| {
                Self::new(name, self.empty_label_hash, self.specific.clone())
                    .expect("parent name is already normalized")
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    pub fn label_name(&self) -> &str {
        self.clean.label_name()
    }

    pub fn labelhash(&self) -> B256 {
        self.clean.labelhash()
    }

    pub fn tld(&self) -> &Tld {
        self.clean.tld()
    }

    pub fn name(&self) -> &str {
        self.clean.name()
    }
}

#[derive(Debug, Clone)]
pub struct DomainNameOnProtocol<'a> {
    pub inner: DomainName,
    pub deployed_protocol: DeployedProtocol<'a>,
}

impl<'a> DomainNameOnProtocol<'a> {
    pub fn new(name: DomainName, protocol_network: DeployedProtocol<'a>) -> Self {
        Self {
            inner: name,
            deployed_protocol: protocol_network,
        }
    }

    pub fn from_str(
        name: &str,
        protocol_network: DeployedProtocol<'a>,
    ) -> Result<Self, ProtocolError> {
        let specific = protocol_network.protocol.info.protocol_specific.clone();
        let domain = DomainName::new_from_name_and_protocol(name, &specific)?;

        Ok(Self::new(domain, protocol_network))
    }

    pub fn tld_is_native(&self) -> bool {
        self.deployed_protocol
            .protocol
            .info
            .tld_list
            .contains(self.inner.tld())
    }
}

fn ens_normalize(name: &str) -> Result<String, ProtocolError> {
    let trimmed = name.trim().trim_matches(SEPARATOR);
    let normalized = ENS_NORMALIZER
        .normalize(trimmed)
        .map_err(|e| ProtocolError::InvalidName {
            name: name.to_string(),
            reason: e.to_string(),
        })?;
    if normalized.is_empty() {
        return Err(ProtocolError::InvalidName {
            name: name.to_string(),
            reason: "empty name".to_string(),
        });
    }
    Ok(normalized)
}

fn calculate_id(
    name: &str,
    empty_label_hash: Option<B256>,
    specific: &Option<ProtocolSpecific>,
) -> B256 {
    match specific {
        Some(ProtocolSpecific::EnsLike(_) | ProtocolSpecific::D3Connect(_)) | None => {
            hash_ens_domain_name(name, empty_label_hash)
        }
        Some(ProtocolSpecific::InfinityName(_)) => hash_infinity_domain_name(name),
    }
}

#[cfg(test)]
mod tests {
    use crate::protocols::InfinityNameProtocol;

    use super::*;
    use alloy::hex::FromHex;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    #[test]
    fn ens_domain_creation_works() {
        for (name, empty_label_hash, expected_id, expected_label, expected_name) in [
            (
                "eth",
                None,
                "0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae",
                "eth",
                "eth",
            ),
            (
                ".eth",
                None,
                "0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae",
                "eth",
                "eth",
            ),
            (
                "levvv.eth",
                None,
                "0x38a7804a53792b0cdefe3e7271b0b85422d620ea4a82df7b7bf750a6d4b297a4",
                "levvv",
                "levvv.eth",
            ),
            (
                ".levvv.eth",
                None,
                "0x38a7804a53792b0cdefe3e7271b0b85422d620ea4a82df7b7bf750a6d4b297a4",
                "levvv",
                "levvv.eth",
            ),
            (
                ".levvv.gno",
                Some(
                    B256::from_hex(
                        "0x6cbb71e02aa156be31c9be2644cd7e3fe375b291d0786c825495af35fe98ee72",
                    )
                    .unwrap(),
                ),
                "0x79e028f97b232b1600b2ed68cc7d9811c28595c3ab859b166d13980bcfcece9d",
                "levvv",
                "levvv.gno",
            ),
            (
                "🏴󠁧󠁢󠁥󠁮󠁧󠁿.eth",
                None,
                "0x64b8e43c3907b77414f712a75c9718b0082ba41490806479e22d72b640c1445c",
                "🏴󠁧󠁢󠁥󠁮󠁧󠁿",
                "🏴󠁧󠁢󠁥󠁮󠁧󠁿.eth",
            ),
            (
                "LEVVV.ETH",
                None,
                "0x38a7804a53792b0cdefe3e7271b0b85422d620ea4a82df7b7bf750a6d4b297a4",
                "levvv",
                "levvv.eth",
            ),
        ] {
            let domain_name =
                DomainName::new(name, empty_label_hash, None).expect("failed to build domain name");
            assert_eq!(domain_name.id, expected_id);
            assert_eq!(domain_name.label_name(), expected_label);
            assert_eq!(domain_name.name(), expected_name)
        }
    }

    #[test]
    fn infinity_domain_creation_works() {
        for (name, expected_id, expected_label, expected_name) in [
            (
                "suleyman.blue",
                "0xd8959309308b01a3125249e04f2abadaeeba5e6d68c63d29c97743d514d94fd3",
                "suleyman",
                "suleyman.blue",
            ),
            (
                "test.suleyman.blue",
                "0x4d0241fe34d59d115986ed1157736bd231ed8706d5b05172f6c51dc81092b889",
                "test",
                "test.suleyman.blue",
            ),
        ] {
            let specific = ProtocolSpecific::InfinityName(InfinityNameProtocol {
                main_contract: Address::from_str("0x0000000000000000000000000000000000000000")
                    .unwrap(),
            });
            let domain_name =
                DomainName::new(name, None, Some(specific)).expect("failed to build domain name");
            assert_eq!(domain_name.id, expected_id);
            assert_eq!(domain_name.label_name(), expected_label);
            assert_eq!(domain_name.name(), expected_name);
        }
    }

    #[test]
    fn reverse_works() {
        let addr = Address::from_str("0x43C960FA130e3Eb58e7AaF65f46F76B5C607C3a9").unwrap();
        let domain_name = DomainName::addr_reverse(&addr);
        assert_eq!(
            domain_name.id,
            "0x397426edefbcd650b9878aabf579977fd0b2c4dd5b09beca41e055ca2273e743",
        );
        assert_eq!(
            domain_name.label_name(),
            "43c960fa130e3eb58e7aaf65f46f76b5c607c3a9"
        )
    }

    #[test]
    fn iter_parents_works() {
        let domain_name = DomainName::new("5.fourth.third.vitalik.eth", None, None).unwrap();
        let parents = domain_name
            .iter_parents_with_self()
            .map(|d| d.clean.name().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            parents,
            vec![
                "5.fourth.third.vitalik.eth",
                "fourth.third.vitalik.eth",
                "third.vitalik.eth",
                "vitalik.eth",
                "eth"
            ]
        );
    }
}
