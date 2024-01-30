use crate::verification_test_helpers::{
    init_alliance_db, init_db, init_eth_bytecode_db_server_with_settings_setup,
    init_verifier_server, verifier_alliance_types::TestCase, VerifierService,
};
use async_trait::async_trait;
use blockscout_service_launcher::test_database::TestDbGuard;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use eth_bytecode_db_server::Settings;
use futures::future::BoxFuture;
use sea_orm::DatabaseConnection;
use smart_contract_verifier_proto::{
    blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2,
    http_client::mock::{MockSolidityVerifierService, SmartContractVerifierServer},
};
use std::{collections::HashMap, future::Future, path::PathBuf, str::FromStr, sync::Arc};
use tonic::Response;

const VERIFICATION_ROUTE: &str = "/api/v2/verifier/solidity/sources:verify-standard-json";

const API_KEY_NAME: &str = "x-api-key";

const API_KEY: &str = "some api key";

fn verify_request(test_case: &TestCase) -> eth_bytecode_db_v2::VerifySolidityStandardJsonRequest {
    let transaction_hash =
        (!test_case.is_genesis).then_some(test_case.transaction_hash.to_string());
    let block_number = (!test_case.is_genesis).then_some(test_case.block_number);
    let transaction_index = (!test_case.is_genesis).then_some(test_case.transaction_index);
    let deployer = (!test_case.is_genesis).then_some(test_case.deployer.to_string());
    let metadata = eth_bytecode_db_v2::VerificationMetadata {
        chain_id: Some(format!("{}", test_case.chain_id)),
        contract_address: Some(test_case.address.to_string()),
        transaction_hash,
        block_number,
        transaction_index,
        deployer,
        creation_code: test_case
            .deployed_creation_code
            .as_ref()
            .map(ToString::to_string),
        runtime_code: Some(test_case.deployed_runtime_code.to_string()),
    };

    eth_bytecode_db_v2::VerifySolidityStandardJsonRequest {
        bytecode: "".to_string(),
        bytecode_type: eth_bytecode_db_v2::BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        input: "".to_string(),
        metadata: Some(metadata),
    }
}

pub struct RequestWrapper<'a, Request> {
    inner: &'a Request,
    headers: reqwest::header::HeaderMap,
}

impl<'a, Request> From<&'a Request> for RequestWrapper<'a, Request> {
    fn from(value: &'a Request) -> Self {
        Self {
            inner: value,
            headers: Default::default(),
        }
    }
}

impl<'a, Request> RequestWrapper<'a, Request> {
    pub fn header(&mut self, key: &str, value: &str) {
        let key = reqwest::header::HeaderName::from_str(key)
            .expect("Error converting key string into header name");
        let value = reqwest::header::HeaderValue::from_str(value)
            .expect("Error converting value string into header value");
        self.headers.insert(key, value);
    }

    pub fn headers(&mut self, headers: HashMap<String, String>) {
        for (key, value) in headers {
            self.header(&key, &value);
        }
    }
}

async fn send_request<Request: serde::Serialize, Response: for<'a> serde::Deserialize<'a>>(
    eth_bytecode_db_base: &reqwest::Url,
    route: &str,
    request: &RequestWrapper<'_, Request>,
) -> Response {
    let response = reqwest::Client::new()
        .post(eth_bytecode_db_base.join(route).unwrap())
        .json(&request.inner)
        .headers(request.headers.clone())
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to send request"));

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

pub trait SetupDbFn {
    fn setup(&self, db: Arc<DatabaseConnection>, test_case: TestCase) -> BoxFuture<'static, ()>;
}

impl<F, Fut> SetupDbFn for F
where
    F: Fn(Arc<DatabaseConnection>, TestCase) -> Fut,
    Fut: Future<Output = ()> + 'static + Send,
{
    fn setup(&self, db: Arc<DatabaseConnection>, test_case: TestCase) -> BoxFuture<'static, ()> {
        Box::pin(self(db, test_case))
    }
}

pub struct SetupData {
    pub eth_bytecode_db_base: url::Url,
    pub eth_bytecode_db: TestDbGuard,
    pub alliance_db: TestDbGuard,
    pub test_case: TestCase,
}

pub struct Setup<'a> {
    test_prefix: &'a str,
    setup_db: Box<dyn SetupDbFn>,
    is_authorized: bool,
    alliance_db: Option<TestDbGuard>,
}

impl<'a> Setup<'a> {
    pub fn new(test_prefix: &'a str) -> Self {
        let noop_setup_db = |_db: Arc<DatabaseConnection>, _test_case: TestCase| async {};
        Self {
            test_prefix,
            setup_db: Box::new(noop_setup_db),
            is_authorized: false,
            alliance_db: None,
        }
    }

    pub async fn setup(&self, test_suite_name: &str, test_case_path: PathBuf) -> SetupData {
        let test_case = TestCase::from_file(test_case_path);

        let service = MockSolidityVerifierService::new();

        let test_name = format!("{}_{}", self.test_prefix, test_case.test_case_name,);

        let db = init_db(test_suite_name, &test_name).await;
        let alliance_db = match self.alliance_db.clone() {
            Some(alliance_db) => alliance_db,
            None => init_alliance_db(test_suite_name, &test_name).await,
        };

        let test_input_data = test_case.to_test_input_data();

        self.setup_db
            .setup(alliance_db.client(), test_case.clone())
            .await;

        let db_url = db.db_url();
        let verifier_addr = init_verifier_server::<
            _,
            eth_bytecode_db_v2::VerifySolidityStandardJsonRequest,
            _,
        >(service, test_input_data.verifier_response)
        .await;

        let settings_setup = |mut settings: Settings| {
            settings.verifier_alliance_database.enabled = true;
            settings.verifier_alliance_database.url = alliance_db.db_url().to_string();

            settings.authorized_keys =
                serde_json::from_value(serde_json::json!({"blockscout": {"key": API_KEY}}))
                    .unwrap();
            settings
        };
        let eth_bytecode_db_base =
            init_eth_bytecode_db_server_with_settings_setup(db_url, verifier_addr, settings_setup)
                .await;
        // Fill the database with existing value
        {
            let dummy_request = verify_request(&test_case);
            let mut wrapped_request: RequestWrapper<_> = (&dummy_request).into();
            if self.is_authorized {
                wrapped_request.header(API_KEY_NAME, API_KEY);
            }
            let _verification_response: eth_bytecode_db_v2::VerifyResponse =
                send_request(&eth_bytecode_db_base, VERIFICATION_ROUTE, &wrapped_request).await;
        }

        SetupData {
            eth_bytecode_db_base,
            eth_bytecode_db: db,
            alliance_db,
            test_case,
        }
    }

    pub fn setup_db(mut self, function: impl SetupDbFn + 'static) -> Self {
        self.setup_db = Box::new(function);
        self
    }

    pub fn authorized(mut self) -> Self {
        self.is_authorized = true;
        self
    }

    pub fn alliance_db(mut self, alliance_db: TestDbGuard) -> Self {
        self.alliance_db = Some(alliance_db);
        self
    }
}
