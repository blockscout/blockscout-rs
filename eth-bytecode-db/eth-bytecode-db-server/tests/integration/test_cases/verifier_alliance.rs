use crate::{
    types,
    types::{
        verifier_alliance::TestCase, Request, Route, VerifierMock, VerifierRequest,
        VerifierResponse, VerifierRoute,
    },
    EthBytecodeDbDatabaseChecker, VerifierAllianceDatabaseChecker,
};
use blockscout_service_launcher::test_database::TestDbGuard;
use blockscout_service_launcher::{test_server};
use eth_bytecode_db_server::Settings;
use std::net::SocketAddr;
use std::{path::PathBuf, str::FromStr};
use url::Url;

const API_KEY_NAME: &str = "x-api-key";

const API_KEY: &str = "some api key";

pub struct SetupResult {
    eth_bytecode_db_db: TestDbGuard,
    alliance_db: TestDbGuard,
    eth_bytecode_db_service: Url,
}

pub async fn setup(test_case_name: &str, verifier_addr: SocketAddr) -> SetupResult {
    let eth_bytecode_db_db =
        TestDbGuard::new::<migration::Migrator>(&format!("verifier_alliance_{test_case_name}"))
            .await;

    let alliance_db = TestDbGuard::new::<verifier_alliance_migration::Migrator>(&format!(
        "alliance_verifier_alliance_{test_case_name}"
    ))
    .await;

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
            settings.authorized_keys =
                serde_json::from_value(serde_json::json!({"blockscout": {"key": API_KEY}}))
                    .unwrap();

            (settings, base)
        };

        test_server::init_server(|| eth_bytecode_db_server::run(settings), &base).await;

        base
    };

    SetupResult {
        eth_bytecode_db_db,
        alliance_db,
        eth_bytecode_db_service,
    }
}

pub async fn initialize_verifier_service<Rou: Route>(test_cases: &[TestCase]) -> SocketAddr
where
    TestCase: Request<Rou>,
    TestCase: VerifierRequest<<<Rou as Route>::VerifierRoute as VerifierRoute>::Request>,
    TestCase: VerifierResponse<<<Rou as Route>::VerifierRoute as VerifierRoute>::Response>,
{
    let mut mock_verifier =
        <<Rou as Route>::VerifierRoute as VerifierRoute>::MockService::default();
    for test_case in test_cases {
        mock_verifier.expect(test_case.clone());
    }
    mock_verifier
        .add_as_service(
            smart_contract_verifier_proto::http_client::mock::SmartContractVerifierServer::new(),
        )
        .start()
        .await
}

async fn send_post_request<Rou: Route>(
    eth_bytecode_db_base: &Url,
    request: &<Rou as Route>::Request,
    headers: &[(&str, &str)],
) -> <Rou as Route>::Response {
    let headers = headers
        .into_iter()
        .map(|(key, value)| {
            let key = reqwest::header::HeaderName::from_str(key)
                .expect("Error converting key string into header name");
            let value = reqwest::header::HeaderValue::from_str(value)
                .expect("Error converting value string into header value");
            (key, value)
        })
        .collect();
    let response = reqwest::Client::new()
        .post(eth_bytecode_db_base.join(<Rou as Route>::ROUTE).unwrap())
        .json(request)
        .headers(headers)
        .send()
        .await
        .unwrap_or_else(|err| panic!("Failed to send request: {err}"));

    // Assert that status code is success
    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await.expect("Read body as text");
        panic!("Invalid status code (success expected). Status: {status}. Message: {message}")
    }

    response
        .json()
        .await
        .unwrap_or_else(|_| panic!("Response deserialization failed"))
}

pub async fn success<Rou: Route>(test_case_path: PathBuf)
where
    TestCase: Request<Rou>,
    TestCase: VerifierRequest<<<Rou as Route>::VerifierRoute as VerifierRoute>::Request>,
    TestCase: VerifierResponse<<<Rou as Route>::VerifierRoute as VerifierRoute>::Response>,
{
    let test_case_name = format!(
        "{}_{}",
        <Rou as Route>::ROUTE,
        test_case_path.as_path().to_string_lossy()
    )
    .replace(|c: char| !c.is_alphanumeric(), "_");

    let test_case = types::from_path::<Rou, TestCase>(&test_case_path);

    let verifier_addr = initialize_verifier_service(&[test_case.clone()]).await;

    let SetupResult {
        alliance_db,
        eth_bytecode_db_db,
        eth_bytecode_db_service,
    } = setup(&test_case_name, verifier_addr).await;

    let request = <TestCase as Request<Rou>>::to_request(&test_case);
    let _response: <Rou as Route>::Response = send_post_request::<Rou>(
        &eth_bytecode_db_service,
        &request,
        &[(API_KEY_NAME, API_KEY)],
    )
    .await;

    // Check that correct data inserted into verifier alliance database
    {
        let alliance_db = alliance_db.client();
        let alliance_db = alliance_db.as_ref();

        let contract_deployment = test_case.check_contract_deployment(alliance_db).await;
        let compiled_contract = test_case.check_compiled_contract(alliance_db).await;
        test_case
            .check_verified_contract(alliance_db, &contract_deployment, &compiled_contract)
            .await;
    }

    // Check that correct data inserted into eth bytecode db database
    {
        let db = eth_bytecode_db_db.client();
        let db = db.as_ref();

        let source = test_case.check_source(db).await;
        let files = test_case.check_files(db).await;
        test_case.check_source_files(db, &source, &files).await;
        let bytecodes = test_case.check_bytecodes(db, &source).await;
        let parts = test_case.check_parts(db).await;
        test_case.check_bytecode_parts(db, &bytecodes, &parts).await;
    }
}
