use crate::database;
use blockscout_display_bytes::decode_hex;
use sea_orm::{prelude::Uuid, DatabaseConnection};
use serde_json::json;
use std::collections::BTreeMap;
use verification_common::verifier_alliance::{
    CompilationArtifacts, CreationCodeArtifacts, Match, MatchTransformation, MatchType,
    MatchValues, RuntimeCodeArtifacts, SourceId,
};
use verifier_alliance_database::{
    internal, CompiledContract, CompiledContractCompiler, CompiledContractLanguage,
    ContractDeployment, VerifiedContract, VerifiedContractMatches,
};

const MOD_NAME: &str = "verified_contracts";

#[tokio::test]
async fn insert_verified_contract_with_complete_matches_work() {
    const TEST_NAME: &str = "insert_verified_contract_with_complete_matches_work";

    let db_guard = database!();

    let contract_deployment_id = insert_contract_deployment(db_guard.client().as_ref()).await;
    let compiled_contract = compiled_contract();
    let verified_contract = VerifiedContract {
        contract_deployment_id,
        compiled_contract,
        matches: VerifiedContractMatches::Complete {
            runtime_match: Match {
                r#type: MatchType::Full,
                transformations: vec![],
                values: Default::default(),
            },
            creation_match: Match {
                r#type: MatchType::Full,
                transformations: vec![],
                values: Default::default(),
            },
        },
    };

    let _inserted_model =
        internal::insert_verified_contract(db_guard.client().as_ref(), verified_contract)
            .await
            .expect("error while inserting");
}

#[tokio::test]
async fn insert_verified_contract_with_runtime_only_matches_work() {
    const TEST_NAME: &str = "insert_verified_contract_with_runtime_only_matches_work";

    let db_guard = database!();

    let contract_deployment_id = insert_contract_deployment(db_guard.client().as_ref()).await;
    let compiled_contract = compiled_contract();
    let verified_contract = VerifiedContract {
        contract_deployment_id,
        compiled_contract,
        matches: VerifiedContractMatches::OnlyRuntime {
            runtime_match: Match {
                r#type: MatchType::Full,
                transformations: vec![],
                values: Default::default(),
            },
        },
    };

    let _inserted_model =
        internal::insert_verified_contract(db_guard.client().as_ref(), verified_contract)
            .await
            .expect("error while inserting");
}

#[tokio::test]
async fn insert_verified_contract_with_creation_only_matches_work() {
    const TEST_NAME: &str = "insert_verified_contract_with_creation_only_matches_work";

    let db_guard = database!();

    let contract_deployment_id = insert_contract_deployment(db_guard.client().as_ref()).await;
    let compiled_contract = compiled_contract();
    let verified_contract = VerifiedContract {
        contract_deployment_id,
        compiled_contract,
        matches: VerifiedContractMatches::OnlyCreation {
            creation_match: Match {
                r#type: MatchType::Full,
                transformations: vec![],
                values: Default::default(),
            },
        },
    };

    let _inserted_model =
        internal::insert_verified_contract(db_guard.client().as_ref(), verified_contract)
            .await
            .expect("error while inserting");
}

