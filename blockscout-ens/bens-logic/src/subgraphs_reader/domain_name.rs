use crate::hash_name::domain_id;
use ethers::types::{Address, Bytes};

#[derive(Debug, Clone)]
pub struct DomainName {
    pub id: String,
    pub label_name: String,
    pub name: String,
}

impl DomainName {
    pub fn new(name: &str, empty_label_hash: Option<Bytes>) -> Result<Self, anyhow::Error> {
        let name = name.trim_matches('.');
        if name.is_empty() {
            anyhow::bail!("empty name provided");
        }
        let (label_name, _) = name.split_once('.').unwrap_or((name, ""));
        let id = domain_id(name, empty_label_hash);
        Ok(Self {
            id,
            label_name: label_name.to_string(),
            name: name.to_string(),
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex::FromHex;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    #[test]
    fn it_works() {
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
                    Bytes::from_hex(
                        "0x1a13b687a5ff1d8ab1a9e189e1507a6abe834a9296cc8cff937905e3dee0c4f6",
                    )
                    .unwrap(),
                ),
                "0xa3504cdec527495c69c760c85d5be9996252f853b91fd0df04c5b6aa2deb3347",
                "levvv",
                "levvv.gno",
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
}
