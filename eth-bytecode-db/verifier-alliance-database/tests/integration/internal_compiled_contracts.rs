use crate::from_json;
use blockscout_service_launcher::test_database::database;
use std::collections::BTreeMap;
use verification_common::verifier_alliance::{
    CompilationArtifacts, CreationCodeArtifacts, RuntimeCodeArtifacts,
};
use verifier_alliance_database::{
    internal, CompiledContract, CompiledContractCompiler, CompiledContractLanguage,
};
use verifier_alliance_migration::Migrator;

#[tokio::test]
async fn insert_compiled_contract_works() {
    let db_guard = database!(Migrator);

    let compiled_contract = CompiledContract {
        compiler: CompiledContractCompiler::Solc,
        version: "".to_string(),
        language: CompiledContractLanguage::Solidity,
        name: "Counter".to_string(),
        fully_qualified_name: "src/Counter.sol:Counter".to_string(),
        sources: BTreeMap::from([(
            "src/Counter.sol".into(),
            "// SPDX-License-Identifier: UNLICENSED\npragma solidity ^0.8.13;\n\ncontract Counter {\n    uint256 public number;\n\n    function setNumber(uint256 newNumber) public {\n        number = newNumber;\n    }\n\n    function increment() public {\n        number++;\n    }\n}\n".into(),
        )]),
        compiler_settings: from_json!({"evmVersion":"paris","libraries":{},"optimizer":{"enabled":true,"runs":200},"outputSelection":{"*":{"*":["*"]}},"remappings":[],"viaIR":false}),

        compilation_artifacts: CompilationArtifacts {
            abi: Some(from_json!({"abi": "value"})),
            devdoc: Some(from_json!({"devdoc": "value"})),
            userdoc: Some(from_json!({"userdoc": "value"})),
            storage_layout: Some(from_json!({"storage": "value"})),
            sources: Some(from_json!({"src/Counter.sol": { "id": 0 }})),
        },
        creation_code: vec![0x1, 0x2],
        creation_code_artifacts: CreationCodeArtifacts {
            source_map: Some(from_json!("source_map")),
            link_references: Some(from_json!({"lib.sol": {"lib": [{"length": 20, "start": 1}]}})),
            cbor_auxdata: Some(from_json!({"1": {"value": "0x1234", "offset": 1}})),
        },
        runtime_code: vec![0x3, 0x4],
        runtime_code_artifacts: RuntimeCodeArtifacts {
            cbor_auxdata: Some(from_json!({"1": {"value": "0x1234", "offset": 1}})),
            immutable_references: Some(from_json!({"1": [{"length": 32, "start": 1}]})),
            link_references: Some(from_json!({"lib.sol": {"lib": [{"length": 20, "start": 1}]}})),
            source_map: Some(from_json!("source_map")),
        },
    };

    let _inserted_model =
        internal::insert_compiled_contract(db_guard.client().as_ref(), compiled_contract)
            .await
            .expect("error while inserting");
}

#[tokio::test]
async fn insert_compiled_contract_with_empty_artifact_values() {
    let db_guard = database!(Migrator);

    let compiled_contract = CompiledContract {
        compiler: CompiledContractCompiler::Solc,
        version: "".to_string(),
        language: CompiledContractLanguage::Solidity,
        name: "Counter".to_string(),
        fully_qualified_name: "src/Counter.sol:Counter".to_string(),
        sources: BTreeMap::from([(
            "src/Counter.sol".into(),
            "// SPDX-License-Identifier: UNLICENSED\npragma solidity ^0.8.13;\n\ncontract Counter {\n    uint256 public number;\n\n    function setNumber(uint256 newNumber) public {\n        number = newNumber;\n    }\n\n    function increment() public {\n        number++;\n    }\n}\n".into(),
        )]),
        compiler_settings: from_json!({"evmVersion":"paris","libraries":{},"optimizer":{"enabled":true,"runs":200},"outputSelection":{"*":{"*":["*"]}},"remappings":[],"viaIR":false}),
        compilation_artifacts: CompilationArtifacts {
            abi: None,
            devdoc: None,
            userdoc: None,
            storage_layout: None,
            sources: None,
        },
        creation_code: vec![0x1, 0x2],
        creation_code_artifacts: CreationCodeArtifacts {
            source_map: None,
            link_references: None,
            cbor_auxdata: None,
        },
        runtime_code: vec![0x3, 0x4],
        runtime_code_artifacts: RuntimeCodeArtifacts {
            cbor_auxdata: None,
            immutable_references: None,
            link_references: None,
            source_map: None,
        },
    };

    let _inserted_model =
        internal::insert_compiled_contract(db_guard.client().as_ref(), compiled_contract)
            .await
            .expect("error while inserting");
}