#[tokio::test]
async fn insert_verified_contract_with_filled_matches() {
    const TEST_NAME: &str = "insert_verified_contract_with_filled_matches";

    let db_guard = database!();

    let contract_deployment_id = insert_contract_deployment(db_guard.client().as_ref()).await;
    let compiled_contract = compiled_contract();

    let (runtime_match_values, runtime_match_transformations) = {
        let mut match_values = MatchValues::default();
        let mut match_transformations = vec![];

        match_values.add_immutable(
            "immutable",
            decode_hex("0x0000000000000000000000000000000000000000000000000000000000000032")
                .unwrap()
                .into(),
        );
        match_transformations.push(MatchTransformation::immutable(1, "immutable"));
        match_values.add_library(
            "library",
            decode_hex("0x0000000000000000000000000000000000000020")
                .unwrap()
                .into(),
        );
        match_transformations.push(MatchTransformation::library(1, "library"));
        match_values.add_cbor_auxdata(
            "cborAuxdata",
            decode_hex("0x1000000000000000000000000000000000000000000000000000000000000032")
                .unwrap()
                .into(),
        );
        match_transformations.push(MatchTransformation::auxdata(1, "cborAuxdata"));

        (match_values, match_transformations)
    };

    let (creation_match_values, creation_match_transformations) = {
        let mut match_values = MatchValues::default();
        let mut match_transformations = vec![];

        match_values.add_constructor_arguments(decode_hex("0x01020304").unwrap().into());
        match_transformations.push(MatchTransformation::constructor(1));
        match_values.add_library(
            "library",
            decode_hex("0x0000000000000000000000000000000000000020")
                .unwrap()
                .into(),
        );
        match_transformations.push(MatchTransformation::library(1, "library"));
        match_values.add_cbor_auxdata(
            "cborAuxdata",
            decode_hex("0x1000000000000000000000000000000000000000000000000000000000000032")
                .unwrap()
                .into(),
        );
        match_transformations.push(MatchTransformation::auxdata(1, "cborAuxdata"));

        (match_values, match_transformations)
    };

    let verified_contract = VerifiedContract {
        contract_deployment_id,
        compiled_contract,
        matches: VerifiedContractMatches::Complete {
            runtime_match: Match {
                r#type: MatchType::Partial,
                transformations: runtime_match_transformations,
                values: runtime_match_values,
            },
            creation_match: Match {
                r#type: MatchType::Partial,
                transformations: creation_match_transformations,
                values: creation_match_values,
            },
        },
    };

    let _inserted_model =
        internal::insert_verified_contract(db_guard.client().as_ref(), verified_contract)
            .await
            .expect("error while inserting");
}

async fn insert_contract_deployment(database_connection: &DatabaseConnection) -> Uuid {
    let contract_deployment = ContractDeployment::Regular {
        chain_id: 10,
        address: decode_hex("0x8FbB39A5a79aeCE03c8f13ccEE0b96C128ec1a67").unwrap(),
        transaction_hash: decode_hex(
            "0xf4042e19c445551d1059ad3856f83383c48699367cfb3e0edeccd26002dd2292",
        )
        .unwrap(),
        block_number: 127387809,
        transaction_index: 16,
        deployer: decode_hex("0x1F98431c8aD98523631AE4a59f267346ea31F984").unwrap(),
        creation_code: vec![0x1, 0x2],
        runtime_code: vec![0x3, 0x4],
    };

    internal::insert_contract_deployment(database_connection, contract_deployment)
        .await
        .expect("error while inserting contract deployment")
        .id
}

fn compiled_contract() -> CompiledContract {
    CompiledContract {
        compiler: CompiledContractCompiler::Solc,
        version: "".to_string(),
        language: CompiledContractLanguage::Solidity,
        name: "Counter".to_string(),
        fully_qualified_name: "src/Counter.sol:Counter".to_string(),
        sources: BTreeMap::from([(
            "src/Counter.sol".into(),
            "// SPDX-License-Identifier: UNLICENSED\npragma solidity ^0.8.13;\n\ncontract Counter {\n    uint256 public number;\n\n    function setNumber(uint256 newNumber) public {\n        number = newNumber;\n    }\n\n    function increment() public {\n        number++;\n    }\n}\n".into(),
        )]),
        compiler_settings: json!({"evmVersion":"paris","libraries":{},"optimizer":{"enabled":true,"runs":200},"outputSelection":{"*":{"*":["*"]}},"remappings":[],"viaIR":false}),
        compilation_artifacts: CompilationArtifacts {
            abi: Some(json!({"abi": "value"})),
            devdoc: Some(json!({"devdoc": "value"})),
            userdoc: Some(json!({"userdoc": "value"})),
            storage_layout: Some(json!({"storage": "value"})),
            sources: Some(BTreeMap::from([("src/Counter.sol".into(), SourceId {id: 0})]))
        },
        creation_code: vec![0x1, 0x2],
        creation_code_artifacts: CreationCodeArtifacts {
            source_map: Some(json!("source_map")),
            link_references: Some(json!({"linkReferences": "value"})),
            cbor_auxdata: Some(json!({"cborAuxdata": "value"})),
        },
        runtime_code: vec![0x3, 0x4],
        runtime_code_artifacts: RuntimeCodeArtifacts {
            cbor_auxdata: Some(json!({"cborAuxdata": "value"})),
            immutable_references: Some(json!({"immutableReferences": "value"})),
            link_references: Some(json!({"linkReferences": "value"})),
            source_map: Some(json!("source_map")),
        },
    }
}
