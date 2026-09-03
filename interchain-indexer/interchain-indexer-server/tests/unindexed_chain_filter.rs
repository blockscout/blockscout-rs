// SPDX-License-Identifier: LicenseRef-Blockscout

//! DB-backed HTTP contract tests for the read-side unindexed-chain default-hide
//! behavior on endpoints that need the running server (bridge config wiring and
//! `IndexedChains` injection), rather than the bare SeaORM/`ChainBridgeFilter`
//! layer already covered in `interchain-indexer-logic/src/database.rs`.
//!
//! `helpers::init_interchain_indexer_server` boots the server from
//! `config/omnibridge/bridges.json` and `tests/fixtures/chains-offline.json`
//! (same chains, unreachable endpoints): bridge 1 has contracts on chains
//! `{1, 100}`, so `IndexedChains` is `PerBridge({1: {1, 100}})`.

mod helpers;

use blockscout_service_launcher::test_server;
use chrono::Utc;
use interchain_indexer_entity::{chains, crosschain_messages, sea_orm_active_enums::MessageStatus};
use sea_orm::{ActiveValue::Set, EntityTrait};

/// Public numeric message ID for the seeded NULL-destination message.
/// `8888 == 0x22b8`.
const HIDDEN_MESSAGE_ID: i64 = 8888;
const HIDDEN_MESSAGE_HEX: &str = "0x22b8";

/// Public numeric message ID for the seeded fully-indexed message.
/// `7777 == 0x1e61`.
const INDEXED_MESSAGE_ID: i64 = 7777;
const INDEXED_MESSAGE_HEX: &str = "0x1e61";

/// A chain outside `config/omnibridge/chains.json` (`{1, 100}`) and outside
/// bridge 1's contract set, so it is not in `IndexedChains::configured_union()`.
const UNINDEXED_CHAIN_ID: i64 = 999;

