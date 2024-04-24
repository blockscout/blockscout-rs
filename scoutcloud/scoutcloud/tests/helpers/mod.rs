use blockscout_service_launcher::{
    launcher::ConfigSettings, test_database::TestDbGuard, test_server,
};
use reqwest::Url;
use scoutcloud::server::Settings;

pub async fn init_test_db(db_prefix: &str, test_name: &str) -> TestDbGuard {
    let db_name = format!("{db_prefix}_{test_name}");
    TestDbGuard::new::<migration::Migrator>(db_name.as_str()).await
}

pub async fn init_scoutcloud_server<F>(db_url: String, settings_setup: F) -> Url
where
    F: Fn(Settings) -> Settings,
{
    let (settings, base) = {
        std::env::set_var("SCOUTCLOUD__CONFIG", "./tests/config.test.toml");
        std::env::set_var("SCOUTCLOUD__DATABASE__CONNECT__URL", db_url);
        let mut settings = Settings::build().expect("Failed to build settings");
        let (server_settings, base) = test_server::get_test_server_settings();
        settings.server = server_settings;

        (settings_setup(settings), base)
    };

    test_server::init_server(|| scoutcloud::server::run(settings), &base).await;
    base
}
