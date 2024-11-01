#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ContractCode {
    OnlyRuntimeCode {
        code: Vec<u8>,
    },
    CompleteCode {
        creation_code: Vec<u8>,
        runtime_code: Vec<u8>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

impl ContractDeployment {
    pub fn chain_id(&self) -> u128 {
        match self {
            ContractDeployment::Genesis { chain_id, .. } => *chain_id,
            ContractDeployment::Regular { chain_id, .. } => *chain_id,
        }
    }

    pub fn address(&self) -> &[u8] {
        match self {
            ContractDeployment::Genesis { address, .. } => address,
            ContractDeployment::Regular { address, .. } => address,
        }
    }

    pub fn runtime_code(&self) -> &[u8] {
        match self {
            ContractDeployment::Genesis { runtime_code, .. } => runtime_code,
            ContractDeployment::Regular { runtime_code, .. } => runtime_code,
        }
    }

    pub fn creation_code(&self) -> Option<&[u8]> {
        match self {
            ContractDeployment::Genesis { .. } => None,
            ContractDeployment::Regular { creation_code, .. } => Some(creation_code),
        }
    }
}
