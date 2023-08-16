#![allow(unused_imports, dead_code)]

mod database_helpers;

pub mod smart_contract_verifer_mock;
pub mod test_input_data;

use async_trait::async_trait;
use database_helpers::TestDbGuard;
use eth_bytecode_db::verification::SourceType;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use eth_bytecode_db_server::Settings;
use reqwest::Url;
use smart_contract_verifer_mock::SmartContractVerifierServer;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use std::{net::SocketAddr, str::FromStr};
use tonic::transport::Uri;

const DB_PREFIX: &str = "server";

const DB_SEARCH_ROUTE: &str = "/api/v2/bytecodes/sources:search";

#[async_trait]
pub trait VerifierService<Response> {
    fn add_into_service(&mut self, response: Response);

    fn build_server(self) -> SmartContractVerifierServer;
}

pub async fn init_db(test_suite_name: &str, test_name: &str) -> TestDbGuard {
    #[allow(unused_variables)]
    let db_url: Option<String> = None;
    // Uncomment if providing url explicitly is more convenient
    // let db_url = Some("postgres://postgres:admin@localhost:9432/".into());
    let db_name = format!("{DB_PREFIX}_{test_suite_name}_{test_name}");
    TestDbGuard::new(db_name.as_str(), db_url).await
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

pub async fn init_eth_bytecode_db_server(db_url: &str, verifier_addr: SocketAddr) -> Url {
    let verifier_uri = Uri::from_str(&format!("http://{verifier_addr}")).unwrap();

    let settings = {
        let mut settings = Settings::default(db_url.into(), verifier_uri);

        // Take a random port in range [10000..65535]
        let port = (rand::random::<u16>() % 55535) + 10000;
        settings.server.http.addr = SocketAddr::from_str(&format!("127.0.0.1:{port}")).unwrap();
        settings.server.grpc.enabled = false;
        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;
        settings.database.sourcify.enabled = true;
        settings
    };

    let _server_handle = {
        let settings = settings.clone();
        tokio::spawn(async move { eth_bytecode_db_server::run(settings).await })
    };

    let client = reqwest::Client::new();
    let base = Url::parse(&format!("http://{}", settings.server.http.addr)).unwrap();

    let health_endpoint = base.join("health").unwrap();
    // Wait for the server to start
    loop {
        if let Ok(_response) = client
            .get(health_endpoint.clone())
            .query(&[("service", "blockscout.ethBytecodeDb.v2.SourcifyVerifier")])
            .send()
            .await
        {
            break;
        }
    }

    base
}

pub async fn send_annotated_request<
    Request: serde::Serialize,
    Response: for<'a> serde::Deserialize<'a>,
>(
    eth_bytecode_db_base: &Url,
    route: &str,
    request: &Request,
    annotation: Option<&str>,
) -> Response {
    let annotation = annotation.map(|v| format!("({v}) ")).unwrap_or_default();

    let response = reqwest::Client::new()
        .post(eth_bytecode_db_base.join(route).unwrap())
        .json(&request)
        .send()
        .await
        .unwrap_or_else(|_| panic!("{annotation}Failed to send request"));

    // Assert that status code is success
    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await.expect("Read body as text");
        panic!("({annotation})Invalid status code (success expected). Status: {status}. Message: {message}")
    }

    response
        .json()
        .await
        .unwrap_or_else(|_| panic!("({annotation})Response deserialization failed"))
}
pub async fn send_request<Request: serde::Serialize, Response: for<'a> serde::Deserialize<'a>>(
    eth_bytecode_db_base: &Url,
    route: &str,
    request: &Request,
) -> Response {
    send_annotated_request(eth_bytecode_db_base, route, request, None).await
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
            send_request(&eth_bytecode_db_base, route, &request).await;

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
            send_request(&eth_bytecode_db_base, route, &verification_request).await;

        let creation_input_search_response: eth_bytecode_db_v2::SearchSourcesResponse = {
            let request = {
                eth_bytecode_db_v2::SearchSourcesRequest {
                    bytecode: creation_input,
                    bytecode_type: eth_bytecode_db_v2::BytecodeType::CreationInput.into(),
                }
            };
            send_annotated_request(
                &eth_bytecode_db_base,
                DB_SEARCH_ROUTE,
                &request,
                Some("Creation input search"),
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
            send_annotated_request(
                &eth_bytecode_db_base,
                DB_SEARCH_ROUTE,
                &request,
                Some("Deployed bytecode search"),
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
            send_request(&eth_bytecode_db_base, route, &verification_request).await;
        let verification_response_2: eth_bytecode_db_v2::VerifyResponse =
            send_request(&eth_bytecode_db_base, route, &verification_request).await;

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

            send_annotated_request(
                &eth_bytecode_db_base,
                DB_SEARCH_ROUTE,
                &request,
                Some("Creation input search"),
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
        let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;
        let _verification_response: eth_bytecode_db_v2::VerifyResponse =
            send_request(&eth_bytecode_db_base, route, &verification_request).await;

        let verifier_addr = init_verifier_server(
            Service::default(),
            partial_match_test_data.verifier_response,
        )
        .await;
        let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;
        let _verification_response: eth_bytecode_db_v2::VerifyResponse =
            send_request(&eth_bytecode_db_base, route, &verification_request).await;

        let creation_input_search_response: eth_bytecode_db_v2::SearchSourcesResponse = {
            let request = {
                eth_bytecode_db_v2::SearchSourcesRequest {
                    bytecode: full_match_creation_input,
                    bytecode_type: eth_bytecode_db_v2::BytecodeType::CreationInput.into(),
                }
            };

            send_annotated_request(
                &eth_bytecode_db_base,
                DB_SEARCH_ROUTE,
                &request,
                Some("Creation input search"),
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
            let _: eth_bytecode_db_v2::VerifyResponse =
                send_annotated_request(&eth_bytecode_db_base, route, &request, Some(&annotation))
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
}
