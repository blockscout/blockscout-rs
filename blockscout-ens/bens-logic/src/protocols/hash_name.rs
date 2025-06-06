use alloy::primitives::{keccak256, B256};
use alloy_ccip_read::DomainIdProvider;

#[derive(Clone)]
pub struct CustomDomainIdGenerator {
    empty_label_hash: Option<B256>,
}

impl DomainIdProvider for CustomDomainIdGenerator {
    fn generate(&self, name: &str) -> B256 {
        hash_ens_domain_name(name, self.empty_label_hash)
    }
}

impl CustomDomainIdGenerator {
    pub fn new(empty_label_hash: Option<B256>) -> Self {
        Self { empty_label_hash }
    }
}

/// Implementation of
/// https://docs.ens.domains/contract-api-reference/name-processing#algorithm
/// with custom empty_label_hash
pub fn hash_ens_domain_name(name: &str, empty_label_hash: Option<B256>) -> B256 {
    if name.is_empty() {
        empty_label_hash.unwrap_or_else(|| [0; 32].into())
    } else {
        let (label, remainder) = name.split_once('.').unwrap_or((name, ""));
        let remainder_hash = hash_ens_domain_name(remainder, empty_label_hash);
        let label_hash = keccak256(label.as_bytes());
        let concatenated: Vec<u8> = remainder_hash.into_iter().chain(label_hash).collect();
        keccak256(concatenated)
    }
}

pub fn domain_id(name: &str, empty_label_hash: Option<B256>) -> String {
    hex(hash_ens_domain_name(name, empty_label_hash))
}

pub fn hex<T>(data: T) -> String
where
    T: AsRef<[u8]>,
{
    format!("0x{}", hex::encode(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::hex::FromHex;
    use pretty_assertions::assert_eq;

    #[test]
    fn default_works() {
        for (name, expected_hash) in [
            (
                "",
                "0x0000000000000000000000000000000000000000000000000000000000000000",
            ),
            (
                "eth",
                "0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae",
            ),
            (
                "levvv.eth",
                "0x38a7804a53792b0cdefe3e7271b0b85422d620ea4a82df7b7bf750a6d4b297a4",
            ),
            (
                "vitalik.eth",
                "0xee6c4522aab0003e8d14cd40a6af439055fd2577951148c14b6cea9a53475835",
            ),
            (
                "abcnews.eth",
                "0x7a68d23f9d7e32e79f09e024d21e2e12b66f74cbbc4aff0e5a36043a6a42778d",
            ),
        ] {
            let hash = domain_id(name, None);
            assert_eq!(hash, expected_hash);
        }
    }

    #[test]
    fn genome_testnet_works() {
        for (name, expected_hash) in [
            (
                "",
                "0x1a13b687a5ff1d8ab1a9e189e1507a6abe834a9296cc8cff937905e3dee0c4f6",
            ),
            (
                "gno",
                "0x634ae5e4e77ee5a262a820f4a9eacd51ac137dd75989e5a5d993f5b1db797fba",
            ),
            (
                "levvv.gno",
                "0xa3504cdec527495c69c760c85d5be9996252f853b91fd0df04c5b6aa2deb3347",
            ),
            (
                "unknown.gno",
                "0x7dd2724da2c399aa963a8ecf14e2a017b7f12026dcdf17277f96ac263d0ffbae",
            ),
            (
                "abcnews.gno",
                "0xefc07af2d64eead3daec2e3004575bfc86bfc43c34e316294bd01c957e70cba2",
            ),
        ] {
            let genome_testnet_empty_label = B256::from_hex(
                "0x1a13b687a5ff1d8ab1a9e189e1507a6abe834a9296cc8cff937905e3dee0c4f6",
            )
            .expect("valid hex");
            let hash = domain_id(name, Some(genome_testnet_empty_label));
            assert_eq!(hash, expected_hash);
        }
    }
}
