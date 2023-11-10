use crate::{
    proto,
    types::{MatchTypeWrapper, SourceTypeWrapper},
};
use amplify::{From, Wrapper};
use blockscout_display_bytes::Bytes as DisplayBytes;
use eth_bytecode_db::{search, verification};

#[derive(Wrapper, From, Clone, Debug, PartialEq)]
pub struct SourceWrapper(proto::Source);

impl From<verification::Source> for SourceWrapper {
    fn from(value: verification::Source) -> Self {
        let source_type = SourceTypeWrapper::from(value.source_type).into_inner();
        let match_type = MatchTypeWrapper::from(value.match_type).into_inner();
        proto::Source {
            file_name: value.file_name,
            contract_name: value.contract_name,
            compiler_version: value.compiler_version,
            compiler_settings: value.compiler_settings,
            source_type: source_type.into(),
            source_files: value.source_files,
            abi: value.abi,
            constructor_arguments: value.constructor_arguments,
            match_type: match_type.into(),
            compilation_artifacts: value.compilation_artifacts,
            creation_input_artifacts: value.creation_input_artifacts,
            deployed_bytecode_artifacts: value.deployed_bytecode_artifacts,
        }
        .into()
    }
}

impl From<search::MatchContract> for SourceWrapper {
    fn from(value: search::MatchContract) -> Self {
        let source_type = SourceTypeWrapper::from(value.source_type).into_inner();
        let match_type = MatchTypeWrapper::from(value.match_type).into_inner();
        proto::Source {
            file_name: value.file_name,
            contract_name: value.contract_name,
            compiler_version: value.compiler_version,
            compiler_settings: value.compiler_settings,
            source_type: source_type.into(),
            source_files: value.source_files,
            abi: value.abi,
            constructor_arguments: value.constructor_arguments,
            match_type: match_type.into(),
            compilation_artifacts: value.compilation_artifacts,
            creation_input_artifacts: value.creation_input_artifacts,
            deployed_bytecode_artifacts: value.deployed_bytecode_artifacts,
        }
        .into()
    }
}

impl TryFrom<sourcify::GetSourceFilesResponse> for SourceWrapper {
    type Error = tonic::Status;

