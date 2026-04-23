mod health;
mod settings;
mod solidity;

pub async fn init_server() -> url::Url {
    let (settings, base) = {
        let mut settings = visualizer_server::Settings::default();
        let (server_settings, base) =
            blockscout_service_launcher::test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;

        (settings, base)
    };

    blockscout_service_launcher::test_server::init_server(
        || visualizer_server::run(settings),
        &base,
    )
    .await;
    base
}
