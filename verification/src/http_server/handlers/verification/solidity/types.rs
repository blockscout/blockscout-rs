use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, PartialEq)]
pub struct VerificationRequest<T> {
    pub contract_name: String,
    pub deployed_bytecode: String,
    pub creation_bytecode: String,
    pub compiler_version: String,
    pub constructor_arguments: Option<String>,

    #[serde(flatten)]
    pub content: T,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct VerificationResponse {
    pub verified: bool,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum EvmVersion {
    Homestead,
    TangerineWhistle,
    SpuriousDragon,
    Byzantium,
    Constantinople,
    Petersburg,
    Istanbul,
    Berlin,
    London,
    Default,
}

#[derive(Debug, Deserialize, PartialEq)]
struct ContractLibrary {
    lib_name: String,
    lib_address: String,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct FlattenedSource {
    source_code: String,
    evm_version: EvmVersion,
    optimization_runs: Option<u32>,
    contract_libraries: Option<Vec<ContractLibrary>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    use std::fmt::Debug;

    fn test_parse_ok<T>(tests: Vec<(&str, T)>)
    where
        T: Debug + PartialEq + DeserializeOwned,
    {
        for (s, value) in tests {
            let v: T = serde_json::from_str(s).unwrap();
            assert_eq!(v, value);
        }
    }

    #[test]
    fn verification_request() {
        test_parse_ok(vec![(
            r#"{
                    "contract_name": "test",
                    "deployed_bytecode": "0x6001",
                    "creation_bytecode": "0x6001",
                    "compiler_version": "test",
                    "source_code": "pragma",
                    "evm_version": "london",
                    "optimization_runs": 200
                }"#,
            VerificationRequest::<FlattenedSource> {
                contract_name: "test".into(),
                deployed_bytecode: "0x6001".into(),
                creation_bytecode: "0x6001".into(),
                compiler_version: "test".into(),
                constructor_arguments: None,
                content: FlattenedSource {
                    source_code: "pragma".into(),
                    evm_version: EvmVersion::London,
                    optimization_runs: Some(200),
                    contract_libraries: None,
                },
            },
        )])
    }
}
