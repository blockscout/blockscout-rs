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
