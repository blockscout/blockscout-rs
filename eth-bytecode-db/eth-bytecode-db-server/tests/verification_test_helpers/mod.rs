#![allow(unused_imports, dead_code)]

mod database_helpers;
pub mod smart_contract_verifer_mock;
mod test_input_data;

use amplify::set;
use async_trait::async_trait;
use database_helpers::TestDbGuard;
use eth_bytecode_db::verification::{MatchType, SourceType};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use eth_bytecode_db_server::Settings;
use pretty_assertions::assert_eq;
use reqwest::Url;
use serde::Serialize;
use smart_contract_verifer_mock::SmartContractVerifierServer;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use std::{net::SocketAddr, str::FromStr};
use tonic::transport::Uri;

const DB_PREFIX: &str = "server";

const DB_SEARCH_ROUTE: &str = "/api/v2/bytecodes/sources:search";

#[async_trait]
pub trait VerifierService {
    fn add_into_service(&mut self, response: smart_contract_verifier_v2::VerifyResponse);

    fn build_server(self) -> SmartContractVerifierServer;

    fn source_type(&self) -> SourceType;
}

async fn init_db(test_suite_name: &str, test_name: &str) -> TestDbGuard {
    #[allow(unused_variables)]
    let db_url: Option<String> = None;
    // Uncomment if providing url explicitly is more convenient
    let db_url = Some("postgres://postgres:admin@localhost:9432/".into());
    let db_name = format!("{}_{}_{}", DB_PREFIX, test_suite_name, test_name);
    TestDbGuard::new(db_name.as_str(), db_url).await
}

async fn init_verifier_server<Service>(
    mut service: Service,
    verifier_response: smart_contract_verifier_v2::VerifyResponse,
) -> SocketAddr
where
    Service: VerifierService,
{
    service.add_into_service(verifier_response);
    service.build_server().start().await
}

async fn init_eth_bytecode_db_server(db_url: &str, verifier_addr: SocketAddr) -> Url {
    let verifier_uri = Uri::from_str(&format!("http://{}", verifier_addr)).unwrap();

    let settings = {
        let mut settings = Settings::default(db_url.into(), verifier_uri);

        // Take a random port in range [10000..65535]
        let port = (rand::random::<u16>() % 55535) + 10000;
        settings.server.http.addr = SocketAddr::from_str(&format!("127.0.0.1:{port}")).unwrap();
        settings.server.grpc.enabled = false;
        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;
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

pub async fn test_returns_valid_source<Service, Request>(
    test_suite_name: &str,
    service: Service,
    route: &str,
    request: Request,
) where
    Service: VerifierService,
    Request: Serialize,
{
    let db = init_db(test_suite_name, "test_returns_valid_source").await;

    let test_data = test_input_data::input_data_1(service.source_type(), MatchType::Partial);

    let db_url = db.db_url();
    let verifier_addr = init_verifier_server(service, test_data.verifier_response).await;

    let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

    let response = reqwest::Client::new()
        .post(eth_bytecode_db_base.join(route).unwrap())
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Assert that status code is success
    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await.expect("Read body as text");
        panic!(
            "Invalid status code (success expected). Status: {}. Message: {}",
            status, message
        )
    }

    let verification_response: eth_bytecode_db_v2::VerifyResponse = response
        .json()
        .await
        .expect("Response deserialization failed");

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
) where
    Service: VerifierService,
    Request: Serialize,
{
    let db = init_db(test_suite_name, "test_verify_then_search").await;

    let test_data = test_input_data::input_data_1(service.source_type(), MatchType::Full);
    let creation_input = test_data.creation_input().unwrap();
    let deployed_bytecode = test_data.deployed_bytecode().unwrap();

    let db_url = db.db_url();
    let verifier_addr = init_verifier_server(service, test_data.verifier_response).await;

    let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

    let response = reqwest::Client::new()
        .post(eth_bytecode_db_base.join(route).unwrap())
        .json(&verification_request)
        .send()
        .await
        .expect("Failed to send verification request");

    let verification_response: eth_bytecode_db_v2::VerifyResponse = response
        .json()
        .await
        .expect("Verification response deserialization failed");

    let creation_input_search_response: eth_bytecode_db_v2::SearchSourcesResponse = {
        let request = {
            eth_bytecode_db_v2::SearchSourcesRequest {
                bytecode: creation_input,
                bytecode_type: eth_bytecode_db_v2::BytecodeType::CreationInput.into(),
            }
        };

        let response = reqwest::Client::new()
            .post(eth_bytecode_db_base.join(DB_SEARCH_ROUTE).unwrap())
            .json(&request)
            .send()
            .await
            .expect("Failed to send creation input search request");
        // Assert that status code is success
        if !response.status().is_success() {
            let status = response.status();
            let message = response.text().await.expect("Read body as text");
            panic!(
                "Creation input search: invalid status code (success expected). Status: {}. Message: {}",
                status, message
            )
        }
        response
            .json()
            .await
            .expect("Creation input search response deserialization failed")
    };

    let deployed_bytecode_search_response: eth_bytecode_db_v2::SearchSourcesResponse = {
        let request = {
            eth_bytecode_db_v2::SearchSourcesRequest {
                bytecode: deployed_bytecode,
                bytecode_type: eth_bytecode_db_v2::BytecodeType::DeployedBytecode.into(),
            }
        };

        let response = reqwest::Client::new()
            .post(eth_bytecode_db_base.join(DB_SEARCH_ROUTE).unwrap())
            .json(&request)
            .send()
            .await
            .expect("Failed to send deployed bytecode search request");
        // Assert that status code is success
        if !response.status().is_success() {
            let status = response.status();
            let message = response.text().await.expect("Read body as text");
            panic!(
                "Deployed bytecode search: invalid status code (success expected). Status: {}. Message: {}",
                status, message
            )
        }
        response
            .json()
            .await
            .expect("Deployed bytecode search response deserialization failed")
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
