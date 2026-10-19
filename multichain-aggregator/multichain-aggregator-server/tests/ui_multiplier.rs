// SPDX-License-Identifier: LicenseRef-Blockscout

#![allow(dead_code)]
mod helpers;

use alloy_primitives::Address;
use blockscout_service_launcher::{database, test_server};
use helpers::{create_test_chains, init_server_with_setup, upsert_api_keys};
use migration::Migrator;
use multichain_aggregator_logic::types::api_keys::ApiKey;
use multichain_aggregator_proto::blockscout::{
    cluster_explorer::v1 as cluster_proto, multichain_aggregator::v1::TokenType,
};
use pretty_assertions::assert_eq;
use sea_orm::prelude::Uuid;
use serde_json::json;

// 1e18, i.e. a multiplier of 1.0
const UI_MULTIPLIER: &str = "1000000000000000000";
// 1.5e18, the multiplier scheduled to replace it
const NEW_UI_MULTIPLIER: &str = "1500000000000000000";
// RFC 3339 in and out; the response always carries millisecond precision
const EFFECTIVE_AT_IN: &str = "2026-09-23T10:00:00Z";
const EFFECTIVE_AT_RFC3339: &str = "2026-09-23T10:00:00.000Z";

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_ui_multiplier_is_exposed() {
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
        s.cluster_explorer.clusters.insert(
            "test".to_string(),
            serde_json::from_value(json!({ "chain_ids": "1" })).unwrap(),
        );
        s
    })
    .await;

    let scaled_token = Address::repeat_byte(0x01);
    let plain_token = Address::repeat_byte(0x02);
    let holder = Address::repeat_byte(0xAA);

    test_server::send_post_request::<serde_json::Value>(
        &base,
        "/api/v1/import:batch",
        &json!({
            "chain_id": "1",
            "api_key": api_key.key.to_string(),
            "tokens": [
                {
                    "address_hash": scaled_token.to_string(),
                    "metadata": {
                        "name": "Scaled Token",
                        "symbol": "SCALED",
                        "decimals": 18,
                        "token_type": "ERC-8056",
                        "ui_multiplier": UI_MULTIPLIER,
                        "new_ui_multiplier": NEW_UI_MULTIPLIER,
                        "ui_multiplier_effective_at": EFFECTIVE_AT_IN,
                    }
                },
                {
                    "address_hash": plain_token.to_string(),
                    "metadata": {
                        "name": "Plain Token",
                        "symbol": "PLAIN",
                        "decimals": 18,
                        "token_type": "ERC-20"
                    }
                }
            ],
            "address_token_balances": [
                {
                    "address_hash": holder.to_string(),
                    "token_address_hash": scaled_token.to_string(),
                    "value": "1000"
                },
                {
                    "address_hash": holder.to_string(),
                    "token_address_hash": plain_token.to_string(),
                    "value": "2000"
                }
            ]
        }),
    )
    .await;

    // Address token balances carry the token's current multiplier
    let response: cluster_proto::ListAddressTokensResponse = test_server::send_get_request(
        &base,
        &format!("/api/v1/clusters/test/addresses/{holder}/tokens"),
    )
    .await;

    let by_symbol = |symbol: &str| -> cluster_proto::AggregatedTokenInfo {
        response
            .items
            .iter()
            .filter_map(|i| i.token.as_ref())
            .find(|t| t.symbol.as_deref() == Some(symbol))
            .unwrap_or_else(|| panic!("{symbol} missing from the response"))
            .clone()
    };

    let scaled = by_symbol("SCALED");
    assert_eq!(scaled.ui_multiplier.as_deref(), Some(UI_MULTIPLIER));
    assert_eq!(scaled.new_ui_multiplier.as_deref(), Some(NEW_UI_MULTIPLIER));
    assert_eq!(
        scaled.ui_multiplier_effective_at.as_deref(),
        Some(EFFECTIVE_AT_RFC3339)
    );

    // A token that declares no multiplier reports all three as null
    let plain = by_symbol("PLAIN");
    assert_eq!(plain.ui_multiplier, None);
    assert_eq!(plain.new_ui_multiplier, None);
    assert_eq!(plain.ui_multiplier_effective_at, None);

    // ERC-8056 is accepted as a token type filter
    let response: cluster_proto::ListAddressTokensResponse = test_server::send_get_request(
        &base,
        &format!("/api/v1/clusters/test/addresses/{holder}/tokens?type=ERC-8056"),
    )
    .await;

    let tokens: Vec<&cluster_proto::AggregatedTokenInfo> = response
        .items
        .iter()
        .filter_map(|i| i.token.as_ref())
        .collect();
    assert_eq!(
        tokens.len(),
        1,
        "only the ERC-8056 token should be returned"
    );
    assert_eq!(tokens[0].symbol.as_deref(), Some("SCALED"));
    assert_eq!(tokens[0].r#type(), TokenType::Erc8056);
    assert_eq!(tokens[0].ui_multiplier.as_deref(), Some(UI_MULTIPLIER));
    assert_eq!(
        tokens[0].new_ui_multiplier.as_deref(),
        Some(NEW_UI_MULTIPLIER)
    );
    assert_eq!(
        tokens[0].ui_multiplier_effective_at.as_deref(),
        Some(EFFECTIVE_AT_RFC3339)
    );

    // Cluster token list carries it too
    let response: cluster_proto::ListClusterTokensResponse =
        test_server::send_get_request(&base, "/api/v1/clusters/test/tokens?type=ERC-8056").await;

    assert_eq!(response.items.len(), 1);
    assert_eq!(
        response.items[0].ui_multiplier.as_deref(),
        Some(UI_MULTIPLIER)
    );

    // As does the single-token endpoint
    let response: cluster_proto::GetAggregatedTokenResponse = test_server::send_get_request(
        &base,
        &format!("/api/v1/clusters/test/tokens/{scaled_token}?chain_id=1"),
    )
    .await;

    assert_eq!(
        response.token.unwrap().ui_multiplier.as_deref(),
        Some(UI_MULTIPLIER)
    );
}
