// SPDX-License-Identifier: LicenseRef-Blockscout

//! Runs alone in its process: the compiler runner registers its metrics once, under the prefix
//! in effect when the first executor is built.

use blockscout_service_launcher::test_server::{get_test_server_settings, init_server};
use smart_contract_verifier_server::Settings;

#[tokio::test]
async fn compiler_runner_metrics_keep_the_service_prefix() {
    let mut settings = Settings::default();
    let (server_settings, base) = get_test_server_settings();
    settings.server = server_settings;
    settings.solidity.enabled = false;
    settings.vyper.enabled = false;
    settings.sourcify.enabled = false;
    settings.zksync_solidity.enabled = false;
    settings.jaeger.enabled = false;
    settings.tracing.enabled = false;

    init_server(|| smart_contract_verifier_server::run(settings), &base).await;

    let names: Vec<String> = prometheus::gather()
        .iter()
        .map(|family| family.name().to_string())
        .collect();
    assert!(
        names.contains(&"smart_contract_verifier_compiler_runner_max_concurrent_jobs".to_string()),
        "compiler runner metrics are missing the service prefix: {names:?}"
    );
    assert!(
        !names
            .iter()
            .any(|name| name.starts_with("compiler_runner_")),
        "compiler runner metrics were registered without a prefix: {names:?}"
    );
}