    fn try_from(value: sourcify::GetSourceFilesResponse) -> Result<Self, Self::Error> {
        let match_type = MatchTypeWrapper::from(value.status).into_inner();

        let metadata: ethers::solc::artifacts::Metadata =
            serde_json::from_value(value.metadata.clone()).map_err(|err| {
                tracing::error!(target: "sourcify", "returned metadata cannot be parsed: {err}");
                tonic::Status::internal("error occurred when parsing sourcify response")
            })?;

        // Compiler settings inside metadata contains a "compilationTarget"
        // which does not exist in compiler input. We should remove the key
        // to make the settings which could be used for the compiler input.
        let compiler_settings = {
            let mut compiler_settings = value
                .metadata
                .as_object()
                .expect("metadata has been parsed successfully and must be an object")
                .get("settings")
                .expect("metadata has been parsed successfully and must contain 'settings' key")
                .as_object()
                .expect("metadata has been parsed successfully and 'settings' must be an object")
                .clone();

            compiler_settings.remove("compilationTarget");
            compiler_settings
        };

        let abi = value
            .metadata
            .as_object()
            .expect("metadata has been parsed successfully and must be an object")
            .get("output")
            .expect("metadata has been parsed successfully and must contain 'output' key")
            .as_object()
            .expect("metadata has been parsed successfully and 'output' must be an object")
            .get("abi")
            .expect("metadata has been parsed successfully and must contain 'output.abi' key")
            .clone();

        let (file_name, contract_name) = metadata.settings.compilation_target.into_iter()
            .next().ok_or_else(|| {
            tracing::error!(target: "sourcify", "returned metadata does not contain any compilation target");
            tonic::Status::internal("error occurred when parsing sourcify response")
        })?;

        let constructor_arguments = value
            .constructor_arguments
            .map(|v| DisplayBytes::from(v).to_string());

        Ok(proto::Source {
            file_name,
            contract_name,
            compiler_version: metadata.compiler.version,
            compiler_settings: serde_json::to_string(&compiler_settings).unwrap(),
            source_type: proto::source::SourceType::Solidity.into(),
            source_files: value.sources,
            abi: Some(serde_json::to_string(&abi).unwrap()),
            constructor_arguments,
            match_type: match_type.into(),
            compilation_artifacts: None,
            creation_input_artifacts: None,
            deployed_bytecode_artifacts: None,
        }
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeMap;

    #[test]
    fn from_verification_source_to_proto_source() {
        let verification_source = verification::Source {
            file_name: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "compiler_version".to_string(),
            compiler_settings: "compiler_settings".to_string(),
            source_type: verification::SourceType::Solidity,
            source_files: BTreeMap::from([("source".into(), "content".into())]),
            abi: Some("abi".into()),
            constructor_arguments: Some("args".into()),
            match_type: verification::MatchType::Partial,
            compilation_artifacts: Some("compilation_artifacts".into()),
            creation_input_artifacts: Some("creation_input_artifacts".into()),
            deployed_bytecode_artifacts: Some("deployed_bytecode_artifacts".into()),
            raw_creation_input: vec![0u8, 1u8, 2u8, 3u8, 4u8],
            raw_deployed_bytecode: vec![5u8, 6u8, 7u8, 8u8],
            creation_input_parts: vec![
                verification::BytecodePart::Main {
                    data: vec![0u8, 1u8],
                },
                verification::BytecodePart::Meta {
                    data: vec![3u8, 4u8],
                },
            ],
            deployed_bytecode_parts: vec![
                verification::BytecodePart::Main {
                    data: vec![5u8, 6u8],
                },
                verification::BytecodePart::Meta {
                    data: vec![7u8, 8u8],
                },
            ],
        };

        let expected = proto::Source {
            file_name: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "compiler_version".to_string(),
            compiler_settings: "compiler_settings".to_string(),
            source_type: proto::source::SourceType::Solidity.into(),
            source_files: BTreeMap::from([("source".into(), "content".into())]),
            abi: Some("abi".into()),
            constructor_arguments: Some("args".into()),
            match_type: proto::source::MatchType::Partial.into(),
            compilation_artifacts: Some("compilation_artifacts".into()),
            creation_input_artifacts: Some("creation_input_artifacts".into()),
            deployed_bytecode_artifacts: Some("deployed_bytecode_artifacts".into()),
        };

        let result = SourceWrapper::from(verification_source).into_inner();
        assert_eq!(expected, result);
    }

    #[test]
    fn from_search_source_to_proto_source() {
        let search_source = search::MatchContract {
            updated_at: Default::default(),
            file_name: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "compiler_version".to_string(),
            compiler_settings: "compiler_settings".to_string(),
            source_type: verification::SourceType::Solidity,
            source_files: BTreeMap::from([("source".into(), "content".into())]),
            abi: Some("abi".into()),
            constructor_arguments: Some("args".into()),
            match_type: verification::MatchType::Partial,
            compilation_artifacts: Some("compilation_artifacts".into()),
            creation_input_artifacts: Some("creation_input_artifacts".into()),
            deployed_bytecode_artifacts: Some("deployed_bytecode_artifacts".into()),
            raw_creation_input: vec![0u8, 1u8, 2u8, 3u8, 4u8],
            raw_deployed_bytecode: vec![5u8, 6u8, 7u8, 8u8],
        };

        let expected = proto::Source {
            file_name: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "compiler_version".to_string(),
            compiler_settings: "compiler_settings".to_string(),
            source_type: proto::source::SourceType::Solidity.into(),
            source_files: BTreeMap::from([("source".into(), "content".into())]),
            abi: Some("abi".into()),
            constructor_arguments: Some("args".into()),
            match_type: proto::source::MatchType::Partial.into(),
            compilation_artifacts: Some("compilation_artifacts".into()),
            creation_input_artifacts: Some("creation_input_artifacts".into()),
            deployed_bytecode_artifacts: Some("deployed_bytecode_artifacts".into()),
        };

        let result = SourceWrapper::from(search_source).into_inner();
        assert_eq!(expected, result);
    }

    #[test]
    fn try_from_sourcify_get_source_files_response_to_proto_source() {
        let sourcify_response: sourcify::GetSourceFilesResponse = serde_json::from_value(serde_json::json!({
            "status": "full",
            "files": [
                {
                    "name": "immutable-references.json",
                    "path": "/data/repository/contracts/full_match/5/0xb5fa8e1E33df0742Fa40F7bB5f41e782eb5524c3/immutable-references.json",
                    "content": "{\"76\":[{\"length\":32,\"start\":242}],\"90\":[{\"length\":32,\"start\":636}],\"92\":[{\"length\":32,\"start\":708}]}"
                },
                {
                    "name": "library-map.json",
                    "path": "/data/repository/contracts/full_match/5/0xb5fa8e1E33df0742Fa40F7bB5f41e782eb5524c3/library-map.json",
                    "content": "{\"__$b01ade520cf6862e0b214d4ce779d49f2a$__\":\"77479d54e233b4b79b9e3f4cf2bd20575fdeb1bb\",\"__$21bb6923d223bd045c4cab9806bdf3594d$__\":\"4ea76bb37c82f6f453c4cbb4ae726036a7a8b820\"}"
                },
                {
                    "name": "metadata.json",
                    "path": "/data/repository/contracts/full_match/5/0xb5fa8e1E33df0742Fa40F7bB5f41e782eb5524c3/metadata.json",
                    "content": "{\"compiler\":{\"version\":\"0.8.7+commit.e28d00a7\"},\"language\":\"Solidity\",\"output\":{\"abi\":[{\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"_c\",\"type\":\"uint256\"}],\"stateMutability\":\"nonpayable\",\"type\":\"constructor\"},{\"inputs\":[],\"name\":\"a\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"name\":\"arr\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"b\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"c\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"test_array\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}],\"devdoc\":{\"kind\":\"dev\",\"methods\":{},\"version\":1},\"userdoc\":{\"kind\":\"user\",\"methods\":{},\"version\":1}},\"settings\":{\"compilationTarget\":{\"contracts/C2.sol\":\"C2\"},\"evmVersion\":\"london\",\"libraries\":{},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"optimizer\":{\"enabled\":false,\"runs\":200},\"remappings\":[]},\"sources\":{\"contracts/Array.sol\":{\"keccak256\":\"0xa3c043d47c03251f392e0dcdf7c2ad5069436cc72561b2404a195d84a78fb33c\",\"license\":\"MIT\",\"urls\":[\"bzz-raw://9eeae2fe8bb3e5addd48dea0b8c5a81581b4137ad605b103b7edc35ea1ab2997\",\"dweb:/ipfs/QmRaC1FTQxkcWwmyZeZ4twQswdpcTnN5GQJ2Db7RMdf14j\"]},\"contracts/Array2.sol\":{\"keccak256\":\"0x6b5b4acc9b4f10f7df803702516752978c40cb4eadd48a218dcc0503fd7f4f88\",\"license\":\"MIT\",\"urls\":[\"bzz-raw://6f9b39fbd016e6b0c62cb3a7d8abc8981fe383c345381dfd7533a8cb8a1ded69\",\"dweb:/ipfs/QmUznGnpnz1TFuQZzjDX4NzoVFsiwRzaA2T7gZQZUoFDim\"]},\"contracts/C1.sol\":{\"keccak256\":\"0x92ca0ef5999e6bae5556c90314c3b7fe3bf95cabf8423d8415ac3d3955f524df\",\"license\":\"MIT\",\"urls\":[\"bzz-raw://1bb5f1bc39e5507da05e9d7700546f74896b47c4da076fbd744d8b6269c32a09\",\"dweb:/ipfs/QmRoabfGHHmoKTPb8vtdWFuaesoc4mUjzuSgEDE8TpeYwF\"]},\"contracts/C2.sol\":{\"keccak256\":\"0x20dd630583b88340fae8cda0a6008c0db058ee4b35d8a1f5a44bcd66ef892782\",\"license\":\"MIT\",\"urls\":[\"bzz-raw://998e0420f165f61fec241b5fbd71f06bc150098c8981f0aeedbf5a95c240b2a0\",\"dweb:/ipfs/QmSG3mwV4zqk4NacHtf3MWSAbWW96sorEVjNBPJiWXs2LQ\"]}},\"version\":1}"
                },
                {
                    "name": "Array.sol",
                    "path": "/data/repository/contracts/full_match/5/0xb5fa8e1E33df0742Fa40F7bB5f41e782eb5524c3/sources/contracts/Array.sol",
                    "content": "// SPDX-License-Identifier: MIT\npragma solidity 0.8.7;\n\n// Array function to delete element at index and re-organize the array\n// so that there are no gaps between the elements.\nlibrary Array {\n    function remove(uint[] storage arr, uint index) public {\n        // Move the last element into the place to delete\n        require(arr.length > 0, \"Can't remove from empty array\");\n        arr[index] = arr[arr.length - 1];\n        arr.pop();\n    }\n}\n"
                },
                {
                    "name": "Array2.sol",
                    "path": "/data/repository/contracts/full_match/5/0xb5fa8e1E33df0742Fa40F7bB5f41e782eb5524c3/sources/contracts/Array2.sol",
                    "content": "// SPDX-License-Identifier: MIT\npragma solidity 0.8.7;\n\n// Array function to delete element at index and re-organize the array\n// so that there are no gaps between the elements.\nlibrary Array2 {\n    function remove(uint[] storage arr, uint index) public {\n        // Move the last element into the place to delete\n        require(arr.length > 0, \"Can't remove from empty array\");\n        arr[index] = arr[arr.length - 1];\n        arr.pop();\n    }\n}\n"
                },
                {
                    "name": "C1.sol",
                    "path": "/data/repository/contracts/full_match/5/0xb5fa8e1E33df0742Fa40F7bB5f41e782eb5524c3/sources/contracts/C1.sol",
                    "content": "// SPDX-License-Identifier: MIT\npragma solidity 0.8.7;\n\ncontract C1 {\n    uint256 immutable public a = 0;\n}\n\n"
                },
                {
                    "name": "C2.sol",
                    "path": "/data/repository/contracts/full_match/5/0xb5fa8e1E33df0742Fa40F7bB5f41e782eb5524c3/sources/contracts/C2.sol",
                    "content": "// SPDX-License-Identifier: MIT\npragma solidity 0.8.7;\n\nimport \"./C1.sol\";\nimport \"./Array.sol\";\nimport \"./Array2.sol\";\n\ncontract C2 is C1 {\n    uint[] public arr;\n\n    uint256 immutable public b = 10;\n    uint256 immutable public c;\n\n    constructor(uint256 _c) {\n        c = _c;\n    }\n\n    function test_array() public {\n        for (uint i = 0; i < 3; i++) {\n            arr.push(i);\n        }\n\n        Array.remove(arr, 1);\n        Array2.remove(arr, 1);\n\n        assert(arr.length == 1);\n        assert(arr[0] == 0);\n    }\n}\n"
                }
            ]
        })).unwrap();

        let expected = proto::Source {
            file_name: "contracts/C2.sol".to_string(),
            contract_name: "C2".to_string(),
            compiler_version: "0.8.7+commit.e28d00a7".to_string(),
            compiler_settings: "{\"evmVersion\":\"london\",\"libraries\":{},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"optimizer\":{\"enabled\":false,\"runs\":200},\"remappings\":[]}".to_string(),
            source_type: proto::source::SourceType::Solidity.into(),
            source_files: BTreeMap::from([
                ("contracts/Array.sol".into(), "// SPDX-License-Identifier: MIT\npragma solidity 0.8.7;\n\n// Array function to delete element at index and re-organize the array\n// so that there are no gaps between the elements.\nlibrary Array {\n    function remove(uint[] storage arr, uint index) public {\n        // Move the last element into the place to delete\n        require(arr.length > 0, \"Can't remove from empty array\");\n        arr[index] = arr[arr.length - 1];\n        arr.pop();\n    }\n}\n".into()),
                ("contracts/Array2.sol".into(), "// SPDX-License-Identifier: MIT\npragma solidity 0.8.7;\n\n// Array function to delete element at index and re-organize the array\n// so that there are no gaps between the elements.\nlibrary Array2 {\n    function remove(uint[] storage arr, uint index) public {\n        // Move the last element into the place to delete\n        require(arr.length > 0, \"Can't remove from empty array\");\n        arr[index] = arr[arr.length - 1];\n        arr.pop();\n    }\n}\n".into()),
                ("contracts/C1.sol".into(), "// SPDX-License-Identifier: MIT\npragma solidity 0.8.7;\n\ncontract C1 {\n    uint256 immutable public a = 0;\n}\n\n".into()),
                ("contracts/C2.sol".into(), "// SPDX-License-Identifier: MIT\npragma solidity 0.8.7;\n\nimport \"./C1.sol\";\nimport \"./Array.sol\";\nimport \"./Array2.sol\";\n\ncontract C2 is C1 {\n    uint[] public arr;\n\n    uint256 immutable public b = 10;\n    uint256 immutable public c;\n\n    constructor(uint256 _c) {\n        c = _c;\n    }\n\n    function test_array() public {\n        for (uint i = 0; i < 3; i++) {\n            arr.push(i);\n        }\n\n        Array.remove(arr, 1);\n        Array2.remove(arr, 1);\n\n        assert(arr.length == 1);\n        assert(arr[0] == 0);\n    }\n}\n".into()),
            ]),
            abi: Some("[{\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"_c\",\"type\":\"uint256\"}],\"stateMutability\":\"nonpayable\",\"type\":\"constructor\"},{\"inputs\":[],\"name\":\"a\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"name\":\"arr\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"b\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"c\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"test_array\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}]".into()),
            constructor_arguments: None,
            match_type: proto::source::MatchType::Full.into(),
            compilation_artifacts: None,
            creation_input_artifacts: None,
            deployed_bytecode_artifacts: None,
        };

        let result = SourceWrapper::try_from(sourcify_response)
            .expect("converting into SourceWrapper failed")
            .into_inner();
        assert_eq!(expected, result);
    }
}
