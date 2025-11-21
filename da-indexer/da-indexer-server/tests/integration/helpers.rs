use blockscout_service_launcher::test_server;
use da_indexer_server::Settings;
use url::Url;

pub async fn init_server_with_setup<F>(db_url: String, settings_setup: F) -> Url
where
    F: Fn(Settings) -> Settings,
{
    let (settings, base) = {
        let mut settings = Settings::default(db_url);
        let (server_settings, base) = test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;

        settings.s3_storage = None;
        settings.indexer = None;
        settings.l2_router = None;

        (settings_setup(settings), base)
    };

    test_server::init_server(|| da_indexer_server::run(settings), &base).await;
    base
}

pub async fn init_server(db_url: String) -> Url {
    init_server_with_setup(db_url, |x| x).await
}
