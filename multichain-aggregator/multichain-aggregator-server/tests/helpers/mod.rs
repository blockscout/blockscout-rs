use blockscout_service_launcher::test_server;
use multichain_aggregator_logic::types::api_keys::ApiKey;
use multichain_aggregator_server::Settings;
use reqwest::Url;
use sea_orm::{ActiveValue::Set, DatabaseConnection, DbErr, EntityTrait};

pub async fn init_multichain_aggregator_server<F>(db_url: String, settings_setup: F) -> Url
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

        (settings_setup(settings), base)
    };

    test_server::init_server(|| multichain_aggregator_server::run(settings), &base).await;
    base
}

#[allow(dead_code)]
pub async fn upsert_api_keys(db: &DatabaseConnection, api_keys: Vec<ApiKey>) -> Result<(), DbErr> {
    let api_keys = api_keys.into_iter().map(|k| entity::api_keys::ActiveModel {
        key: Set(k.key),
        chain_id: Set(k.chain_id),
        ..Default::default()
    });

    entity::api_keys::Entity::insert_many(api_keys)
        .exec_without_returning(db)
        .await?;

    Ok(())
}
