pub enum ContractCode {
    OnlyRuntimeCode {
        code: Vec<u8>,
    },
    CompleteCode {
        creation_code: Vec<u8>,
        runtime_code: Vec<u8>,
    },
}

pub enum ContractDeployment {
    Genesis {
        chain_id: u128,
        address: Vec<u8>,
        runtime_code: Vec<u8>,
    },
    Regular {
        chain_id: u128,
        address: Vec<u8>,
        transaction_hash: Vec<u8>,
        block_number: u128,
        transaction_index: u128,
        deployer: Vec<u8>,
        creation_code: Vec<u8>,
        runtime_code: Vec<u8>,
    },
}
