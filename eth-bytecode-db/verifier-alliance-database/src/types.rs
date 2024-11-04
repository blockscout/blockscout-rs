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
pub struct RetrieveContractDeployment {
    pub(crate) chain_id: u128,
    pub(crate) address: Vec<u8>,
    pub(crate) transaction_hash: Option<Vec<u8>>,
    pub(crate) runtime_code: Option<Vec<u8>>,
}

impl RetrieveContractDeployment {
    pub fn regular(chain_id: u128, address: Vec<u8>, transaction_hash: Vec<u8>) -> Self {
        Self {
            chain_id,
            address,
            transaction_hash: Some(transaction_hash),
            runtime_code: None,
        }
    }

    pub fn genesis(chain_id: u128, address: Vec<u8>, runtime_code: Vec<u8>) -> Self {
        Self {
            chain_id,
            address,
            transaction_hash: None,
            runtime_code: Some(runtime_code),
        }
    }

    pub fn chain_id(&self) -> u128 {
        self.chain_id
    }

    pub fn address(&self) -> &[u8] {
        &self.address
    }
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
