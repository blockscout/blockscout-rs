#![allow(dead_code)]
mod helpers;
mod test_db;

use crate::helpers::upsert_api_keys;
use alloy_primitives::Address;
use blockscout_service_launcher::{
    database,
    database::{DatabaseConnectSettings, DatabaseSettings, ReadWriteRepo},
};
use multichain_aggregator_logic::{
    repository::chains,
    services::native_coin_updater::NativeCoinUpdater,
    types::{api_keys::ApiKey, chains::Chain},
};
use pretty_assertions::assert_eq;
use reqwest::StatusCode;
use sea_orm::{EntityTrait, prelude::Uuid};
use serde_json::json;
use std::sync::Arc;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_native_coin_updater() {
    let db = database!(test_db::TestMigrator);

    let mock_server_1 = mock_blockscout_server("Ethereum", "ETH", 18).await;
    let mock_server_2 = mock_blockscout_server("Polygon", "POL", 18).await;

    let chains_data = vec![
        Chain {
            id: 1,
            name: Some("Ethereum".to_string()),
            explorer_url: Some(mock_server_1.uri()),
            icon_url: Some("https://test1".to_string()),
        },
        Chain {
            id: 137,
            name: Some("Polygon".to_string()),
            explorer_url: Some(mock_server_2.uri()),
            icon_url: Some("https://test2".to_string()),
        },
    ];
    let api_keys = chains_data
        .iter()
        .map(|chain| ApiKey {
            key: Uuid::new_v4(),
            chain_id: chain.id,
        })
        .collect::<Vec<_>>();
    chains::upsert_many(db.client().as_ref(), chains_data)
        .await
        .unwrap();
    upsert_api_keys(db.client().as_ref(), api_keys)
        .await
        .unwrap();

    let repo = Arc::new(
        ReadWriteRepo::new::<migration::Migrator>(
            &DatabaseSettings {
                connect: DatabaseConnectSettings::Url(db.db_url()),
                connect_options: Default::default(),
                create_database: false,
                run_migrations: false,
            },
            None,
        )
        .await
        .unwrap(),
    );

    let updater = NativeCoinUpdater::new(repo);
    let native_address = Address::ZERO.to_vec();

    let metadata_endpoint =
        multichain_aggregator_logic::clients::blockscout::node_api_config::NodeApiConfig {};
    updater
        .fetch_and_save_updates(&metadata_endpoint, 2)
        .await
        .unwrap();

    let token_1 = entity::tokens::Entity::find_by_id((native_address.clone(), 1i64))
        .one(db.client().as_ref())
        .await
        .unwrap()
        .expect("ETH token should exist");
    assert_eq!(token_1.name, Some("Ethereum".to_string()));
    assert_eq!(token_1.symbol, Some("ETH".to_string()));
    assert_eq!(token_1.decimals, Some(18));
    assert_eq!(token_1.fiat_value, None);
    assert_eq!(token_1.circulating_market_cap, None);

    let token_137 = entity::tokens::Entity::find_by_id((native_address.clone(), 137i64))
        .one(db.client().as_ref())
        .await
        .unwrap()
        .expect("POL token should exist");
    assert_eq!(token_137.name, Some("Polygon".to_string()));
    assert_eq!(token_137.symbol, Some("POL".to_string()));
    assert_eq!(token_137.decimals, Some(18));
    assert_eq!(token_137.fiat_value, None);
    assert_eq!(token_137.circulating_market_cap, None);

    let price_endpoint = multichain_aggregator_logic::clients::blockscout::stats::Stats {};
    updater
        .fetch_and_save_updates(&price_endpoint, 2)
        .await
        .unwrap();

    let token_1_updated = entity::tokens::Entity::find_by_id((native_address.clone(), 1i64))
        .one(db.client().as_ref())
        .await
        .unwrap()
        .expect("ETH token should still exist");
    assert_eq!(
        token_1_updated.icon_url,
        Some("https://example.com/icon.png".to_string())
    );
    assert!(token_1_updated.fiat_value.is_some());
    assert!(token_1_updated.circulating_market_cap.is_some());

    let token_137_updated = entity::tokens::Entity::find_by_id((native_address, 137i64))
        .one(db.client().as_ref())
        .await
        .unwrap()
        .expect("POL token should still exist");
    assert_eq!(
        token_137_updated.icon_url,
        Some("https://example.com/icon.png".to_string())
    );
    assert!(token_137_updated.fiat_value.is_some());
    assert!(token_137_updated.circulating_market_cap.is_some());
}

async fn mock_blockscout_server(chain_name: &str, symbol: &str, decimals: u8) -> MockServer {
    let mock = MockServer::start().await;

    let config_response = json!({
        "envs": {
            "NEXT_PUBLIC_NETWORK_CURRENCY_NAME": chain_name,
            "NEXT_PUBLIC_NETWORK_CURRENCY_SYMBOL": symbol,
            "NEXT_PUBLIC_NETWORK_CURRENCY_DECIMALS": decimals.to_string(),
            "NEXT_PUBLIC_MARKETPLACE_ENABLED": "false"
        }
    });
    Mock::given(method("GET"))
        .and(path("/node-api/config"))
        .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(config_response))
        .mount(&mock)
        .await;

    let stats_response = json!({
        "coin_price": "1234.56",
        "market_cap": "1000000000",
        "coin_image": "https://example.com/icon.png"
    });
    Mock::given(method("GET"))
        .and(path("/api/v2/stats"))
        .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(stats_response))
        .mount(&mock)
        .await;

    mock
}
