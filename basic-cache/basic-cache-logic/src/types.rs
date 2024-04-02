struct SmartContractId {
    /// id of the chain the contract is deployed at
    chain_id: String,
    /// address of a contract for the given chain
    /// (e.g., 0xc3279442a5acacf0a2ecb015d1cddbb3e0f3f775)
    address: ethers_core::types::Address,
}

struct SmartContract {
    id: SmartContractId
    /// url which leads to the contract on th corresponding blocksocut instance
    /// (e.g., https://blockscout.com/address/0xc3279442a5acacf0a2ecb015d1cddbb3e0f3f775)
    blockscout_url: url::Url,
    /// contract source code stored as a mapping from file name to the content
    sources: BTreeMap<String, String>,
}
