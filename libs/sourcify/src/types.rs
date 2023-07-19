use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchType {
    Full,
    Partial,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Error {
    pub error: String,
}

pub use get_source_files_response::GetSourceFilesResponse;
mod get_source_files_response {
    use super::*;

    pub type Files = Vec<File>;

    #[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
    pub struct File {
        pub name: String,
        pub path: String,
        pub content: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
    pub struct GetSourceFilesResponse {
        pub status: MatchType,
        pub files: Files,
    }
}

#[cfg(test)]
mod tests {
    use super::{get_source_files_response::*, *};
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::fmt::Debug;

    fn check<T: PartialEq + Debug + for<'de> Deserialize<'de>>(
        value: serde_json::Value,
        expected: T,
        msg_prefix: Option<&str>,
    ) {
        let msg_prefix = msg_prefix.map(|msg| format!("{msg} ")).unwrap_or_default();
        let result: T = serde_json::from_value(value)
            .unwrap_or_else(|_| panic!("{msg_prefix}deserialization failed"));
        assert_eq!(expected, result, "{msg_prefix}check failed");
    }

    #[test]
    fn parse_error_response() {
        let value = json!({
            "error": "Invalid address: 0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c51",
            "message": "Invalid address: 0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c51"
        });
        let expected = Error {
            error: "Invalid address: 0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c51".to_string(),
        };
        check(value, expected, None);
    }

    #[test]
    fn parse_get_source_files_response() {
        /********** Full match response deserialization **********/

        let value = json!({
            "status": "full",
            "files": [
                {
                    "name": "library-map.json",
                    "path": "/home/data/repository/contracts/full_match/5/0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52/library-map.json",
                    "content": "{\"__$54103d3e1543ebb87230c9454f838057a5$__\":\"6b88c55cfbd4eda1320f802b724193cab062ccce\"}"
                },
                {
                    "name": "metadata.json",
                    "path": "/home/data/repository/contracts/full_match/5/0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52/metadata.json",
                    "content": "{\"compiler\":{\"version\":\"0.6.8+commit.0bbfe453\"},\"language\":\"Solidity\",\"output\":{\"abi\":[{\"anonymous\":false,\"inputs\":[],\"name\":\"SourcifySolidity14\",\"type\":\"event\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"input\",\"type\":\"address\"}],\"name\":\"identity\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"\",\"type\":\"address\"}],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}],\"devdoc\":{\"methods\":{}},\"userdoc\":{\"methods\":{}}},\"settings\":{\"compilationTarget\":{\"contracts/project:/ExternalTestMultiple.sol\":\"ExternalTestMultiple\"},\"evmVersion\":\"istanbul\",\"libraries\":{},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"optimizer\":{\"enabled\":true,\"runs\":300},\"remappings\":[]},\"sources\":{\"contracts/project:/ExternalTestMultiple.sol\":{\"keccak256\":\"0xc40380283b7d4a97da5e247fbb7b795f6241cfe3d86e34493d87528dfcb4d56b\",\"license\":\"MIT\",\"urls\":[\"bzz-raw://86ec578963cb912c4b912f066390e564c54ea1bc5fb1a55aa4e4c77bb92b07ba\",\"dweb:/ipfs/QmeqihJa8kUjbNHNCpFRHkq1scCbjjFvaUN2gWEJCNEx1Q\"]},\"contracts/project_/ExternalTestMultiple.sol\":{\"keccak256\":\"0xff9e0ddd21b0579491371fe8d4f7e09254ffc7af9382ba287ef8d2a2fd1ce8e2\",\"license\":\"MIT\",\"urls\":[\"bzz-raw://1f516a34091c829a18a8c5dd13fbd82f44b532e7dea6fed9f60ae731c9042d74\",\"dweb:/ipfs/QmZqm6CLGUKQ3RJCLAZy5CWo2ScLzV2r5JXWNWfBwbGCsK\"]}},\"version\":1}"
                },
                {
                    "name": "ExternalTestMultiple.sol",
                    "path": "/home/data/repository/contracts/full_match/5/0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52/sources/contracts/project_/ExternalTestMultiple.sol",
                    "content": "//SPDX-License-Identifier: MIT\r\npragma solidity ^0.6.8;\r\n\r\nlibrary ExternalTestLibraryMultiple {\r\n  function pop(address[] storage list) external returns (address out) {\r\n    out = list[list.length - 1];\r\n    list.pop();\r\n  }\r\n}\r\n"
                }
            ]
        });
        let expected = GetSourceFilesResponse {
            status: MatchType::Full,
            files: vec![
                File {
                    name: "library-map.json".to_string(),
                    path: "/home/data/repository/contracts/full_match/5/0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52/library-map.json".to_string(),
                    content: "{\"__$54103d3e1543ebb87230c9454f838057a5$__\":\"6b88c55cfbd4eda1320f802b724193cab062ccce\"}".to_string()
                },
                File {
                    name: "metadata.json".to_string(),
                    path: "/home/data/repository/contracts/full_match/5/0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52/metadata.json".to_string(),
                    content: "{\"compiler\":{\"version\":\"0.6.8+commit.0bbfe453\"},\"language\":\"Solidity\",\"output\":{\"abi\":[{\"anonymous\":false,\"inputs\":[],\"name\":\"SourcifySolidity14\",\"type\":\"event\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"input\",\"type\":\"address\"}],\"name\":\"identity\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"\",\"type\":\"address\"}],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}],\"devdoc\":{\"methods\":{}},\"userdoc\":{\"methods\":{}}},\"settings\":{\"compilationTarget\":{\"contracts/project:/ExternalTestMultiple.sol\":\"ExternalTestMultiple\"},\"evmVersion\":\"istanbul\",\"libraries\":{},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"optimizer\":{\"enabled\":true,\"runs\":300},\"remappings\":[]},\"sources\":{\"contracts/project:/ExternalTestMultiple.sol\":{\"keccak256\":\"0xc40380283b7d4a97da5e247fbb7b795f6241cfe3d86e34493d87528dfcb4d56b\",\"license\":\"MIT\",\"urls\":[\"bzz-raw://86ec578963cb912c4b912f066390e564c54ea1bc5fb1a55aa4e4c77bb92b07ba\",\"dweb:/ipfs/QmeqihJa8kUjbNHNCpFRHkq1scCbjjFvaUN2gWEJCNEx1Q\"]},\"contracts/project_/ExternalTestMultiple.sol\":{\"keccak256\":\"0xff9e0ddd21b0579491371fe8d4f7e09254ffc7af9382ba287ef8d2a2fd1ce8e2\",\"license\":\"MIT\",\"urls\":[\"bzz-raw://1f516a34091c829a18a8c5dd13fbd82f44b532e7dea6fed9f60ae731c9042d74\",\"dweb:/ipfs/QmZqm6CLGUKQ3RJCLAZy5CWo2ScLzV2r5JXWNWfBwbGCsK\"]}},\"version\":1}".to_string()
                },
                File {
                    name: "ExternalTestMultiple.sol".to_string(),
                    path: "/home/data/repository/contracts/full_match/5/0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52/sources/contracts/project_/ExternalTestMultiple.sol".to_string(),
                    content: "//SPDX-License-Identifier: MIT\r\npragma solidity ^0.6.8;\r\n\r\nlibrary ExternalTestLibraryMultiple {\r\n  function pop(address[] storage list) external returns (address out) {\r\n    out = list[list.length - 1];\r\n    list.pop();\r\n  }\r\n}\r\n".to_string()
                }
            ],
        };
        check(value, expected, Some("full match"));

        /********** Partial match response deserialization **********/

        let value = json!({
            "status": "partial",
            "files": [
                {
                    "name": "metadata.json",
                    "path": "/home/data/repository/contracts/partial_match/420/0x341577fB771EBFB4FaF74fBcF786d4F7Ce02BBaB/metadata.json",
                    "content": "{\"compiler\":{\"version\":\"0.5.17+commit.d19bba13\"},\"language\":\"Solidity\",\"output\":{\"abi\":[{\"constant\":true,\"inputs\":[],\"name\":\"a\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"payable\":false,\"stateMutability\":\"view\",\"type\":\"function\"},{\"constant\":true,\"inputs\":[],\"name\":\"a_plus_one\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"payable\":false,\"stateMutability\":\"view\",\"type\":\"function\"},{\"constant\":false,\"inputs\":[],\"name\":\"dead\",\"outputs\":[],\"payable\":false,\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"constant\":true,\"inputs\":[],\"name\":\"helloworld\",\"outputs\":[{\"internalType\":\"string\",\"name\":\"\",\"type\":\"string\"}],\"payable\":false,\"stateMutability\":\"view\",\"type\":\"function\"}],\"devdoc\":{\"methods\":{}},\"userdoc\":{\"methods\":{}}},\"settings\":{\"compilationTarget\":{\"Reinit_Poc.sol\":\"Reinit_Poc\"},\"evmVersion\":\"istanbul\",\"libraries\":{},\"optimizer\":{\"enabled\":false,\"runs\":200},\"remappings\":[]},\"sources\":{\"Reinit_Poc.sol\":{\"keccak256\":\"0x37b3c1d0756395feecb440eb088e3de92e7300db1770e5e00f29ffc22c83ad28\",\"urls\":[\"bzz-raw://f1877139b66f70c9cebcee66f25879cd611e00b6ec658ea5009463e97768c0d3\",\"dweb:/ipfs/QmcVf6NsWfFGN9UTbmWGkuQJxo7EoGXLq6LxRnu4B78fDj\"]}},\"version\":1}"
                },
                {
                    "name": "Reinit_Poc.sol",
                    "path": "/home/data/repository/contracts/partial_match/420/0x341577fB771EBFB4FaF74fBcF786d4F7Ce02BBaB/sources/Reinit_Poc.sol",
                    "content": "contract Reinit_Poc {\r\n    uint public constant a = 999;\r\n    function a_plus_one() public view returns(uint){\r\n        return a+1;\r\n    }\r\n    function helloworld() public view returns(string memory){\r\n        return \"hello world\";\r\n    }\r\n    function dead() public{\r\n        selfdestruct(msg.sender);\r\n    }\r\n}"
                }
            ]
        });
        let expected = GetSourceFilesResponse {
            status: MatchType::Partial,
            files: vec![
                File {
                    name: "metadata.json".to_string(),
                    path: "/home/data/repository/contracts/partial_match/420/0x341577fB771EBFB4FaF74fBcF786d4F7Ce02BBaB/metadata.json".to_string(),
                    content: "{\"compiler\":{\"version\":\"0.5.17+commit.d19bba13\"},\"language\":\"Solidity\",\"output\":{\"abi\":[{\"constant\":true,\"inputs\":[],\"name\":\"a\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"payable\":false,\"stateMutability\":\"view\",\"type\":\"function\"},{\"constant\":true,\"inputs\":[],\"name\":\"a_plus_one\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"payable\":false,\"stateMutability\":\"view\",\"type\":\"function\"},{\"constant\":false,\"inputs\":[],\"name\":\"dead\",\"outputs\":[],\"payable\":false,\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"constant\":true,\"inputs\":[],\"name\":\"helloworld\",\"outputs\":[{\"internalType\":\"string\",\"name\":\"\",\"type\":\"string\"}],\"payable\":false,\"stateMutability\":\"view\",\"type\":\"function\"}],\"devdoc\":{\"methods\":{}},\"userdoc\":{\"methods\":{}}},\"settings\":{\"compilationTarget\":{\"Reinit_Poc.sol\":\"Reinit_Poc\"},\"evmVersion\":\"istanbul\",\"libraries\":{},\"optimizer\":{\"enabled\":false,\"runs\":200},\"remappings\":[]},\"sources\":{\"Reinit_Poc.sol\":{\"keccak256\":\"0x37b3c1d0756395feecb440eb088e3de92e7300db1770e5e00f29ffc22c83ad28\",\"urls\":[\"bzz-raw://f1877139b66f70c9cebcee66f25879cd611e00b6ec658ea5009463e97768c0d3\",\"dweb:/ipfs/QmcVf6NsWfFGN9UTbmWGkuQJxo7EoGXLq6LxRnu4B78fDj\"]}},\"version\":1}".to_string()
                },
                File {
                    name: "Reinit_Poc.sol".to_string(),
                    path: "/home/data/repository/contracts/partial_match/420/0x341577fB771EBFB4FaF74fBcF786d4F7Ce02BBaB/sources/Reinit_Poc.sol".to_string(),
                    content: "contract Reinit_Poc {\r\n    uint public constant a = 999;\r\n    function a_plus_one() public view returns(uint){\r\n        return a+1;\r\n    }\r\n    function helloworld() public view returns(string memory){\r\n        return \"hello world\";\r\n    }\r\n    function dead() public{\r\n        selfdestruct(msg.sender);\r\n    }\r\n}".to_string(),
                }
            ]
        };
        check(value, expected, Some("partial match"));
    }
}
