#![allow(dead_code)]
mod helpers;

use alloy_primitives::Address;
use blockscout_service_launcher::{database, test_server};
use helpers::{create_test_chains, init_server_with_setup, upsert_api_keys};
use migration::Migrator;
use multichain_aggregator_logic::types::api_keys::ApiKey;
use multichain_aggregator_proto::blockscout::cluster_explorer::v1 as cluster_proto;
use sea_orm::prelude::Uuid;
use serde_json::json;

const METADATA_IMPORT_API_KEY: &str = "test-metadata-import-key";

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_poor_reputation_tokens_filtered() {
    let db = database!(Migrator);

    create_test_chains(db.client().as_ref(), 1).await;
    let api_key = ApiKey {
        key: Uuid::new_v4(),
        chain_id: 1,
    };
    upsert_api_keys(db.client().as_ref(), vec![api_key.clone()])
        .await
        .unwrap();

    let base = init_server_with_setup(db.db_url(), |mut s| {
        s.service.metadata_import_api_key = Some(METADATA_IMPORT_API_KEY.to_string());
        s.cluster_explorer.clusters.insert(
            "test".to_string(),
            serde_json::from_value(json!({ "chain_ids": "1" })).unwrap(),
        );
        s
    })
    .await;

    let good_token = Address::repeat_byte(0x01);
    let bad_token = Address::repeat_byte(0x02);
    let holder = Address::repeat_byte(0xAA);

    // Import two tokens and balances
    test_server::send_post_request::<serde_json::Value>(
        &base,
        "/api/v1/import:batch",
        &json!({
            "chain_id": "1",
            "api_key": api_key.key.to_string(),
            "tokens": [
                {
                    "address_hash": good_token.to_string(),
                    "metadata": { "name": "Good Token", "symbol": "GOOD", "token_type": "ERC-20" }
                },
                {
                    "address_hash": bad_token.to_string(),
                    "metadata": { "name": "Bad Token", "symbol": "BAD", "token_type": "ERC-20" }
                }
            ],
            "address_token_balances": [
                {
                    "address_hash": holder.to_string(),
                    "token_address_hash": good_token.to_string(),
                    "value": "1000"
                },
                {
                    "address_hash": holder.to_string(),
                    "token_address_hash": bad_token.to_string(),
                    "value": "2000"
                }
            ]
        }),
    )
    .await;

    // Mark bad_token as poor reputation
    test_server::send_post_request::<serde_json::Value>(
        &base,
        "/api/v1/import:poor-reputation-tokens",
        &json!({
            "api_key": METADATA_IMPORT_API_KEY,
            "tokens": [
                {
                    "address_hash": bad_token.to_string(),
                    "chain_id": "1"
                }
            ]
        }),
    )
    .await;

    // Query without include_poor_reputation_tokens
    let response: cluster_proto::ListAddressTokensResponse = test_server::send_get_request(
        &base,
        &format!("/api/v1/clusters/test/addresses/{holder}/tokens"),
    )
    .await;

    assert_eq!(
        response.items.len(),
        1,
        "poor reputation token should be filtered out"
    );
    let token_name = response.items[0]
        .token
        .as_ref()
        .unwrap()
        .name
        .as_deref()
        .unwrap_or("");
    assert_eq!(token_name, "Good Token");

    // Query with include_poor_reputation_tokens
    let response: cluster_proto::ListAddressTokensResponse = test_server::send_get_request(
        &base,
        &format!(
            "/api/v1/clusters/test/addresses/{holder}/tokens?include_poor_reputation_tokens=true"
        ),
    )
    .await;

    assert_eq!(
        response.items.len(),
        2,
        "poor reputation token should be included when requested"
    );

    let mut names: Vec<String> = response
        .items
        .iter()
        .filter_map(|i| i.token.as_ref()?.name.clone())
        .collect();
    names.sort();
    assert_eq!(names, vec!["Bad Token", "Good Token"]);
}
