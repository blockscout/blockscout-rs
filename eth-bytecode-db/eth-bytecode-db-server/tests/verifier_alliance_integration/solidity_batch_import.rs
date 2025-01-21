use blockscout_service_launcher::test_database::TestDbGuard;
use eth_bytecode_db_proto::http_client;
use verifier_alliance_database_tests::{build_all_tests, TestCase};

async fn initialize(database: TestDbGuard, test_case: TestCase) {
    let setup_result = crate::setup(&test_case.test_case_name, database).await;

    let request = helpers::eth_bytecode_db_request(&test_case);

    let _verify_response =
        http_client::verifier_alliance_client::batch_import_solidity_standard_json(
            &setup_result.service_client,
            request,
        )
        .await
        .expect("sending verification request failed");
}

build_all_tests!(
    (
        constructor_arguments,
        full_match,
        immutables,
        libraries_linked_by_compiler,
        // libraries_manually_linked,
        // metadata_hash_absent,
        partial_match,
        partial_match_double_auxdata
    ),
    initialize
);

mod helpers {
    use blockscout_display_bytes::ToHex;
    use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
    use serde::Serialize;
    use std::collections::BTreeMap;
    use verifier_alliance_database_tests::TestCase;

    pub fn eth_bytecode_db_request(
        test_case: &TestCase,
    ) -> eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityStandardJsonRequest {
        eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityStandardJsonRequest {
            contracts: vec![contract(test_case)],
            compiler_version: test_case.version.clone(),
            input: standard_input(test_case).to_string(),
        }
    }

    fn standard_input(test_case: &TestCase) -> serde_json::Value {
        #[derive(Serialize)]
        struct Source {
            content: String,
        }

        #[derive(Serialize)]
        struct StandardJsonInput {
            language: String,
            sources: BTreeMap<String, Source>,
            settings: serde_json::Value,
        }

        let input = StandardJsonInput {
            language: "Solidity".to_string(),
            sources: test_case
                .sources
                .iter()
                .map(|(file_path, content)| {
                    (
                        file_path.to_string(),
                        Source {
                            content: content.to_string(),
                        },
                    )
                })
                .collect(),
            settings: test_case.compiler_settings.clone(),
        };

        serde_json::to_value(&input).unwrap()
    }

    fn contract(test_case: &TestCase) -> eth_bytecode_db_v2::VerifierAllianceContract {
        eth_bytecode_db_v2::VerifierAllianceContract {
            chain_id: format!("{}", test_case.chain_id),
            contract_address: test_case.address.to_hex(),
            transaction_hash: Some(test_case.transaction_hash.to_hex()),
            block_number: Some(test_case.block_number.try_into().unwrap()),
            transaction_index: Some(test_case.transaction_index.try_into().unwrap()),
            deployer: Some(test_case.deployer.to_hex()),
            creation_code: Some(test_case.deployed_creation_code.to_hex()),
            runtime_code: test_case.deployed_runtime_code.to_hex(),
        }
    }
}