#[tokio::test]
#[ignore = "Needs database to run"]
async fn get_message_details_returns_hidden_row_with_flag() {
    let db = helpers::init_db("test", "get_message_details_returns_hidden_row_with_flag").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let conn = db.client();
    crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
        id: Set(HIDDEN_MESSAGE_ID),
        bridge_id: Set(1),
        status: Set(MessageStatus::Initiated),
        init_timestamp: Set(Utc::now().naive_utc()),
        src_chain_id: Set(1),
        dst_chain_id: Set(None),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();

    // Both chains are in bridge 1's configured set, so this row is fully
    // indexed and must report the flag explicitly as `false`.
    crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
        id: Set(INDEXED_MESSAGE_ID),
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

    // `GetMessageDetails` bypasses the default-hide filter entirely and still
    // sets the flag.
    let route = format!("/api/v1/interchain/messages/{HIDDEN_MESSAGE_HEX}");
    let details: serde_json::Value = test_server::send_get_request(&base, &route).await;
    assert_eq!(details["has_unindexed_chain"], serde_json::json!(true));

    // The negative case must be an explicit `false`, not an omitted key: the
    // field is non-optional in the proto and carries no `skip_serializing_if`.
    let indexed_route = format!("/api/v1/interchain/messages/{INDEXED_MESSAGE_HEX}");
    let indexed_details: serde_json::Value =
        test_server::send_get_request(&base, &indexed_route).await;
    assert_eq!(
        indexed_details["has_unindexed_chain"],
        serde_json::json!(false),
        "a fully-indexed message must carry has_unindexed_chain=false, not omit it; got {indexed_details}"
    );

    // The same row is excluded from the default list view.
    let list: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/interchain/messages").await;
    let ids: Vec<&str> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["message_id"].as_str().unwrap())
        .collect();
    assert!(
        !ids.contains(&HIDDEN_MESSAGE_HEX),
        "NULL-dst message must be hidden from the default list; got {ids:?}"
    );

    // The opt-in list includes it, still flagged.
    let opt_in: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/interchain/messages?include_unindexed_chains=true",
    )
    .await;
    let hidden_item = opt_in["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["message_id"] == serde_json::json!(HIDDEN_MESSAGE_HEX))
        .expect("opt-in list must include the NULL-dst message");
    assert_eq!(hidden_item["has_unindexed_chain"], serde_json::json!(true));
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn get_chains_default_omits_chain_no_bridge_indexes() {
    let db = helpers::init_db("test", "get_chains_default_omits_chain_no_bridge_indexes").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let conn = db.client();
    chains::Entity::insert(chains::ActiveModel {
        id: Set(UNINDEXED_CHAIN_ID),
        name: Set("Unindexed".to_string()),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();

    let default_resp: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/interchain/chains").await;
    let default_ids: Vec<&str> = default_resp["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(
        !default_ids.contains(&UNINDEXED_CHAIN_ID.to_string().as_str()),
        "default chain directory must omit a chain no configured bridge indexes; got {default_ids:?}"
    );
    // Chain 1 is covered by bridge 1's contracts, so it stays visible.
    assert!(default_ids.contains(&"1"));

    let opt_in: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/interchain/chains?include_unindexed_chains=true",
    )
    .await;
    let opt_in_ids: Vec<&str> = opt_in["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(
        opt_in_ids.contains(&UNINDEXED_CHAIN_ID.to_string().as_str()),
        "opt-in chain directory must include the unindexed chain; got {opt_in_ids:?}"
    );
}

/// `coding-task-2b` removes the temporary `InvalidArgument` rejections
/// `coding-task-2a` installed on the three raw-SQL stats endpoints and wires
/// `include_unindexed_chains` to the real `IndexedChains`-derived restriction.
/// None of the four routes below reject `include_unindexed_chains=true` any
/// more, and `/stats/chains` now honors the default-hide / opt-in contract the
/// same way `GetChains` already does.
#[tokio::test]
#[ignore = "Needs database to run"]
async fn stats_endpoints_reject_nothing_after_2b() {
    let db = helpers::init_db("test", "stats_endpoints_reject_nothing_after_2b").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let cases = [
        "/api/v1/stats/chains?include_unindexed_chains=true",
        "/api/v1/stats/chain/1/bridged-tokens?include_unindexed_chains=true",
        "/api/v1/stats/chain/1/messages-paths/sent?include_unindexed_chains=true",
        "/api/v1/stats/chain/1/messages-paths/received?include_unindexed_chains=true",
    ];
    for route in cases {
        let (status, body) = helpers::get_raw(&base, route).await;
        assert_eq!(status, reqwest::StatusCode::OK, "route {route}: {body}");
    }

    // Absent / false keeps working too.
    let (status, _) = helpers::get_raw(&base, "/api/v1/stats/chains").await;
    assert_eq!(status, reqwest::StatusCode::OK);
}

/// `/stats/chains` gains the same default-hide + opt-in contract as `GetChains`
/// (`coding-task-2b` item 1/2), both derived from `IndexedChains::configured_union()`
/// so the two directory views cannot drift apart.
#[tokio::test]
#[ignore = "Needs database to run"]
async fn stats_chains_default_omits_chain_no_bridge_indexes_and_agrees_with_get_chains() {
    let db = helpers::init_db(
        "test",
        "stats_chains_default_omits_chain_no_bridge_indexes_and_agrees_with_get_chains",
    )
    .await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let conn = db.client();
    chains::Entity::insert(chains::ActiveModel {
        id: Set(UNINDEXED_CHAIN_ID),
        name: Set("Unindexed".to_string()),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();

    let ids_from = |body: &serde_json::Value| -> Vec<String> {
        body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap().to_string())
            .collect()
    };

    let stats_default: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/stats/chains").await;
    let stats_default_ids = ids_from(&stats_default);
    assert!(
        !stats_default_ids.contains(&UNINDEXED_CHAIN_ID.to_string()),
        "default /stats/chains must omit a chain no configured bridge indexes; got {stats_default_ids:?}"
    );
    assert!(stats_default_ids.contains(&"1".to_string()));

    let stats_opt_in: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/stats/chains?include_unindexed_chains=true")
            .await;
    let stats_opt_in_ids = ids_from(&stats_opt_in);
    assert!(
        stats_opt_in_ids.contains(&UNINDEXED_CHAIN_ID.to_string()),
        "opt-in /stats/chains must include the unindexed chain; got {stats_opt_in_ids:?}"
    );

    // Both directory views are keyed by chain alone and derive their
    // restriction from the same `configured_union()`, so their default id
    // sets must agree exactly.
    let get_chains_default: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/interchain/chains").await;
    let mut get_chains_ids = ids_from(&get_chains_default);
    let mut stats_ids_sorted = stats_default_ids.clone();
    get_chains_ids.sort();
    stats_ids_sorted.sort();
    assert_eq!(
        get_chains_ids, stats_ids_sorted,
        "/stats/chains and GetChains must agree on the default chain set"
    );
}
