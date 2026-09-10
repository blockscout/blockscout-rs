// SPDX-License-Identifier: LicenseRef-Blockscout

//! DB-backed HTTP contract tests for
//! `GET /api/v1/interchain/messages/{message_id}` and its optional `bridge_id`
//! qualifier.

mod helpers;

use blockscout_service_launcher::{test_database::TestDbGuard, test_server};
use chrono::Utc;
use interchain_indexer_entity::{
    bridges, crosschain_messages, crosschain_transfers,
    sea_orm_active_enums::{MessageStatus, TransferType},
};
use pretty_assertions::assert_eq;
use reqwest::StatusCode;
use sea_orm::{ActiveValue::Set, EntityTrait, prelude::BigDecimal};

/// Public numeric message ID that intentionally collides across two bridges.
/// `7777 == 0x1e61`.
const COLLIDING_MESSAGE_ID: i64 = 7777;
const COLLIDING_MESSAGE_HEX: &str = "0x1e61";

/// Public numeric message ID for the seeded unresolved-destination message.
/// `9999 == 0x270f`.
const UNRESOLVED_DESTINATION_MESSAGE_ID: i64 = 9999;
const UNRESOLVED_DESTINATION_MESSAGE_HEX: &str = "0x270f";

/// Public numeric message ID for the seeded fully-resolved message.
/// `6666 == 0x1a0a`.
const RESOLVED_MESSAGE_ID: i64 = 6666;
const RESOLVED_MESSAGE_HEX: &str = "0x1a0a";

/// Seeds a single numeric message ID under bridge 1 and bridge 2 with distinct
/// transfer amounts so a wrong-bridge selection cannot pass silently.
///
/// Bridge 1 is already upserted from the service config on startup; bridge 2 is
/// DB-only, so the service's bridge map does not know it and its `bridge.id`
/// falls back to the raw stored id (2), which is sufficient for these assertions.
async fn seed_bridge_collision(db: &TestDbGuard) {
    let conn = db.client();

    bridges::Entity::insert(bridges::ActiveModel {
        id: Set(2),
        name: Set("DbOnlyBridge".to_string()),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();

    let now = Utc::now().naive_utc();
    crosschain_messages::Entity::insert_many([
        crosschain_messages::ActiveModel {
            id: Set(COLLIDING_MESSAGE_ID),
            bridge_id: Set(1),
            status: Set(MessageStatus::Initiated),
            init_timestamp: Set(now),
            src_chain_id: Set(1),
            dst_chain_id: Set(Some(100)),
            payload: Set(Some(vec![0xB1])),
            ..Default::default()
        },
        crosschain_messages::ActiveModel {
            id: Set(COLLIDING_MESSAGE_ID),
            bridge_id: Set(2),
            status: Set(MessageStatus::Completed),
            init_timestamp: Set(now),
            src_chain_id: Set(1),
            dst_chain_id: Set(Some(100)),
            payload: Set(Some(vec![0xB2])),
            ..Default::default()
        },
    ])
    .exec(conn.as_ref())
    .await
    .unwrap();

    crosschain_transfers::Entity::insert_many([
        crosschain_transfers::ActiveModel {
            id: Set(7701),
            message_id: Set(COLLIDING_MESSAGE_ID),
            bridge_id: Set(1),
            index: Set(0),
            r#type: Set(Some(TransferType::Erc20)),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(Some(BigDecimal::from(111u32))),
            dst_amount: Set(Some(BigDecimal::from(111u32))),
            token_ids: Set(None),
            ..Default::default()
        },
        crosschain_transfers::ActiveModel {
            id: Set(7702),
            message_id: Set(COLLIDING_MESSAGE_ID),
            bridge_id: Set(2),
            index: Set(0),
            r#type: Set(Some(TransferType::Erc20)),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(Some(BigDecimal::from(222u32))),
            dst_amount: Set(Some(BigDecimal::from(222u32))),
            token_ids: Set(None),
            ..Default::default()
        },
    ])
    .exec(conn.as_ref())
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn message_details_bridge_qualifier_contract() {
    let db = helpers::init_db("test", "message_details_bridge_qualifier_contract").await;
    let db_url = db.db_url();
    let base = helpers::init_interchain_indexer_server(db_url, |x| x).await;

    seed_bridge_collision(&db).await;

    // 1. Unqualified collision -> HTTP 400, tonic code 9, "provide bridge_id".
    let route = format!("/api/v1/interchain/messages/{COLLIDING_MESSAGE_HEX}");
    let (status, body) = helpers::get_raw(&base, &route).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], serde_json::json!(9));
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("provide bridge_id"),
        "unexpected ambiguity message: {body}"
    );

    // 2. Qualified with bridge 1 -> HTTP 200, bridge.id == 1, its transfer only.
    let qualified_1: serde_json::Value =
        test_server::send_get_request(&base, &format!("{route}?bridge_id=1")).await;
    assert_eq!(qualified_1["bridge"]["id"], serde_json::json!(1));
    assert_eq!(qualified_1["transfers"].as_array().unwrap().len(), 1);
    assert_eq!(
        qualified_1["transfers"][0]["source_amount"],
        serde_json::json!("111")
    );

    // 3. Qualified with bridge 2 -> HTTP 200, bridge.id == 2, its transfer only.
    let qualified_2: serde_json::Value =
        test_server::send_get_request(&base, &format!("{route}?bridge_id=2")).await;
    assert_eq!(qualified_2["bridge"]["id"], serde_json::json!(2));
    assert_eq!(qualified_2["transfers"].as_array().unwrap().len(), 1);
    assert_eq!(
        qualified_2["transfers"][0]["source_amount"],
        serde_json::json!("222")
    );

    // 4. bridge_id above i32::MAX (but within u32) -> HTTP 400, tonic code 3
    //    (InvalidArgument), rejected by the checked conversion.
    let (status, body) = helpers::get_raw(&base, &format!("{route}?bridge_id={}", u32::MAX)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], serde_json::json!(3));
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("bridge_id exceeds the supported int32 range"),
        "unexpected invalid-argument message: {body}"
    );

    // 5. Malformed (non-hex) message_id -> HTTP 400, tonic code 3
    //    (InvalidArgument), rejected before any DB lookup rather than surfacing
    //    a generic internal error.
    let (status, body) = helpers::get_raw(&base, "/api/v1/interchain/messages/not-hex").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], serde_json::json!(3));
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("invalid message_id"),
        "unexpected invalid-argument message: {body}"
    );
}

