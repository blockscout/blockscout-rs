use blockscout_service_launcher::{
    {% if database -%}
    test_database::TestDbGuard,
    {% endif -%}
    test_server
};
use reqwest::Url;
use {{crate_name}}_server::Settings;

{% if database -%}
pub async fn init_db(db_prefix: &str, test_name: &str) -> TestDbGuard {
    let db_name = format!("{db_prefix}_{test_name}");
    TestDbGuard::new::<migration::Migrator>(db_name.as_str()).await
}
{% endif -%}

pub async fn init_{{crate_name}}_server<F>(
    {% if database -%}
    db_url: String,
    {% endif -%}
    settings_setup: F
) -> Url
where
    F: Fn(Settings) -> Settings,
{
    let (settings, base) = {
        let mut settings = Settings::default(
            {% if database -%}
            db_url
            {% endif -%}
        );
        let (server_settings, base) = test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;

        (settings_setup(settings), base)
    };

    test_server::init_server(|| {{crate_name}}_server::run(settings), &base).await;
    base
}