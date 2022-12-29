mod database_helpers;
pub mod smart_contract_verifer_mock;
mod test_input_data;

use async_trait::async_trait;
use database_helpers::TestDbGuard;
use eth_bytecode_db::verification::SourceType;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use eth_bytecode_db_server::Settings;
use pretty_assertions::assert_eq;
use reqwest::Url;
use serde::Serialize;
use smart_contract_verifer_mock::SmartContractVerifierServer;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use std::{net::SocketAddr, str::FromStr, time::Duration};
use tonic::transport::Uri;

const DB_PREFIX: &str = "eth_bytecode_db_server";

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

    let mut settings = {
        let mut settings = Settings::default(db_url.into(), verifier_uri);

        settings.server.http.addr = SocketAddr::from_str("127.0.0.1:10000").unwrap();
        settings.server.grpc.enabled = false;
        settings.metrics.enabled = false;
        settings.jaeger.enabled = false;
        settings
    };

    let server_addr = {
        // We do not know what ports are busy, so we just iterate through the range until we find an empty port number
        let mut port = 10000u16;
        while port < 65535 {
            settings.server.http.addr.set_port(port);
            {
                let settings = settings.clone();
                let handle =
                    tokio::spawn(async move { eth_bytecode_db_server::run(settings).await });
                // The assumption is that 1 sec is enough to server to fail in case if the port is busy.
                // In case of blinking tests that is the first place to look for the possible cause.
                tokio::time::sleep(Duration::from_secs(1)).await;
                if !handle.is_finished() {
                    // If server has not failed in 1 second, assume that it would be successfully run
                    break;
                }
            }
            port += 1;
        }
        settings.server.http.addr
    };

    let client = reqwest::Client::new();
    let base = Url::parse(&format!("http://{}", server_addr)).unwrap();

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

    let test_data = test_input_data::input_data_1(service.source_type());

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
