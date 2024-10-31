pub enum ContractCode {
    OnlyCreationCode {
        code: Vec<u8>,
    },
    OnlyRuntimeCode {
        code: Vec<u8>,
    },
    CompleteCode {
        creation_code: Vec<u8>,
        runtime_code: Vec<u8>,
    },
}
