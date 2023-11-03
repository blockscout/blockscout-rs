#![allow(unused_imports, dead_code)]

pub mod smart_contract_verifer_mock;
pub mod test_input_data;

pub mod verifier_alliance_types;

use async_trait::async_trait;
use blockscout_service_launcher::{test_database::TestDbGuard, test_server};
use eth_bytecode_db::verification::SourceType;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use eth_bytecode_db_server::Settings;
use migration::MigratorTrait;
use reqwest::Url;
use smart_contract_verifer_mock::SmartContractVerifierServer;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use std::{collections::HashMap, net::SocketAddr, str::FromStr};
use tonic::transport::Uri;

const DB_PREFIX: &str = "server";

const DB_SEARCH_ROUTE: &str = "/api/v2/bytecodes/sources:search";

#[async_trait]
pub trait VerifierService<Response> {
    fn add_into_service(&mut self, response: Response);

    fn build_server(self) -> SmartContractVerifierServer;
}

pub async fn init_db(test_suite_name: &str, test_name: &str) -> TestDbGuard {
    init_db_raw::<migration::Migrator>(test_suite_name, test_name).await
}

pub async fn init_db_raw<Migrator: MigratorTrait>(
    test_suite_name: &str,
    test_name: &str,
) -> TestDbGuard {
    let db_name = format!("{DB_PREFIX}_{test_suite_name}_{test_name}");
    TestDbGuard::new::<Migrator>(db_name.as_str()).await
}

pub async fn init_verifier_server<Service, Response>(
    mut service: Service,
    verifier_response: Response,
) -> SocketAddr
where
    Service: VerifierService<Response>,
{
    service.add_into_service(verifier_response);
    service.build_server().start().await
}

pub async fn init_eth_bytecode_db_server(db_url: String, verifier_addr: SocketAddr) -> Url {
    let no_op_settings_setup = |settings: Settings| settings;

    init_eth_bytecode_db_server_with_settings_setup(db_url, verifier_addr, no_op_settings_setup)
        .await
}

pub async fn init_eth_bytecode_db_server_with_settings_setup<F>(
    db_url: String,
    verifier_addr: SocketAddr,
    settings_setup: F,
) -> Url
where
    F: Fn(Settings) -> Settings,
{
    let verifier_uri = Uri::from_str(&format!("http://{verifier_addr}")).unwrap();
    let (settings, base) = {
        let mut settings = Settings::default(db_url, verifier_uri);
        let (server_settings, base) = test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;

        (settings_setup(settings), base)
    };

    test_server::init_server(|| eth_bytecode_db_server::run(settings), &base).await;

    base
}

pub mod test_cases {
    use super::*;
    use amplify::set;
    use eth_bytecode_db::verification::MatchType;
    use pretty_assertions::assert_eq;
    use serde::Serialize;

    pub async fn test_returns_valid_source<Service, Request>(
        test_suite_name: &str,
        service: Service,
        route: &str,
        request: Request,
        source_type: SourceType,
    ) where
        Service: VerifierService<smart_contract_verifier_v2::VerifyResponse>,
        Request: Serialize,
    {
        let db = init_db(test_suite_name, "test_returns_valid_source").await;

        let test_data = test_input_data::basic(source_type, MatchType::Partial);

        let db_url = db.db_url();
        let verifier_addr = init_verifier_server(service, test_data.verifier_response).await;

        let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

        let verification_response: eth_bytecode_db_v2::VerifyResponse =
            test_server::send_post_request(&eth_bytecode_db_base, route, &request).await;

        assert_eq!(
            test_data.eth_bytecode_db_response, verification_response,
            "Invalid verification response"
        );
    }

    pub async fn test_verify_then_search<Service, Request>(
        test_suite_name: &str,
        service: Service,
        route: &str,
        verification_request: Request,
        source_type: SourceType,
    ) where
        Service: VerifierService<smart_contract_verifier_v2::VerifyResponse>,
        Request: Serialize,
    {
        let db = init_db(test_suite_name, "test_verify_then_search").await;

        let test_data = test_input_data::basic(source_type, MatchType::Full);
        let creation_input = test_data.creation_input().unwrap();
        let deployed_bytecode = test_data.deployed_bytecode().unwrap();

        let db_url = db.db_url();
        let verifier_addr = init_verifier_server(service, test_data.verifier_response).await;

        let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

        let verification_response: eth_bytecode_db_v2::VerifyResponse =
            test_server::send_post_request(&eth_bytecode_db_base, route, &verification_request)
                .await;

        let creation_input_search_response: eth_bytecode_db_v2::SearchSourcesResponse = {
            let request = {
                eth_bytecode_db_v2::SearchSourcesRequest {
                    bytecode: creation_input,
                    bytecode_type: eth_bytecode_db_v2::BytecodeType::CreationInput.into(),
                }
            };
            test_server::send_annotated_post_request(
                &eth_bytecode_db_base,
                DB_SEARCH_ROUTE,
                &request,
                "Creation input search",
            )
            .await
        };

        let deployed_bytecode_search_response: eth_bytecode_db_v2::SearchSourcesResponse = {
            let request = {
                eth_bytecode_db_v2::SearchSourcesRequest {
                    bytecode: deployed_bytecode,
                    bytecode_type: eth_bytecode_db_v2::BytecodeType::DeployedBytecode.into(),
                }
            };
            test_server::send_annotated_post_request(
                &eth_bytecode_db_base,
                DB_SEARCH_ROUTE,
                &request,
                "Deployed bytecode search",
            )
            .await
        };

        assert_eq!(
            creation_input_search_response, deployed_bytecode_search_response,
            "Results for creation input and deployed bytecode searches differ"
        );
        assert_eq!(
            1,
            creation_input_search_response.sources.len(),
            "Invalid number of sources returned"
        );
        assert_eq!(
            verification_response.source.unwrap(),
            creation_input_search_response.sources[0],
            "Sources returned on verification and search differ"
        );
    }

