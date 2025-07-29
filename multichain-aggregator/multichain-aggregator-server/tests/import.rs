#![allow(dead_code)]
mod helpers;

use alloy_primitives::{Address, TxHash};
use blockscout_service_launcher::{database, test_server};
use entity::sea_orm_active_enums::TokenType;
use helpers::{create_test_chains, init_server, upsert_api_keys};
use migration::Migrator;
use multichain_aggregator_logic::types::api_keys::ApiKey;
use pretty_assertions::assert_eq;
use sea_orm::{EntityTrait, prelude::Uuid};
use serde_json::json;

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_import_interop_messages() {
    let db = database!(Migrator);
    let base = init_server(db.db_url()).await;

    create_test_chains(&db, 2).await;
    let api_key = ApiKey {
        key: Uuid::new_v4(),
        chain_id: 1,
    };
    upsert_api_keys(&db, vec![api_key.clone()]).await.unwrap();

    let token_address_hash = Address::repeat_byte(1);

    test_server::send_post_request::<serde_json::Value>(
        &base,
        "/api/v1/import:batch",
        &json!({
            "chain_id": "1",
            "api_key": api_key.key.to_string(),
            "tokens": [
                {
                    "address_hash": token_address_hash,
                    "metadata": {
                        "name": "Test Token",
                        "token_type": "ERC-20",
                    }
                }
            ]
        }),
    )
    .await;

    let get_token = || async {
        entity::tokens::Entity::find_by_id((token_address_hash.clone().to_vec(), 1))
            .one(db.client().as_ref())
            .await
            .unwrap()
            .unwrap()
    };

    let token = get_token().await;
    assert_eq!(token.token_type, TokenType::Erc20);

    test_server::send_post_request::<serde_json::Value>(
        &base,
        "/api/v1/import:batch",
        &json!({
            "chain_id": "1",
            "api_key": api_key.key.to_string(),
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
        }),
    )
    .await;

    let token = get_token().await;
    assert_eq!(token.token_type, TokenType::Erc7802);
}
