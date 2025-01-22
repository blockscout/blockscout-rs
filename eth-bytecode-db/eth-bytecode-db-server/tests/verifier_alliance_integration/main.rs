mod solidity_batch_import;
mod solidity_verify;

/****************************************/

use blockscout_service_launcher::{database, test_database::TestDbGuard, test_server};
use eth_bytecode_db_proto::http_client;
use eth_bytecode_db_server::Settings;
use url::Url;

const API_KEY: &str = "some api key";

#[derive(Debug, Clone)]
pub struct SetupResult {
    service_client: http_client::Client,
}

async fn setup(test_case_name: &str, alliance_database: TestDbGuard) -> SetupResult {
    let bytecode_database_prefix = format!(
        "{}_{}",
        std::backtrace::Backtrace::force_capture(),
        test_case_name
    )
    .replace(|c: char| !c.is_ascii_alphanumeric(), "_");
    let bytecode_database = database!(migration::Migrator, bytecode_database_prefix);

    let (settings, base) = {
        let verifier_url = Url::parse(
            std::env::var("VERIFIER_URL")
                .unwrap_or_else(|_| {
                    "https://http.sc-verifier-test.k8s-dev.blockscout.com/".to_string()
                })
                .as_str(),
        )
        .expect("Invalid verifier URL provided");
        let mut settings = Settings::default(bytecode_database.db_url(), verifier_url);
        let (server_settings, base) = test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;

        settings.verifier_alliance_database.enabled = true;
        settings.verifier_alliance_database.url = alliance_database.db_url();
        settings.authorized_keys =
            serde_json::from_value(serde_json::json!({"blockscout": {"key": API_KEY}})).unwrap();

        (settings, base)
    };

    test_server::init_server(|| eth_bytecode_db_server::run(settings), &base).await;

    let client_config =
        http_client::Config::new(base.to_string()).set_api_key(Some(API_KEY.to_string()));
    let client = http_client::Client::new(client_config).await;

    SetupResult {
        service_client: client,
    }
}
