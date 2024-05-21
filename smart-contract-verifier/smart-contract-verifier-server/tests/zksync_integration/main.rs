mod zksync_solidity;

async fn start() -> url::Url {
    let (settings, base) = {
        let mut settings = smart_contract_verifier_server::Settings::default();
        let (server_settings, base) =
            blockscout_service_launcher::test_server::get_test_server_settings();
        settings.server = server_settings;

        settings.solidity.enabled = false;
        settings.vyper.enabled = false;
        settings.sourcify.enabled = false;

        settings.zksync_solidity.enabled = true;

        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;

        (settings, base)
    };

    blockscout_service_launcher::test_server::init_server(
        || smart_contract_verifier_server::run(settings),
        &base,
    )
    .await;

    base
}
