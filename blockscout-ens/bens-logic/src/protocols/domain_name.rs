use super::{domain_id, ProtocolError, Tld};
use crate::protocols::protocoler::DeployedProtocol;
use alloy::primitives::{keccak256, Address, B256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainName {
    pub id: String,
    pub label_name: String,
    pub name: String,
    pub empty_label_hash: Option<B256>,
    pub tld: Tld,
}

const SEPARATOR: char = '.';

impl DomainName {
    pub fn new(name: &str, empty_label_hash: Option<B256>) -> Result<Self, ProtocolError> {
        let name = ens_normalize(name)?;
        let (label_name, _) = name.split_once(SEPARATOR).unwrap_or((&name, ""));
        let id = domain_id(&name, empty_label_hash);
        let tld = Tld::from_domain_name(&name).ok_or_else(|| ProtocolError::InvalidName {
            name: name.clone(),
            reason: "tld not found".to_string(),
        })?;
        Ok(Self {
            id,
            label_name: label_name.to_string(),
            name: name.to_string(),
            empty_label_hash,
            tld,
        })
    }

    pub fn addr_reverse(addr: &Address) -> Self {
        // label name is hexed address without 0x prefix
        let label_name = format!("{:x}", addr);
        let name = format!("{}.addr.reverse", label_name);
        // note that addr.reverse doesn't need empty_label_hash
        let id = domain_id(&name, None);
        Self {
            id,
            label_name,
            name,
            empty_label_hash: None,
            tld: Tld::reverse(),
        }
    }

    /// Returns true if level of domain is greater than 1
    /// e.g. `vitalik.eth`, `test.vitalik.eth`, `test.test.vitalik.eth` are 2nd, 3rd and 4th level domains
    /// `eth` and `vitalik` are TLD
    pub fn level_gt_tld(&self) -> bool {
        self.level() > 1
    }

    pub fn level(&self) -> usize {
        self.name.chars().filter(|c| *c == SEPARATOR).count() + 1
    }

    pub fn iter_parts(&self) -> impl Iterator<Item = &str> {
        self.name.split(SEPARATOR)
    }

    /// Returns an iterator over the parent names of the domain, including the domain itself
    pub fn iter_parents_with_self(&self) -> impl Iterator<Item = Self> {
        alloy_ccip_read::utils::iter_parent_names(&self.name)
            .into_iter()
            .map(|name| {
                Self::new(name, self.empty_label_hash).expect("parent name is already normalized")
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    pub fn labelhash(&self) -> B256 {
        keccak256(self.label_name.as_bytes())
    }

    pub fn tld(&self) -> &Tld {
        &self.tld
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
        let name = DomainName::new(name, protocol_network.protocol.info.empty_label_hash)?;

        Ok(Self::new(name, protocol_network))
    }

    pub fn tld_is_native(&self) -> bool {
        self.deployed_protocol
            .protocol
            .info
            .tld_list
            .contains(self.inner.tld())
    }
}

// TODO: implement https://docs.ens.domains/ensip/15 here
fn ens_normalize(name: &str) -> Result<String, ProtocolError> {
    let name = name.trim().trim_matches(SEPARATOR);
    if name.is_empty() {
        return Err(ProtocolError::InvalidName {
            name: name.to_string(),
            reason: "empty name".to_string(),
        });
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::hex::FromHex;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    #[test]
    fn domain_creation_works() {
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
                        "0x1a13b687a5ff1d8ab1a9e189e1507a6abe834a9296cc8cff937905e3dee0c4f6",
                    )
                    .unwrap(),
                ),
                "0xa3504cdec527495c69c760c85d5be9996252f853b91fd0df04c5b6aa2deb3347",
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
        ] {
            let domain_name =
                DomainName::new(name, empty_label_hash).expect("failed to build domain name");
            assert_eq!(domain_name.id, expected_id);
            assert_eq!(domain_name.label_name, expected_label);
            assert_eq!(domain_name.name, expected_name)
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
            domain_name.label_name,
            "43c960fa130e3eb58e7aaf65f46f76b5c607c3a9"
        )
    }

    #[test]
    fn iter_parents_works() {
        let domain_name = DomainName::new("5.fourth.third.vitalik.eth", None).unwrap();
        let parents = domain_name
            .iter_parents_with_self()
            .map(|d| d.name)
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
