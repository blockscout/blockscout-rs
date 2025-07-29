#![allow(dead_code)]
mod helpers;

use std::pin::Pin;

use alloy_primitives::{Address, TxHash};
use blockscout_service_launcher::{database, test_server};
use entity::sea_orm_active_enums::TokenType;
use helpers::{create_test_chains, init_server, upsert_api_keys};
use migration::Migrator;
use multichain_aggregator_logic::types::api_keys::ApiKey;
use pretty_assertions::assert_eq;
use sea_orm::{
    DatabaseConnection, EntityTrait,
    prelude::{BigDecimal, Uuid},
};
use serde_json::json;
use url::Url;

async fn prepare_import_fn(
    db: &DatabaseConnection,
    base: &Url,
) -> impl Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = serde_json::Value>>> {
    create_test_chains(db, 1).await;
    let api_key = ApiKey {
        key: Uuid::new_v4(),
        chain_id: 1,
    };
    upsert_api_keys(db, vec![api_key.clone()]).await.unwrap();

    move |mut payload: serde_json::Value| {
        let base = base.clone();
        let api_key = api_key.clone();
        Box::pin(async move {
            if let serde_json::Value::Object(ref mut obj) = payload {
                obj.insert("chain_id".to_string(), "1".into());
                obj.insert("api_key".to_string(), api_key.key.to_string().into());
            }
            test_server::send_post_request::<serde_json::Value>(
                &base,
                "/api/v1/import:batch",
                &payload,
            )
            .await
        }) as Pin<_>
    }
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_import_interop_messages() {
    let db = database!(Migrator);
    let base = init_server(db.db_url()).await;

    let import = prepare_import_fn(&db, &base).await;

    let token_address_hash = Address::repeat_byte(1);

    let get_token = || async {
        entity::tokens::Entity::find_by_id((1, token_address_hash.clone().to_vec()))
            .one(db.client().as_ref())
            .await
            .unwrap()
            .unwrap()
    };

    import(json!({
        "tokens": [
            {
                "address_hash": token_address_hash,
                "metadata": {
                    "name": "Test Token",
                    "token_type": "ERC-20",
                }
            }
        ]
    }))
    .await;

    let token = get_token().await;
    assert_eq!(token.token_type, TokenType::Erc20);

    import(json!({
        "interop_messages": [
            {
                "init": {
                    "sender_address_hash": Address::random(),
                    "target_address_hash": Address::random(),
                    "nonce": "1",
                    "init_chain_id": "1",
                    "relay_chain_id": "2",
                    "init_transaction_hash": TxHash::random(),
                    "timestamp": "1",
                    "payload": "",
                    "transfer_token_address_hash": token_address_hash,
                    "transfer_from_address_hash": Address::random(),
                    "transfer_to_address_hash": Address::random(),
                    "transfer_amount": "1"
                },
        }
        ]
    }))
    .await;

    let token = get_token().await;
    assert_eq!(token.token_type, TokenType::Erc7802);
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_import_token() {
    let db = database!(Migrator);
    let base = init_server(db.db_url()).await;

    let import = prepare_import_fn(&db, &base).await;

    let token_address_hash = Address::repeat_byte(1);

    let get_token = || async {
        entity::tokens::Entity::find_by_id((1, token_address_hash.clone().to_vec()))
            .one(db.client().as_ref())
            .await
            .unwrap()
    };

    // Initial partial import should take no effect
    import(json!({
        "tokens": [
            {
                "address_hash": token_address_hash,
                "metadata": {
                    "total_supply": "1234"
                }
            }
        ]
    }))
    .await;

    let token = get_token().await;
    assert_eq!(token.is_none(), true);

    // Import with token type should create a new token
    import(json!({
        "tokens": [
            {
                "address_hash": token_address_hash,
                "metadata": {
                    "name": "Test Token",
                    "token_type": "ERC-721",
                }
            }
        ]
    }))
    .await;

    // Partial import should update the token
    import(json!({
        "tokens": [
            {
                "address_hash": token_address_hash,
                "metadata": {
                    "total_supply": "1234"
                }
            }
        ]
    }))
    .await;

    let token = get_token().await.unwrap();
    assert_eq!(token.name, Some("Test Token".to_string()));
    assert_eq!(token.token_type, TokenType::Erc721);
    assert_eq!(token.total_supply, Some(BigDecimal::from(1234)));
}