    pub async fn test_verify_same_source_twice<Service, Request>(
        test_suite_name: &str,
        service: Service,
        route: &str,
        verification_request: Request,
        source_type: SourceType,
    ) where
        Service: VerifierService<smart_contract_verifier_v2::VerifyResponse>,
        Request: Serialize,
    {
        let db = init_db(test_suite_name, "test_verify_same_source_twice").await;

        let test_data = test_input_data::basic(source_type, MatchType::Full);
        let creation_input = test_data.creation_input().unwrap();

        let db_url = db.db_url();
        let verifier_addr = init_verifier_server(service, test_data.verifier_response).await;

        let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

        let verification_response: eth_bytecode_db_v2::VerifyResponse =
            test_server::send_post_request(&eth_bytecode_db_base, route, &verification_request)
                .await;
        let verification_response_2: eth_bytecode_db_v2::VerifyResponse =
            test_server::send_post_request(&eth_bytecode_db_base, route, &verification_request)
                .await;

        assert_eq!(
            verification_response, verification_response_2,
            "Verification responses are different"
        );

        let creation_input_search_response: eth_bytecode_db_v2::SearchSourcesResponse = {
            let request = {
                eth_bytecode_db_v2::SearchSourcesRequest {
                    bytecode: creation_input,
                    bytecode_type: eth_bytecode_db_v2::BytecodeType::CreationInput.into(),
                }
            };

            test_server::send_annotated_post_request(
                &eth_bytecode_db_base,
                DB_SEARCH_ROUTE,
                &request,
                "Creation input search",
            )
            .await
        };

        assert_eq!(
            1,
            creation_input_search_response.sources.len(),
            "Invalid number of sources returned"
        );
        assert_eq!(
            verification_response.source.unwrap(),
            creation_input_search_response.sources[0],
            "Sources returned on verification and search differ"
        );
    }

    pub async fn test_search_returns_full_matches_only_if_any<Service, Request>(
        test_suite_name: &str,
        route: &str,
        verification_request: Request,
        source_type: SourceType,
    ) where
        Service: Default + VerifierService<smart_contract_verifier_v2::VerifyResponse>,
        Request: Serialize,
    {
        let db = init_db(
            test_suite_name,
            "test_search_returns_full_matches_only_if_any",
        )
        .await;

        let (full_match_test_data, partial_match_test_data) = {
            let partial_match_test_data = test_input_data::basic(source_type, MatchType::Partial);
            let mut full_match_test_data = test_input_data::basic(source_type, MatchType::Full);
            full_match_test_data.set_creation_input_metadata_hash(
                "12345678901234567890123456789012345678901234567890123456789012345678",
            );
            full_match_test_data.add_source_file(
                "additional_file".to_string(),
                "additional_content".to_string(),
            );
            (full_match_test_data, partial_match_test_data)
        };
        let full_match_creation_input = full_match_test_data.creation_input().unwrap();

        let db_url = db.db_url();

        let verifier_addr =
            init_verifier_server(Service::default(), full_match_test_data.verifier_response).await;
        let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url.clone(), verifier_addr).await;
        let _verification_response: eth_bytecode_db_v2::VerifyResponse =
            test_server::send_post_request(&eth_bytecode_db_base, route, &verification_request)
                .await;

        let verifier_addr = init_verifier_server(
            Service::default(),
            partial_match_test_data.verifier_response,
        )
        .await;
        let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;
        let _verification_response: eth_bytecode_db_v2::VerifyResponse =
            test_server::send_post_request(&eth_bytecode_db_base, route, &verification_request)
                .await;

        let creation_input_search_response: eth_bytecode_db_v2::SearchSourcesResponse = {
            let request = {
                eth_bytecode_db_v2::SearchSourcesRequest {
                    bytecode: full_match_creation_input,
                    bytecode_type: eth_bytecode_db_v2::BytecodeType::CreationInput.into(),
                }
            };

            test_server::send_annotated_post_request(
                &eth_bytecode_db_base,
                DB_SEARCH_ROUTE,
                &request,
                "Creation input search",
            )
            .await
        };

