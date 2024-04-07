use crate::{
    types,
    types::verifier_alliance::TestCase,
    verifier_service::{VerifierServiceRequest, VerifierServiceResponse},
    TestCaseRequest, TestCaseResponse, TestCaseRoute,
};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;

mod batch_import_solidity_standard_json {
    use super::*;
    use blockscout_service_launcher::{test_database, test_server};
    use eth_bytecode_db_server::Settings;
    use eth_bytecode_db_v2::verifier_alliance_batch_import_response;
    use std::{path::PathBuf, str::FromStr};
    use tonic::Request;

    struct BatchImportSolidityStandardJsonRoute;

    impl TestCaseRoute for BatchImportSolidityStandardJsonRoute {
        const ROUTE: &'static str = "/api/v2/alliance/solidity/standard-json:batch-import";
    }

    impl TestCaseRequest<BatchImportSolidityStandardJsonRoute> for TestCase {
        type Request = eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityStandardJsonRequest;

        fn to_request(&self) -> Self::Request {
            let contract = eth_bytecode_db_v2::VerifierAllianceContract {
                chain_id: format!("{}", self.chain_id),
                contract_address: self.address.to_string(),
                transaction_hash: Some(self.transaction_hash.to_string()),
                block_number: Some(self.block_number),
                transaction_index: Some(self.transaction_index),
                deployer: Some(self.deployer.to_string()),
                creation_code: self.deployed_creation_code.as_ref().map(|v| v.to_string()),
                runtime_code: self.deployed_runtime_code.to_string(),
            };

            eth_bytecode_db_v2::VerifierAllianceBatchImportSolidityStandardJsonRequest {
                contracts: vec![contract],
                compiler_version: self.version.clone(),
                input: self.standard_input().to_string(),
            }
        }
    }

    impl TestCaseResponse<eth_bytecode_db_v2::VerifierAllianceBatchImportResponse> for TestCase {
        fn check(&self, actual_response: eth_bytecode_db_v2::VerifierAllianceBatchImportResponse) {
            match actual_response {
                eth_bytecode_db_v2::VerifierAllianceBatchImportResponse {
                    result:
                        Some(verifier_alliance_batch_import_response::Result::Results(
                            verifier_alliance_batch_import_response::ImportContractResults {
                                items,
                            },
                        )),
                } => {
                    assert_eq!(1, items.len(), "Invalid number of results returned");
                    let result = items[0].clone();
                    match result {
                        verifier_alliance_batch_import_response::ImportContractResult {
                            result: Some(
                                verifier_alliance_batch_import_response::import_contract_result::Result::Success(_success)
                            )
                        } => {

                        }
                        result => panic!("Invalid result: {:?}", result)
                    }
                }
                response => {
                    panic!("Invalid response: {:?}", response);
                }
            };
        }
    }

    impl VerifierServiceRequest<BatchImportSolidityStandardJsonRoute> for TestCase {
        type VerifierRequest = smart_contract_verifier_v2::BatchVerifySolidityStandardJsonRequest;

        fn with(&self, request: &Request<Self::VerifierRequest>) -> bool {
            let request = &request.get_ref();

            let input = self.standard_input().to_string();
            request.compiler_version == self.version && request.input == input
        }
    }

    impl VerifierServiceResponse<BatchImportSolidityStandardJsonRoute> for TestCase {
        type VerifierResponse = smart_contract_verifier_v2::BatchVerifyResponse;

