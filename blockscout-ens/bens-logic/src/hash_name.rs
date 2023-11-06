use ethers::utils::keccak256;

/// Implementation of
/// https://docs.ens.domains/contract-api-reference/name-processing#algorithm
pub fn hash_ens_domain_name(name: &str) -> [u8; 32] {
    if name.is_empty() {
        [0; 32]
    } else {
        let (label, remainder) = name.split_once('.').unwrap_or((name, ""));
        let remainder_hash = hash_ens_domain_name(remainder);
        let label_hash = keccak256(label.as_bytes());
        let concatenated: Vec<u8> = remainder_hash.into_iter().chain(label_hash).collect();
        keccak256(concatenated)
    }
}

pub fn domain_id(name: &str) -> String {
    hex(hash_ens_domain_name(name))
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

    #[test]
    fn it_works() {
        for (name, expected_hash) in [
            (
                "",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
            (
                "eth",
                "93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae",
            ),
            (
                "levvv.eth",
                "38a7804a53792b0cdefe3e7271b0b85422d620ea4a82df7b7bf750a6d4b297a4",
            ),
            (
                "vitalik.eth",
                "ee6c4522aab0003e8d14cd40a6af439055fd2577951148c14b6cea9a53475835",
            ),
        ] {
            let hash = hash_ens_domain_name(name);
            assert_eq!(hex::encode(hash), expected_hash);
        }
    }
}
