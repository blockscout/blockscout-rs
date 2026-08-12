// SPDX-License-Identifier: LicenseRef-Blockscout

//! HTTP contract tests for `GET /api/v1/status/indexing`.
//!
//! `helpers::init_interchain_indexer_server` boots the full `run()` from
//! `config/omnibridge/{chains,bridges}.json`: bridge 1 declares contracts on
//! chains `{1, 100}` (`amb_proxy` `started_at_block` 20812229 / 36145833).
//! Indexer startup needs `get_block_number()`, which fails in this harness
//! (no reachable RPC), so "indexer failed to start" is the default state
//! these tests exercise -- exactly the config-only case
//! `enumerate_indexing_targets` exists to make visible instead of silently
//! omitting.

mod helpers;

use blockscout_service_launcher::test_server;
use interchain_indexer_entity::{bridge_contracts, chains, indexer_checkpoints};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter,
};

#[tokio::test]
#[ignore = "Needs database to run"]
async fn get_indexing_progress_no_filter_returns_one_item_per_configured_chain() {
    let db = helpers::init_db("test", "get_indexing_progress_no_filter").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let resp: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/status/indexing").await;
    let items = resp["items"].as_array().unwrap();

    let pairs: Vec<(i64, i64)> = items
        .iter()
        .map(|item| {
            (
                // `bridge_id` is `int32`: a JSON number. `chain_id` is
                // `int64`: like every other 64-bit proto field in this API
                // (see the swagger spec), it serializes as a JSON string to
                // avoid JS float precision loss.
                item["bridge_id"].as_i64().unwrap(),
                item["chain_id"].as_str().unwrap().parse::<i64>().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        pairs,
        vec![(1, 1), (1, 100)],
        "one item per configured chain, not per contract (each chain declares two: amb_proxy + omnibridge_mediator)"
    );
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn get_indexing_progress_filters_work_and_empty_match_returns_empty_list() {
    let db = helpers::init_db("test", "get_indexing_progress_filters").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let bridge_filtered: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/status/indexing?bridge_id=1").await;
    assert_eq!(bridge_filtered["items"].as_array().unwrap().len(), 2);

    let chain_filtered: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/status/indexing?chain_id=1").await;
    let items = chain_filtered["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["chain_id"], serde_json::json!("1"));

    let empty: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/status/indexing?bridge_id=999").await;
    let items = empty["items"]
        .as_array()
        .expect("items must be present (as []), not omitted, even when empty");
    assert!(
        items.is_empty(),
        "a filter matching nothing must return an empty list, not an error"
    );
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn get_indexing_progress_pair_with_no_checkpoint_reports_zero_and_absent_updated_at() {
    let db = helpers::init_db("test", "get_indexing_progress_no_checkpoint").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let resp: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/status/indexing").await;
    let items = resp["items"].as_array().unwrap();
    assert!(!items.is_empty());

    for item in items {
        assert_eq!(item["catchup_progress_percent"], serde_json::json!(0.0));
        assert_eq!(item["catchup_scan_complete"], serde_json::json!(false));
        // `Value` indexing yields `Null` for a missing key too, so presence
        // must be asserted separately from the value — omission is exactly
        // what this test exists to catch.
        let object = item.as_object().expect("each item must be a JSON object");
        assert!(
            object.contains_key("checkpoint_updated_at"),
            "absent checkpoint state must serialize as an explicit null, not be omitted: {item:?}"
        );
        assert!(
            object["checkpoint_updated_at"].is_null(),
            "a pair with no checkpoint row must report checkpoint_updated_at as null: {item:?}"
        );
    }
}

/// Direct regression test for the known gotcha ("`bridge_contracts` Is Only A
/// Diagnostic Proxy For Runtime Membership"): `start_block` must come from
/// the in-memory bridges config, never from the (possibly stale)
/// `bridge_contracts` table.
#[tokio::test]
#[ignore = "Needs database to run"]
async fn get_indexing_progress_start_block_ignores_stale_bridge_contracts_row() {
    let db = helpers::init_db("test", "get_indexing_progress_stale_start_block").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let conn = db.client();
    // Update the existing row rather than inserting -- `UNIQUE(bridge_id,
    // chain_id, address, version)` would reject a duplicate.
    let amb_proxy_row = bridge_contracts::Entity::find()
        .filter(bridge_contracts::Column::BridgeId.eq(1))
        .filter(bridge_contracts::Column::ChainId.eq(1))
        .filter(bridge_contracts::Column::Kind.eq("amb_proxy"))
        .one(conn.as_ref())
        .await
        .unwrap()
        .expect("amb_proxy bridge_contracts row for chain 1 must exist after boot");

    let mut active: bridge_contracts::ActiveModel = amb_proxy_row.into();
    active.started_at_block = Set(Some(1));
    active.update(conn.as_ref()).await.unwrap();

    let resp: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/status/indexing").await;
    let items = resp["items"].as_array().unwrap();
    let chain_1 = items
        .iter()
        .find(|item| item["chain_id"] == serde_json::json!("1"))
        .expect("chain 1 must be reported");
    assert_eq!(
        chain_1["start_block"],
        serde_json::json!("20812229"),
        "start_block must come from config, unaffected by the stale bridge_contracts row"
    );
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn get_indexing_progress_checkpoint_for_pair_absent_from_config_is_not_reported() {
    let db = helpers::init_db("test", "get_indexing_progress_absent_pair_not_reported").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let conn = db.client();
    // A chain not present in config/omnibridge/chains.json at all, so a
    // checkpoint row for it is unambiguously "absent from config" rather
    // than merely a chain bridge 1 doesn't cover.
    chains::Entity::insert(chains::ActiveModel {
        id: Set(999),
        name: Set("Unconfigured".to_string()),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();
    indexer_checkpoints::Entity::insert(indexer_checkpoints::ActiveModel {
        bridge_id: Set(1),
        chain_id: Set(999),
        catchup_min_cursor: Set(0),
        catchup_max_cursor: Set(0),
        finality_cursor: Set(0),
        realtime_cursor: Set(0),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    })
    .exec(conn.as_ref())
    .await
    .unwrap();

    let resp: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/status/indexing").await;
    let items = resp["items"].as_array().unwrap();
    assert!(
        !items
            .iter()
            .any(|item| item["chain_id"] == serde_json::json!("999")),
        "a checkpoint row for a (bridge, chain) pair absent from config must not be reported"
    );
}