        fn returning_const(&self) -> Self::VerifierResponse {
            let compiler = match self.compiler.to_lowercase().as_str() {
                "solc" => smart_contract_verifier_v2::contract_verification_success::compiler::Compiler::Solc,
                "vyper" => smart_contract_verifier_v2::contract_verification_success::compiler::Compiler::Vyper,
                _ => panic!("unexpected compiler")
            };
            let language = match self.language.to_lowercase().as_str() {
                "solidity" => smart_contract_verifier_v2::contract_verification_success::language::Language::Solidity,
                "yul" => smart_contract_verifier_v2::contract_verification_success::language::Language::Yul,
                "vyper" => smart_contract_verifier_v2::contract_verification_success::language::Language::Vyper,
                _ => panic!("unexpected language")
            };
            smart_contract_verifier_v2::BatchVerifyResponse {
                verification_result: Some(smart_contract_verifier_v2::batch_verify_response::VerificationResult::ContractVerificationResults(
                    smart_contract_verifier_v2::batch_verify_response::ContractVerificationResults {
                        items: vec![
                            smart_contract_verifier_v2::ContractVerificationResult {
                                verification_result: Some(smart_contract_verifier_v2::contract_verification_result::VerificationResult::Success(
                                    smart_contract_verifier_v2::ContractVerificationSuccess {
                                        creation_code: self.compiled_creation_code.to_string(),
                                        runtime_code: self.compiled_runtime_code.to_string(),
                                        compiler: compiler.into(),
                                        compiler_version: self.version.clone(),
                                        language: language.into(),
                                        file_name: self.file_name(),
                                        contract_name: self.contract_name(),
                                        sources: self.sources.clone(),
                                        compiler_settings: self.compiler_settings.to_string(),
                                        compilation_artifacts: self.compilation_artifacts.to_string(),
                                        creation_code_artifacts: self.creation_code_artifacts.to_string(),
                                        runtime_code_artifacts: self.runtime_code_artifacts.to_string(),
                                        creation_match_details: self.creation_match.then(|| {
                                            smart_contract_verifier_v2::contract_verification_success::MatchDetails {
                                                match_type: smart_contract_verifier_v2::contract_verification_success::MatchType::Undefined.into(),
                                                values: self.creation_values.as_ref().unwrap().to_string(),
                                                transformations: self.creation_transformations.as_ref().unwrap().to_string(),
                                            }
                                        }),
                                        runtime_match_details: self.runtime_match.then(|| {
                                            smart_contract_verifier_v2::contract_verification_success::MatchDetails {
                                                match_type: smart_contract_verifier_v2::contract_verification_success::MatchType::Undefined.into(),
                                                values: self.runtime_values.as_ref().unwrap().to_string(),
                                                transformations: self.runtime_transformations.as_ref().unwrap().to_string(),
                                            }
                                        }),
                                    }
                                )),
                            }
                        ],
                    }
                )),
            }
        }
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn success(
        #[files("tests/alliance_test_cases/full_match.json")] test_case_path: PathBuf,
    ) {
        let test_dir = test_case_path.parent().unwrap().to_str().unwrap();
        let test_case_name = test_case_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_suffix(".json")
            .unwrap();

        let test_case = types::from_file::<
            BatchImportSolidityStandardJsonRoute,
            TestCase,
            eth_bytecode_db_v2::VerifierAllianceBatchImportResponse,
        >(test_dir, test_case_name);

        let eth_bytecode_db_db = test_database::TestDbGuard::new::<migration::Migrator>(&format!(
            "verifier_alliance_{test_case_name}"
        ))
        .await;

        let alliance_db = test_database::TestDbGuard::new::<verifier_alliance_migration::Migrator>(
            &format!("alliance_verifier_alliance_{test_case_name}"),
        )
        .await;

        let verifier_addr = {
            let test_case_f = test_case.clone();
            let test_case_r = test_case.clone();
            let mut solidity_verifier =
                smart_contract_verifier_proto::http_client::mock::MockSolidityVerifierService::new(
                );
            solidity_verifier
                .expect_batch_verify_standard_json()
                .withf(move |request| test_case_f.with(request))
                .returning(move |_| Ok(tonic::Response::new(test_case_r.returning_const())));
            smart_contract_verifier_proto::http_client::mock::SmartContractVerifierServer::new()
                .solidity_service(solidity_verifier)
                .start()
                .await
        };

        let eth_bytecode_db_service = {
            let verifier_uri = url::Url::from_str(&format!("http://{verifier_addr}")).unwrap();
            let (settings, base) = {
                let mut settings = Settings::default(eth_bytecode_db_db.db_url(), verifier_uri);
                let (server_settings, base) = test_server::get_test_server_settings();
                settings.server = server_settings;
                settings.metrics.enabled = false;
                settings.tracing.enabled = false;
                settings.jaeger.enabled = false;

                settings.verifier_alliance_database.enabled = true;
                settings.verifier_alliance_database.url = alliance_db.db_url();

                (settings, base)
            };

            test_server::init_server(|| eth_bytecode_db_server::run(settings), &base).await;

            base
        };

        let response: eth_bytecode_db_v2::VerifierAllianceBatchImportResponse =
            test_server::send_post_request(
                &eth_bytecode_db_service,
                BatchImportSolidityStandardJsonRoute::ROUTE,
                &test_case.to_request(),
            )
            .await;

        test_case.check(response);
    }
}
