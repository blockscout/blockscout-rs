// SPDX-License-Identifier: LicenseRef-Blockscout

//! DB-backed HTTP contract tests for the `bridge_ids` filter on
//! `GET /api/v1/stats/chains`, whose snapshot semantics are recorded in
//! ADR-009.
//!
//! `helpers::init_interchain_indexer_server` boots the server from
//! `config/omnibridge/bridges.json`: bridge 1 has contracts on chains
//! `{1, 100}`, so `IndexedChains` is `PerBridge({1: {1, 100}})`.

mod helpers;

use blockscout_service_launcher::test_server;
use interchain_indexer_entity::{chains, stats_chains, stats_chains_by_bridge};
use reqwest::StatusCode;
use sea_orm::{ActiveValue::Set, EntityTrait};

async fn seed_global(db: &sea_orm::DatabaseConnection, chain_id: i64, transfer: i64) {
    stats_chains::Entity::insert(stats_chains::ActiveModel {
        chain_id: Set(chain_id),
        unique_transfer_users_count: Set(transfer),
        unique_message_users_count: Set(0),
        ..Default::default()
    })
    .exec(db)
    .await
    .unwrap();
}

async fn seed_by_bridge(
    db: &sea_orm::DatabaseConnection,
    bridge_id: i32,
    chain_id: i64,
    transfer: i64,
) {
    stats_chains_by_bridge::Entity::insert(stats_chains_by_bridge::ActiveModel {
        bridge_id: Set(bridge_id),
        chain_id: Set(chain_id),
        unique_transfer_users_count: Set(transfer),
        unique_message_users_count: Set(0),
        ..Default::default()
    })
    .exec(db)
    .await
    .unwrap();
}

fn count_for(body: &serde_json::Value, chain_id: i64) -> Option<u64> {
    body["items"].as_array().unwrap().iter().find_map(|item| {
        (item["id"].as_str() == Some(&chain_id.to_string())).then(|| {
            item["unique_transfer_users_count"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap()
        })
    })
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn absent_and_blank_bridge_ids_match_the_unfiltered_baseline() {
    let db = helpers::init_db("test", "stats_bridge_filter_absent_blank").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |mut s| {
        // Disable the periodic recomputation worker: it would race the seeded
        // rows below in a background task with no synchronization point.
        s.stats.chains_recalculation_period_secs = 0;
        s
    })
    .await;
    let conn = db.client();
    seed_global(conn.as_ref(), 1, 42).await;
    // A per-bridge row that must never leak into the unfiltered response.
    seed_by_bridge(conn.as_ref(), 1, 1, 999).await;

    let baseline: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/stats/chains").await;
    let blank: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/stats/chains?bridge_ids=").await;

    assert_eq!(baseline, blank);
    assert_eq!(
        count_for(&baseline, 1),
        Some(42),
        "unfiltered path must read the exact global snapshot, never a bridge sum"
    );
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn single_bridge_id_reads_the_exact_bridge_cell_not_the_global_row() {
    let db = helpers::init_db("test", "stats_bridge_filter_single").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |mut s| {
        // Disable the periodic recomputation worker: it would race the seeded
        // rows below in a background task with no synchronization point.
        s.stats.chains_recalculation_period_secs = 0;
        s
    })
    .await;
    let conn = db.client();
    seed_global(conn.as_ref(), 1, 42).await;
    seed_by_bridge(conn.as_ref(), 1, 1, 7).await;

    let filtered: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/stats/chains?bridge_ids=1").await;
    assert_eq!(
        count_for(&filtered, 1),
        Some(7),
        "bridge_ids=1 must read the bridge-1 cell, not the global row"
    );
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn unknown_and_duplicate_bridge_ids_are_accepted_and_contribute_correctly() {
    let db = helpers::init_db("test", "stats_bridge_filter_unknown_dup").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |mut s| {
        // Disable the periodic recomputation worker: it would race the seeded
        // rows below in a background task with no synchronization point.
        s.stats.chains_recalculation_period_secs = 0;
        s
    })
    .await;
    let conn = db.client();
    seed_by_bridge(conn.as_ref(), 1, 1, 7).await;

    // Bridge 42 is unknown (not in config, no history) and duplicated; must
    // not error and must not change the result.
    let (status, body) = helpers::get_raw(&base, "/api/v1/stats/chains?bridge_ids=1,1,42").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(count_for(&body, 1), Some(7));
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn bridge_ids_scope_chain_candidates_even_when_unindexed_chains_are_included() {
    let db = helpers::init_db("test", "stats_bridge_filter_candidate_scope").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |mut s| {
        s.stats.chains_recalculation_period_secs = 0;
        s
    })
    .await;
    let conn = db.client();

    chains::Entity::insert(chains::ActiveModel {
        id: Set(250),
        name: Set("unrelated".to_string()),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();
    seed_global(conn.as_ref(), 250, 99).await;
    seed_by_bridge(conn.as_ref(), 1, 1, 7).await;

    let filtered: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/stats/chains?bridge_ids=1&include_unindexed_chains=true",
    )
    .await;

    assert_eq!(count_for(&filtered, 1), Some(7));
    assert_eq!(
        count_for(&filtered, 250),
        None,
        "include_unindexed_chains must not add chains unrelated to the selected bridges"
    );
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn malformed_and_overflow_bridge_ids_return_bad_request() {
    let db = helpers::init_db("test", "stats_bridge_filter_malformed").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |mut s| {
        // Disable the periodic recomputation worker: it would race the seeded
        // rows below in a background task with no synchronization point.
        s.stats.chains_recalculation_period_secs = 0;
        s
    })
    .await;

    for route in [
        "/api/v1/stats/chains?bridge_ids=abc",
        "/api/v1/stats/chains?bridge_ids=3000000000",
    ] {
        let (status, body) = helpers::get_raw(&base, route).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "route {route}: {body}");
    }
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn use_pagination_token_false_paginates_by_the_filtered_count() {
    let db = helpers::init_db("test", "stats_bridge_filter_raw_pagination").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |mut s| {
        s.api.use_pagination_token = false;
        s.stats.chains_recalculation_period_secs = 0;
        s
    })
    .await;
    let conn = db.client();

    // Global order would be 100 (99) then 1 (5); bridge-1-filtered order
    // must be 1 (50) then 100 (3) — proving the raw pagination marker is
    // derived from the filtered count, not the global one.
    seed_global(conn.as_ref(), 1, 5).await;
    seed_global(conn.as_ref(), 100, 99).await;
    seed_by_bridge(conn.as_ref(), 1, 1, 50).await;
    seed_by_bridge(conn.as_ref(), 1, 100, 3).await;

    let page1: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/stats/chains?bridge_ids=1&page_size=1").await;
    let items1 = page1["items"].as_array().unwrap();
    assert_eq!(items1.len(), 1);
    assert_eq!(items1[0]["id"], serde_json::json!("1"));
    let next = &page1["next_page_params"];
    assert_eq!(next["count"], serde_json::json!("50"));
    assert_eq!(next["chain_id"], serde_json::json!("1"));

    let route2 = format!(
        "/api/v1/stats/chains?bridge_ids=1&page_size=1&direction={}&count={}&chain_id={}",
        next["direction"].as_str().unwrap(),
        next["count"].as_str().unwrap(),
        next["chain_id"].as_str().unwrap(),
    );
    let page2: serde_json::Value = test_server::send_get_request(&base, &route2).await;
    let items2 = page2["items"].as_array().unwrap();
    assert_eq!(items2.len(), 1);
    assert_eq!(items2[0]["id"], serde_json::json!("100"));
}
