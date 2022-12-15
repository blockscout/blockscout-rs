use crate::MatchType;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

// This struct is used as input for our endpoint and as
// input for sourcify endpoint at the same time
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequest {
    pub address: String,
    pub chain: String,
    pub files: Files,
    pub chosen_contract: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Files(pub BTreeMap<String, String>);

#[derive(Debug, PartialEq, Eq)]
pub struct Success {
    pub file_name: String,
    pub contract_name: String,
    pub compiler_version: String,
    pub evm_version: String,
    pub optimization: Option<bool>,
    pub optimization_runs: Option<usize>,
    pub constructor_arguments: Option<Bytes>,
    pub contract_libraries: BTreeMap<String, String>,
    pub abi: String,
    pub sources: BTreeMap<String, String>,
    pub compiler_settings: String,
    pub match_type: MatchType,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0:#}")]
    Internal(anyhow::Error),
    #[error("verification error: {0}")]
    Verification(String),
    #[error("validation error: {0}")]
    Validation(String),
}

// Definition of sourcify.dev API response
// https://docs.sourcify.dev/docs/api/server/v1/verify/
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum ApiVerificationResponse {
    Verified {
        result: Vec<ResultItem>,
    },
    Error {
        error: String,
    },
    ValidationErrors {
        message: String,
        errors: Vec<FieldError>,
    },
}

#[derive(Deserialize, Serialize)]
pub(super) struct ResultItem {
    pub address: String,
    pub status: String,
    #[serde(rename = "storageTimestamp")]
    pub storage_timestamp: Option<String>,
}

#[derive(Deserialize, Debug, Serialize)]
pub(super) struct FieldError {
    field: String,
    message: String,
}

#[derive(Deserialize, Debug)]
pub(super) struct ApiFilesResponse {
    pub files: Vec<FileItem>,
}

#[derive(Deserialize, Debug)]
pub(super) struct FileItem {
    pub path: String,
    pub content: String,
}

impl<S: AsRef<str>> TryFrom<(ApiFilesResponse, S, S)> for Files {
    type Error = anyhow::Error;

    fn try_from((response, chain, address): (ApiFilesResponse, S, S)) -> Result<Self, Self::Error> {
        let chain = chain.as_ref();
        let address = address.as_ref();
        let files_map = response
            .files
            .into_iter()
            .map(|f| {
                let path_prefix = format!("{chain}/{address}/");
                let path = f.path.split_once(&path_prefix).ok_or_else(|| {
                    anyhow::anyhow!(
                        "file path prefix was not found: prefix={}, path={}",
                        path_prefix,
                        f.path
                    )
                })?;
                Ok((path.1.into(), f.content))
            })
            .collect::<Result<BTreeMap<String, String>, anyhow::Error>>()?;
        Ok(Files(files_map))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::parse::test_deserialize_ok;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeMap;

    #[test]
    fn deserialize_api_request() {
        let inputs = Vec::from([
            (
                r#"{
                    "address": "0xcafecafecafecafecafecafecafecafecafecafe",
                    "chain": "100",
                    "files": {
                        "source.sol": "pragma ...",
                        "metadata.json": "{ metadata: ... }"
                    }
                }"#,
                ApiRequest {
                    address: "0xcafecafecafecafecafecafecafecafecafecafe".to_string(),
                    chain: "100".to_string(),
                    files: Files(BTreeMap::from([
                        ("source.sol".to_string(), "pragma ...".to_string()),
                        ("metadata.json".to_string(), "{ metadata: ... }".to_string()),
                    ])),
                    chosen_contract: None,
                },
            ),
            (
                r#"{
                    "address": "0xcafecafecafecafecafecafecafecafecafecafe",
                    "chain": "100",
                    "files": {
                        "source.sol": "pragma ...",
                        "metadata.json": "{ metadata: ... }"
                    },
                    "chosenContract": 1
                }"#,
                ApiRequest {
                    address: "0xcafecafecafecafecafecafecafecafecafecafe".to_string(),
                    chain: "100".to_string(),
                    files: Files(BTreeMap::from([
                        ("source.sol".to_string(), "pragma ...".to_string()),
                        ("metadata.json".to_string(), "{ metadata: ... }".to_string()),
                    ])),
                    chosen_contract: Some(1),
                },
            ),
        ]);

        test_deserialize_ok(inputs);
    }

    #[test]
    fn files_try_from_api_files_response() {
        let chain = "77";
        let address = "0x5d3A6C34Ef73C557958f41A3Bd084316Edf288Cb";
        let api_files_response: ApiFilesResponse = serde_json::from_str(r#"{"status":"full","files":[{"name":"metadata.json","path":"/home/data/repository/contracts/full_match/77/0x5d3A6C34Ef73C557958f41A3Bd084316Edf288Cb/metadata.json","content":"{\"compiler\":{\"version\":\"0.7.4+commit.3f05b770\"},\"language\":\"Solidity\",\"output\":{\"abi\":[{\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"a\",\"type\":\"uint256\"},{\"internalType\":\"uint256\",\"name\":\"b\",\"type\":\"uint256\"}],\"name\":\"diff\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"a\",\"type\":\"uint256\"},{\"internalType\":\"uint256\",\"name\":\"b\",\"type\":\"uint256\"}],\"name\":\"sum\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}],\"devdoc\":{\"kind\":\"dev\",\"methods\":{},\"version\":1},\"userdoc\":{\"kind\":\"user\",\"methods\":{},\"version\":1}},\"settings\":{\"compilationTarget\":{\"contracts/Main.sol\":\"Main\"},\"evmVersion\":\"istanbul\",\"libraries\":{\"LibA\":\"0xcafecafecafecafecafecafecafecafecafecafe\",\"LibB\":\"0xcafecafecafecafecafecafecafecafecafecaf1\"},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"optimizer\":{\"enabled\":false,\"runs\":200},\"remappings\":[]},\"sources\":{\"contracts/A.sol\":{\"keccak256\":\"0xb88438c609f180510044a150fe7017d2a86aae1c82890ba06729240ed234a1a1\",\"urls\":[\"bzz-raw://0c1edf28aa7975bdcbf5b723b8c501fc9ab75c5a48cd16294dabe1a5e43c6756\",\"dweb:/ipfs/QmUACyPgpGF6jws71NTdQwFFHHxSn8sRrCi5nJHgpcrBeA\"]},\"contracts/B.sol\":{\"keccak256\":\"0x33b429a91975c6b156b0a702e76e3eb774edae46bd31655dbd1568348d494504\",\"urls\":[\"bzz-raw://0d5330365659e8f190bd7f1d29b787c0ce675bb3e0faf06614ee0b654e7eef54\",\"dweb:/ipfs/QmUz8h6LjTZTLBNRWHTQh3iu2MvD5aTieBvKm8VhqpuQ9n\"]},\"contracts/LibA.sol\":{\"keccak256\":\"0x706bd2f3238104b8938fa5822599422dccce56546825bea30f3feac86288b471\",\"urls\":[\"bzz-raw://b4b385fbe92bd184153fb2c65717bf958464a0d1972e974e90abbd94abd1c5c6\",\"dweb:/ipfs/QmUH57NDBK18piDPmcNz7nmfZCaPZJMJfozp7mgBPqcjig\"]},\"contracts/LibB.sol\":{\"keccak256\":\"0x2e976b20b42a06fef2bece371601e815061cd921837f0e3f62810b7e101315b9\",\"urls\":[\"bzz-raw://35b1fbbba0254213154baef1feacc777a095235804a603492f078d9e57040d51\",\"dweb:/ipfs/QmRRxD5xyvJvMFBDeT5c6P7y5Tq2GqZYro2iVZa3YYbZbs\"]},\"contracts/Main.sol\":{\"keccak256\":\"0xf3ccfe83d0096df6514b62fc6779f525892fda2173436138d4bc0b5271b2d024\",\"urls\":[\"bzz-raw://d80326cdd38bfdf9519d8ebf5d4435f4bf60efac2ff12a4fbea92d0efc5f2f58\",\"dweb:/ipfs/QmTwxUhbycDv9dcmqSnreMEYxmX4ACibiUk3mBHocbCcWn\"]}},\"version\":1}"},{"name":"A.sol","path":"/home/data/repository/contracts/full_match/77/0x5d3A6C34Ef73C557958f41A3Bd084316Edf288Cb/sources/contracts/A.sol","content":"pragma solidity >=0.4.24 <= 0.9.0;\n\nimport \"./LibA.sol\";\n\ncontract A {\n    function sum(uint256 a, uint256 b) external returns (uint256) {\n        return LibA.sum(a, b);\n    }\n}"},{"name":"B.sol","path":"/home/data/repository/contracts/full_match/77/0x5d3A6C34Ef73C557958f41A3Bd084316Edf288Cb/sources/contracts/B.sol","content":"pragma solidity >=0.4.24 <= 0.9.0;\n\nimport \"./LibB.sol\";\n\ncontract B {\n    function diff(uint256 a, uint256 b) external returns (uint256) {\n        return LibB.diff(a, b);\n    }\n}"},{"name":"LibA.sol","path":"/home/data/repository/contracts/full_match/77/0x5d3A6C34Ef73C557958f41A3Bd084316Edf288Cb/sources/contracts/LibA.sol","content":"pragma solidity >=0.4.24 <= 0.9.0;\n\nlibrary LibA {\n    function sum(uint256 a, uint256 b) external returns (uint256) {\n        return a + b;\n    }\n}"},{"name":"LibB.sol","path":"/home/data/repository/contracts/full_match/77/0x5d3A6C34Ef73C557958f41A3Bd084316Edf288Cb/sources/contracts/LibB.sol","content":"pragma solidity >=0.4.24 <= 0.9.0;\n\nlibrary LibB {\n    function diff(uint256 a, uint256 b) external returns (uint256) {\n        return a - b;\n    }\n}"},{"name":"Main.sol","path":"/home/data/repository/contracts/full_match/77/0x5d3A6C34Ef73C557958f41A3Bd084316Edf288Cb/sources/contracts/Main.sol","content":"pragma solidity >=0.4.24 <= 0.9.0;\n\nimport \"./A.sol\";\nimport \"./B.sol\";\n\ncontract Main is A, B {}"}]}"#).unwrap();

        let expected = Files(BTreeMap::from([
            ("metadata.json".into(), "{\"compiler\":{\"version\":\"0.7.4+commit.3f05b770\"},\"language\":\"Solidity\",\"output\":{\"abi\":[{\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"a\",\"type\":\"uint256\"},{\"internalType\":\"uint256\",\"name\":\"b\",\"type\":\"uint256\"}],\"name\":\"diff\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"a\",\"type\":\"uint256\"},{\"internalType\":\"uint256\",\"name\":\"b\",\"type\":\"uint256\"}],\"name\":\"sum\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}],\"devdoc\":{\"kind\":\"dev\",\"methods\":{},\"version\":1},\"userdoc\":{\"kind\":\"user\",\"methods\":{},\"version\":1}},\"settings\":{\"compilationTarget\":{\"contracts/Main.sol\":\"Main\"},\"evmVersion\":\"istanbul\",\"libraries\":{\"LibA\":\"0xcafecafecafecafecafecafecafecafecafecafe\",\"LibB\":\"0xcafecafecafecafecafecafecafecafecafecaf1\"},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"optimizer\":{\"enabled\":false,\"runs\":200},\"remappings\":[]},\"sources\":{\"contracts/A.sol\":{\"keccak256\":\"0xb88438c609f180510044a150fe7017d2a86aae1c82890ba06729240ed234a1a1\",\"urls\":[\"bzz-raw://0c1edf28aa7975bdcbf5b723b8c501fc9ab75c5a48cd16294dabe1a5e43c6756\",\"dweb:/ipfs/QmUACyPgpGF6jws71NTdQwFFHHxSn8sRrCi5nJHgpcrBeA\"]},\"contracts/B.sol\":{\"keccak256\":\"0x33b429a91975c6b156b0a702e76e3eb774edae46bd31655dbd1568348d494504\",\"urls\":[\"bzz-raw://0d5330365659e8f190bd7f1d29b787c0ce675bb3e0faf06614ee0b654e7eef54\",\"dweb:/ipfs/QmUz8h6LjTZTLBNRWHTQh3iu2MvD5aTieBvKm8VhqpuQ9n\"]},\"contracts/LibA.sol\":{\"keccak256\":\"0x706bd2f3238104b8938fa5822599422dccce56546825bea30f3feac86288b471\",\"urls\":[\"bzz-raw://b4b385fbe92bd184153fb2c65717bf958464a0d1972e974e90abbd94abd1c5c6\",\"dweb:/ipfs/QmUH57NDBK18piDPmcNz7nmfZCaPZJMJfozp7mgBPqcjig\"]},\"contracts/LibB.sol\":{\"keccak256\":\"0x2e976b20b42a06fef2bece371601e815061cd921837f0e3f62810b7e101315b9\",\"urls\":[\"bzz-raw://35b1fbbba0254213154baef1feacc777a095235804a603492f078d9e57040d51\",\"dweb:/ipfs/QmRRxD5xyvJvMFBDeT5c6P7y5Tq2GqZYro2iVZa3YYbZbs\"]},\"contracts/Main.sol\":{\"keccak256\":\"0xf3ccfe83d0096df6514b62fc6779f525892fda2173436138d4bc0b5271b2d024\",\"urls\":[\"bzz-raw://d80326cdd38bfdf9519d8ebf5d4435f4bf60efac2ff12a4fbea92d0efc5f2f58\",\"dweb:/ipfs/QmTwxUhbycDv9dcmqSnreMEYxmX4ACibiUk3mBHocbCcWn\"]}},\"version\":1}".into()),
            ("sources/contracts/A.sol".into(), "pragma solidity >=0.4.24 <= 0.9.0;\n\nimport \"./LibA.sol\";\n\ncontract A {\n    function sum(uint256 a, uint256 b) external returns (uint256) {\n        return LibA.sum(a, b);\n    }\n}".into()),
            ("sources/contracts/B.sol".into(), "pragma solidity >=0.4.24 <= 0.9.0;\n\nimport \"./LibB.sol\";\n\ncontract B {\n    function diff(uint256 a, uint256 b) external returns (uint256) {\n        return LibB.diff(a, b);\n    }\n}".into()),
            ("sources/contracts/LibA.sol".into(), "pragma solidity >=0.4.24 <= 0.9.0;\n\nlibrary LibA {\n    function sum(uint256 a, uint256 b) external returns (uint256) {\n        return a + b;\n    }\n}".into()),
            ("sources/contracts/LibB.sol".into(), "pragma solidity >=0.4.24 <= 0.9.0;\n\nlibrary LibB {\n    function diff(uint256 a, uint256 b) external returns (uint256) {\n        return a - b;\n    }\n}".into()),
            ("sources/contracts/Main.sol".into(), "pragma solidity >=0.4.24 <= 0.9.0;\n\nimport \"./A.sol\";\nimport \"./B.sol\";\n\ncontract Main is A, B {}".into()),
        ]));

        let files = Files::try_from((api_files_response, chain, address))
            .expect("Conversion should be valid");
        assert_eq!(expected, files, "Invalid result");
    }
}
