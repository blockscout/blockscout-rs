// SPDX-License-Identifier: LicenseRef-Blockscout

//! DB-backed HTTP contract tests for the `chain_ids` and `bridge_ids` filters
//! on `GET /api/v1/interchain/chains`.
//!
//! `helpers::init_interchain_indexer_server` boots the server from
//! `config/omnibridge/bridges.json` and `tests/fixtures/chains-offline.json`
//! (same chains, unreachable endpoints): bridge 1 has contracts on chains
//! `{1, 100}`, so `IndexedChains` is `PerBridge({1: {1, 100}})`.

mod helpers;

use blockscout_service_launcher::test_server;
use interchain_indexer_entity::{bridges, chains};
use reqwest::StatusCode;
use sea_orm::{ActiveValue::Set, EntityTrait};

/// A chain outside `config/omnibridge/chains.json` (`{1, 100}`) and outside
/// bridge 1's contract set, so no configured bridge indexes it.
const UNINDEXED_CHAIN_ID: i64 = 999;

/// A bridge id present in the `bridges` table but absent from
/// `config/omnibridge/bridges.json` — a bridge removed from config after
/// `upsert_bridges` already wrote its row (upserts never delete).
const REMOVED_BRIDGE_ID: i32 = 999;

fn chain_ids_of(body: &serde_json::Value) -> Vec<String> {
    let mut ids: Vec<String> = body["items"]
        .as_array()
        .expect("items must be present, even when empty")
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    ids
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn absent_blank_and_whitespace_filters_match_the_unfiltered_baseline() {
    let db = helpers::init_db("test", "chains_filters_absent_blank").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let baseline: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/interchain/chains").await;
    assert!(
        !chain_ids_of(&baseline).is_empty(),
        "the baseline must be non-empty or the equality assertions below prove nothing"
    );

    for route in [
        "/api/v1/interchain/chains?chain_ids=",
        "/api/v1/interchain/chains?bridge_ids=",
        "/api/v1/interchain/chains?chain_ids=&bridge_ids=",
        "/api/v1/interchain/chains?chain_ids=%20%20&bridge_ids=%20",
    ] {
        let resp: serde_json::Value = test_server::send_get_request(&base, route).await;
        assert_eq!(resp, baseline, "route {route} must not restrict anything");
    }
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn chain_ids_selects_matching_rows_and_is_idempotent() {
    let db = helpers::init_db("test", "chains_filters_chain_ids").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let single: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/interchain/chains?chain_ids=1").await;
    assert_eq!(chain_ids_of(&single), vec!["1".to_string()]);

    let duplicated: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/interchain/chains?chain_ids=1,1").await;
    assert_eq!(
        duplicated, single,
        "a duplicated chain id must be idempotent"
    );

    let unknown: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/interchain/chains?chain_ids=999999").await;
    assert!(
        chain_ids_of(&unknown).is_empty(),
        "an unknown chain id must match nothing, not everything"
    );
}

/// The one assertion that pins "same source of truth": `?bridge_ids=1` must
/// agree with bridge 1's `indexed_chain_ids` as reported by
/// `GET /api/v1/interchain/bridges`, fetched live in this same test rather
/// than hardcoded.
#[tokio::test]
#[ignore = "Needs database to run"]
async fn bridge_ids_agrees_with_the_bridges_endpoint_indexed_chain_ids() {
    let db = helpers::init_db("test", "chains_filters_bridge_ids_agree").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let bridges_resp: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/interchain/bridges").await;
    let bridge_1 = bridges_resp["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == serde_json::json!(1))
        .expect("bridge 1 (from config) must be present");
    let mut expected: Vec<String> = bridge_1["indexed_chain_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    expected.sort();
    assert!(
        !expected.is_empty(),
        "bridge 1 must index at least one chain or this test proves nothing"
    );

    let filtered: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/interchain/chains?bridge_ids=1").await;
    assert_eq!(
        chain_ids_of(&filtered),
        expected,
        "?bridge_ids=1 must return exactly bridge 1's indexed_chain_ids. \
         The two agree because every chain bridge 1 indexes also has a row in \
         the chain directory; the endpoint intersects the configured union with \
         that directory, so a bridge chain missing from `chains.json` would be \
         listed by /bridges and absent here"
    );

    let duplicated: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/interchain/chains?bridge_ids=1,1").await;
    assert_eq!(
        duplicated, filtered,
        "a duplicated bridge id must be idempotent"
    );
}

/// The `Some([])` trap: `selected_configured_union` returns a bare `Vec` where
/// empty means "no candidates", the opposite of `configured_union()`. Gating on
/// its emptiness instead of on the parsed request value would turn this into
/// "return the whole directory".
#[tokio::test]
#[ignore = "Needs database to run"]
async fn unknown_or_unconfigured_bridge_ids_return_zero_chains() {
    let db = helpers::init_db("test", "chains_filters_bridge_ids_empty").await;
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

    for route in [
        // Never present anywhere.
        "/api/v1/interchain/chains?bridge_ids=42",
        // Present in the `bridges` table, absent from config.
        "/api/v1/interchain/chains?bridge_ids=999",
    ] {
        let resp: serde_json::Value = test_server::send_get_request(&base, route).await;
        assert!(
            chain_ids_of(&resp).is_empty(),
            "route {route} must return an empty list, not the full directory; got {resp}"
        );
    }
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn chain_ids_and_bridge_ids_intersect() {
    let db = helpers::init_db("test", "chains_filters_intersection").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let both: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/interchain/chains?chain_ids=100&bridge_ids=1",
    )
    .await;
    assert_eq!(chain_ids_of(&both), vec!["100".to_string()]);

    let disjoint: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/interchain/chains?chain_ids=999&bridge_ids=1",
    )
    .await;
    assert!(
        chain_ids_of(&disjoint).is_empty(),
        "the two filters compose through AND, so a disjoint pair matches nothing"
    );
}

/// `include_unindexed_chains` is a global gate ("no configured bridge indexes
/// this chain"), never "not indexed by the selected bridges" — so it must not
/// widen a `bridge_ids` scope.
#[tokio::test]
#[ignore = "Needs database to run"]
async fn include_unindexed_chains_does_not_widen_the_bridge_scope() {
    let db = helpers::init_db("test", "chains_filters_unindexed_gate").await;
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

    // The gate on its own does surface the chain.
    let opt_in: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/interchain/chains?include_unindexed_chains=true",
    )
    .await;
    assert!(
        chain_ids_of(&opt_in).contains(&UNINDEXED_CHAIN_ID.to_string()),
        "the opt-in gate must surface a chain no configured bridge indexes"
    );

    let scoped: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/interchain/chains?bridge_ids=1&include_unindexed_chains=true",
    )
    .await;
    assert!(
        !chain_ids_of(&scoped).contains(&UNINDEXED_CHAIN_ID.to_string()),
        "the global gate must not widen the bridge scope; got {scoped}"
    );
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn malformed_and_out_of_range_filters_are_rejected() {
    let db = helpers::init_db("test", "chains_filters_invalid_input").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    let (status, body) = helpers::get_raw(&base, "/api/v1/interchain/chains?chain_ids=abc").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert!(
        body.to_string().contains("chain_ids"),
        "the failure message must carry the parameter label; got {body}"
    );

    for route in [
        "/api/v1/interchain/chains?bridge_ids=abc",
        // Above `i32::MAX`.
        "/api/v1/interchain/chains?bridge_ids=3000000000",
    ] {
        let (status, body) = helpers::get_raw(&base, route).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "route {route}: {body}");
    }
}
