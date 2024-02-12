use blockscout_service_launcher::test_server;
use proxy_verifier_server::Settings;
use reqwest::Url;

pub async fn init_proxy_verifier_server<F>(settings_setup: F) -> Url
where
    F: Fn(Settings) -> Settings,
{
    let (settings, base) = {
        let mut settings = Settings::default();
        let (server_settings, base) = test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;

        (settings_setup(settings), base)
    };

    test_server::init_server(|| proxy_verifier_server::run(settings), &base).await;
    base
}
