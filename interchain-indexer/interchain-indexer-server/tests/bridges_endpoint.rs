// SPDX-License-Identifier: LicenseRef-Blockscout

//! HTTP contract tests for `GET /api/v1/interchain/bridges`'s
//! `indexed_chain_ids` field, which requires the running server (bridge
//! config wiring and `IndexedChains` injection), rather than the bare
//! conversion already covered by `interchain-indexer-server/src/services/bridge_proto.rs`'s
//! unit tests.
//!
//! `helpers::init_interchain_indexer_server` boots the server from
//! `config/omnibridge/{chains,bridges}.json`: bridge 1 has contracts on
//! chains `{1, 100}`, so `IndexedChains` is `PerBridge({1: {1, 100}})`.

mod helpers;

use blockscout_service_launcher::test_server;
use interchain_indexer_entity::bridges;
use sea_orm::{ActiveValue::Set, EntityTrait};

/// A bridge id present in the `bridges` table but absent from
/// `config/omnibridge/bridges.json` — simulates a bridge removed from
/// config after `upsert_bridges` already wrote its row (upserts never
/// delete).
const REMOVED_BRIDGE_ID: i32 = 999;

#[tokio::test]
#[ignore = "Needs database to run"]
async fn get_bridges_reports_configured_chains_sorted_and_empty_for_removed_bridge() {
    let db = helpers::init_db(
        "test",
        "get_bridges_reports_configured_chains_sorted_and_empty_for_removed_bridge",
    )
    .await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let conn = db.client();
    bridges::Entity::insert(bridges::ActiveModel {
        id: Set(REMOVED_BRIDGE_ID),
        name: Set("Removed Bridge".to_string()),
        enabled: Set(true),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();

    let resp: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/interchain/bridges").await;
    let items = resp["items"].as_array().unwrap();

    let bridge_1 = items
        .iter()
        .find(|b| b["id"] == serde_json::json!(1))
        .expect("bridge 1 (from config) must be present");
    let bridge_1_chains: Vec<i64> = bridge_1["indexed_chain_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(
        bridge_1_chains,
        vec![1, 100],
        "bridge 1 must report its configured contract chains, sorted ascending; got {bridge_1_chains:?}"
    );

    let removed = items
        .iter()
        .find(|b| b["id"] == serde_json::json!(REMOVED_BRIDGE_ID))
        .expect("bridge removed from config but still in the bridges table must still be listed");
    let removed_chains = removed["indexed_chain_ids"]
        .as_array()
        .expect("indexed_chain_ids must be present even when empty");
    assert!(
        removed_chains.is_empty(),
        "a bridge absent from the bridges config must report an empty chain list, not stale \
         bridge_contracts history; got {removed_chains:?}"
    );
}