/// A message with an unresolved Avalanche ICM destination: `dst_chain_id` is
/// NULL and `protocol_metadata` carries the diagnostics `ProtocolMetadata`
/// would produce. Read API must render exactly the five documented `extra`
/// keys, keep `destination_chain` absent, and still report
/// `has_unindexed_chain = true` (a NULL destination is unindexed by
/// definition).
#[tokio::test]
#[ignore = "Needs database to run"]
async fn message_details_unresolved_destination_renders_extra_and_omits_destination_chain() {
    let db = helpers::init_db(
        "test",
        "message_details_unresolved_destination_renders_extra_and_omits_destination_chain",
    )
    .await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;
    let conn = db.client();

    crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
        id: Set(UNRESOLVED_DESTINATION_MESSAGE_ID),
        bridge_id: Set(1),
        status: Set(MessageStatus::Initiated),
        init_timestamp: Set(Utc::now().naive_utc()),
        src_chain_id: Set(1),
        dst_chain_id: Set(None),
        protocol_metadata: Set(Some(serde_json::json!({
            "unresolved_destination": {
                "reason": "unknown_identifier",
                "protocol": "avalanche_icm",
                "blockchain_id": "0xaa",
                "blockchain_id_cb58": "cb58-placeholder",
                "network": "mainnet",
            }
        }))),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();

    let route = format!("/api/v1/interchain/messages/{UNRESOLVED_DESTINATION_MESSAGE_HEX}");
    let details: serde_json::Value = test_server::send_get_request(&base, &route).await;

    assert!(
        details["destination_chain"].is_null(),
        "unresolved destination must not synthesize a destination_chain; got {details}"
    );
    assert_eq!(details["has_unindexed_chain"], serde_json::json!(true));

    let extra = details["extra"]
        .as_object()
        .expect("extra must be an object");
    let mut keys: Vec<&str> = extra.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "unresolved_destination.blockchain_id",
            "unresolved_destination.blockchain_id_cb58",
            "unresolved_destination.network",
            "unresolved_destination.protocol",
            "unresolved_destination.reason",
        ],
        "extra must contain exactly the five documented keys, got {details}"
    );
    assert_eq!(
        extra["unresolved_destination.reason"],
        serde_json::json!("unknown_identifier")
    );
    assert_eq!(
        extra["unresolved_destination.protocol"],
        serde_json::json!("avalanche_icm")
    );
    assert_eq!(
        extra["unresolved_destination.network"],
        serde_json::json!("mainnet")
    );
}

/// A normal, fully-resolved message must have an empty `extra` map — no
/// resolved-marker, no leaked internal namespace.
#[tokio::test]
#[ignore = "Needs database to run"]
async fn message_details_resolved_message_has_empty_extra() {
    let db = helpers::init_db("test", "message_details_resolved_message_has_empty_extra").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;
    let conn = db.client();

    crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
        id: Set(RESOLVED_MESSAGE_ID),
        bridge_id: Set(1),
        status: Set(MessageStatus::Initiated),
        init_timestamp: Set(Utc::now().naive_utc()),
        src_chain_id: Set(1),
        dst_chain_id: Set(Some(100)),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();

    let route = format!("/api/v1/interchain/messages/{RESOLVED_MESSAGE_HEX}");
    let details: serde_json::Value = test_server::send_get_request(&base, &route).await;

    let is_empty = details["extra"]
        .as_object()
        .map(|m| m.is_empty())
        .unwrap_or(true);
    assert!(
        is_empty,
        "a resolved message must have empty extra; got {details}"
    );
}