        assert_eq!(
            1,
            creation_input_search_response.sources.len(),
            "Invalid number of sources returned"
        );
        assert_eq!(
            full_match_test_data
                .eth_bytecode_db_response
                .source
                .unwrap(),
            creation_input_search_response.sources[0],
            "Sources returned on verification and search differ"
        );
    }

    pub async fn test_accepts_partial_verification_metadata_in_input<Service, Request>(
        test_suite_name: &str,
        route: &str,
        verification_request: Request,
        source_type: SourceType,
    ) where
        Service: Default + VerifierService<smart_contract_verifier_v2::VerifyResponse>,
        Request: Serialize + Clone,
    {
        let db = init_db(
            test_suite_name,
            "test_accepts_partial_verification_metadata_in_input",
        )
        .await;

        let test_data = test_input_data::basic(source_type, MatchType::Partial);

        let db_url = db.db_url();
        let verifier_addr =
            init_verifier_server(Service::default(), test_data.verifier_response).await;

        let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

        let validate = |metadata: serde_json::Value| async {
            let metadata_to_print = metadata.clone();
            let mut request = serde_json::to_value(verification_request.clone()).unwrap();
            if let Some(value) = request.as_object_mut() {
                value.insert("metadata".to_string(), metadata)
            } else {
                panic!("Request value is not an object")
            };

            let annotation = format!("Metadata: {metadata_to_print}");
            let _: eth_bytecode_db_v2::VerifyResponse = test_server::send_annotated_post_request(
                &eth_bytecode_db_base,
                route,
                &request,
                &annotation,
            )
            .await;
        };

        // `chain_id` is provided, but `contract_address` is missed from the verification metadata
        let metadata = serde_json::json!({ "chainId": "5" });
        validate(metadata).await;

        // `chain_id` is provided, but `contract_address` is missed from the verification metadata
        let metadata =
            serde_json::json!({ "contractAddress": "0x0123456789012345678901234567890123456789" });
        validate(metadata).await;
    }

    pub async fn test_update_source_then_search<Service, Request>(
        test_suite_name: &str,
        route: &str,
        verification_request: Request,
        source_type: SourceType,
    ) where
        Service: VerifierService<smart_contract_verifier_v2::VerifyResponse> + Default,
        Request: Serialize,
    {
        let db = init_db(test_suite_name, "test_update_source_then_search").await;

        {
            let test_data = test_input_data::basic(source_type, MatchType::Full);

            let verifier_addr =
                init_verifier_server(Service::default(), test_data.verifier_response).await;
            let eth_bytecode_db_base =
                init_eth_bytecode_db_server(db.db_url(), verifier_addr).await;

            let _verification_response: eth_bytecode_db_v2::VerifyResponse =
                test_server::send_post_request(&eth_bytecode_db_base, route, &verification_request)
                    .await;
        }

        let updated_test_data = {
            let test_input_data::TestInputData {
                verifier_response,
                mut eth_bytecode_db_response,
            } = test_input_data::basic(source_type, MatchType::Full);
            if let Some(source) = eth_bytecode_db_response.source.as_mut() {
                let mut compilation_artifacts: serde_json::Value =
                    serde_json::from_str(source.compilation_artifacts()).unwrap();
                compilation_artifacts
                    .as_object_mut()
                    .unwrap()
                    .insert("additionalValue".to_string(), serde_json::Value::default());
                source.compilation_artifacts = Some(compilation_artifacts.to_string());
            }

            test_input_data::TestInputData::from_source_and_extra_data(
                eth_bytecode_db_response.source.unwrap(),
                verifier_response.extra_data.unwrap(),
            )
        };

        let creation_input = updated_test_data.creation_input().unwrap();

        let verifier_addr =
            init_verifier_server(Service::default(), updated_test_data.verifier_response).await;
        let eth_bytecode_db_base = init_eth_bytecode_db_server(db.db_url(), verifier_addr).await;

        let verification_response: eth_bytecode_db_v2::VerifyResponse =
            test_server::send_post_request(&eth_bytecode_db_base, route, &verification_request)
                .await;

        assert_eq!(
            verification_response, updated_test_data.eth_bytecode_db_response,
            "Invalid verification response"
        );

        let creation_input_search_response: eth_bytecode_db_v2::SearchSourcesResponse = {
            let request = {
                eth_bytecode_db_v2::SearchSourcesRequest {
                    bytecode: creation_input,
                    bytecode_type: eth_bytecode_db_v2::BytecodeType::CreationInput.into(),
                }
            };

            test_server::send_annotated_post_request(
                &eth_bytecode_db_base,
                DB_SEARCH_ROUTE,
                &request,
                "Creation input search",
            )
            .await
        };

        assert_eq!(
            1,
            creation_input_search_response.sources.len(),
            "Invalid number of sources returned"
        );
        assert_eq!(
            verification_response.source.unwrap(),
            creation_input_search_response.sources[0],
            "Sources returned on verification and search differ"
        );
    }
}
