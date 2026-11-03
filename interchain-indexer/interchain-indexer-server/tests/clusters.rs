// SPDX-License-Identifier: LicenseRef-Blockscout

//! DB-backed HTTP contract test for the cluster-scoped Read API under
//! `/api/v1/clusters/{cluster_id}`.
//!
//! Pins what only a running server can: each cluster route equals the legacy
//! route called with the cluster's four filter fields (the legacy route is the
//! oracle), chain directories and per-chain stats follow universe semantics,
//! message details are scoped inside the lookup, an unknown cluster is a 404
//! checked before any other validation the handler does (a param the HTTP
//! layer cannot parse into its declared type is rejected with 400 earlier), and
//! the cluster fields sent as query params are ignored.
//!
//! One server, one database, deliberately: every check is read-only against
//! the same seed, and each extra server adds to the test-server port race
//! (`get_free_port()` drops its listener before `run()` binds the port).
//!
//! `helpers::init_interchain_indexer_server` boots from
//! `config/omnibridge/bridges.json` (bridge 1 on chains `{1, 100}`) and
//! `tests/fixtures/chains-offline.json` (chains `{1, 100}`). Bridge 2 is
//! DB-only (absent from the config), so its rows stay visible through the
//! permissive "removed bridge" rule.

mod helpers;

use blockscout_service_launcher::test_server;
use chrono::{DateTime, NaiveDateTime};
use interchain_indexer_entity::{
    bridges, chains, crosschain_messages, crosschain_transfers,
    sea_orm_active_enums::{MessageStatus, TransferAssetLinkage},
    stats_chains,
};
use interchain_indexer_logic::{InterchainDatabase, stats::IndexedChains};
use pretty_assertions::assert_eq;
use reqwest::{StatusCode, Url};
use sea_orm::{ActiveValue::Set, DatabaseConnection, EntityTrait, prelude::BigDecimal};
use serde_json::{Value, json};
use std::{collections::BTreeSet, io::Write, sync::Arc};

/// A chain no configured bridge indexes.
const UNINDEXED_CHAIN_ID: i64 = 999;

const TX_HASH: &str = "0xabababababababababababababababababababababababababababababababab";
const ADDRESS: &str = "0xcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";

const CLUSTERS: &str = r#"[
  { "id": "gnosis", "home_chain_id": 100, "bridge_ids": [1] },
  { "id": "pair", "counterparty_chain_ids": [1, 100] },
  { "id": "wide", "counterparty_chain_ids": [1, 100], "include_unindexed_chains": true },
  { "id": "all" }
]"#;

/// The six message/transfer list routes (suffix after `/api/v1/interchain/`
/// or `/api/v1/clusters/{id}/interchain/`).
fn list_suffixes() -> [String; 6] {
    [
        "messages".to_string(),
        "transfers".to_string(),
        format!("messages:byTx/{TX_HASH}"),
        format!("transfers:byTx/{TX_HASH}"),
        format!("messages:byAddress/{ADDRESS}"),
        format!("transfers:byAddress/{ADDRESS}"),
    ]
}

fn ts(secs: i64) -> NaiveDateTime {
    DateTime::from_timestamp(secs, 0)
        .expect("valid timestamp")
        .naive_utc()
}

async fn get(base: &Url, route: &str) -> Value {
    test_server::send_get_request(base, route).await
}

fn ids(body: &Value, id_field: &str) -> Vec<String> {
    body["items"]
        .as_array()
        .unwrap_or_else(|| panic!("no items: {body}"))
        .iter()
        .map(|item| {
            item[id_field]
                .as_str()
                .unwrap_or_else(|| panic!("missing {id_field}: {item}"))
                .to_string()
        })
        .collect()
}

fn id_set(body: &Value, id_field: &str) -> BTreeSet<String> {
    ids(body, id_field).into_iter().collect()
}

fn hex_id(message_id: i64) -> String {
    format!("{message_id:#x}")
}

/// Seeds the messages, their transfers, the stats projection and the stats
/// directory. Message rows: `(id, bridge, src -> dst)`.
async fn seed(conn: &Arc<DatabaseConnection>) {
    bridges::Entity::insert(bridges::ActiveModel {
        id: Set(2),
        name: Set("DbOnlyBridge".to_string()),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();
    chains::Entity::insert(chains::ActiveModel {
        id: Set(UNINDEXED_CHAIN_ID),
        name: Set("Unindexed".to_string()),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();

    let rows: [(i64, i32, i64, i64); 6] = [
        (8101, 1, 1, 100),
        (8102, 1, 100, 1),
        (8103, 2, 1, 100),
        (8104, 1, 1, UNINDEXED_CHAIN_ID),
        (8105, 1, 1, 100),
        // Same public id as the row above, under another bridge.
        (8105, 2, 1, 100),
    ];
    for (index, (message_id, bridge_id, src_chain, dst_chain)) in rows.into_iter().enumerate() {
        let index = index as u8;
        crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
            id: Set(message_id),
            bridge_id: Set(bridge_id),
            status: Set(MessageStatus::Completed),
            // Distinct per row, an hour apart.
            init_timestamp: Set(ts(1_767_313_800 + i64::from(index) * 3_600)),
            src_chain_id: Set(src_chain),
            dst_chain_id: Set(Some(dst_chain)),
            src_tx_hash: Set(Some(vec![0xab; 32])),
            sender_address: Set(Some(vec![0xcd; 20])),
            ..Default::default()
        })
        .exec(conn.as_ref())
        .await
        .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            message_id: Set(message_id),
            bridge_id: Set(bridge_id),
            index: Set(0),
            token_src_chain_id: Set(src_chain),
            token_dst_chain_id: Set(dst_chain),
            token_src_address: Set(Some(vec![0x10 + index; 20])),
            token_dst_address: Set(Some(vec![0x20 + index; 20])),
            src_amount: Set(Some(BigDecimal::from(10 + i32::from(index)))),
            dst_amount: Set(Some(BigDecimal::from(10 + i32::from(index)))),
            asset_linkage: Set(Some(TransferAssetLinkage::Conversion)),
            sender_address: Set(Some(vec![0xcd; 20])),
            ..Default::default()
        })
        .exec(conn.as_ref())
        .await
        .unwrap();
    }

    InterchainDatabase::new(conn.clone())
        .backfill_stats_until_idle(&IndexedChains::AllIndexed)
        .await
        .unwrap();
    for (chain_id, users) in [(1_i64, 5_i64), (100, 99), (UNINDEXED_CHAIN_ID, 7)] {
        stats_chains::Entity::insert(stats_chains::ActiveModel {
            chain_id: Set(chain_id),
            unique_transfer_users_count: Set(users),
            unique_message_users_count: Set(0),
            ..Default::default()
        })
        .exec(conn.as_ref())
        .await
        .unwrap();
    }
}

/// The seed must be able to tell a cluster route from its legacy twin, or the
/// parity checks below would be trivially true.
async fn check_self_validation(base: &Url) {
    let legacy_messages = get(base, "/api/v1/interchain/messages").await;
    assert!(
        id_set(&legacy_messages, "message_id").contains(&hex_id(8103)),
        "the bare legacy list must contain the bridge-2 message: {legacy_messages}"
    );
    let gnosis_messages = get(base, "/api/v1/clusters/gnosis/interchain/messages").await;
    assert!(
        !id_set(&gnosis_messages, "message_id").contains(&hex_id(8103)),
        "the gnosis cluster (bridge 1 only) must not contain it: {gnosis_messages}"
    );
    assert!(
        !ids(&gnosis_messages, "message_id").is_empty(),
        "the gnosis list must not be vacuously empty: {gnosis_messages}"
    );

    let legacy_bridges = get(base, "/api/v1/interchain/bridges").await;
    assert_eq!(legacy_bridges["items"].as_array().unwrap().len(), 2);

    let legacy_chains = get(
        base,
        "/api/v1/interchain/chains?include_unindexed_chains=true",
    )
    .await;
    assert!(
        ids(&legacy_chains, "id").contains(&UNINDEXED_CHAIN_ID.to_string()),
        "the legacy opt-in directory must contain the unindexed chain: {legacy_chains}"
    );
}

async fn check_list_parity(base: &Url) {
    let gnosis_params = "home_chain_id=100&bridge_ids=1&include_unindexed_chains=false";
    for suffix in list_suffixes() {
        let cluster = get(
            base,
            &format!("/api/v1/clusters/gnosis/interchain/{suffix}"),
        )
        .await;
        let legacy = get(
            base,
            &format!("/api/v1/interchain/{suffix}?{gnosis_params}"),
        )
        .await;
        assert!(
            !cluster["items"].as_array().unwrap().is_empty(),
            "{suffix}: the gnosis list must not be vacuously empty: {cluster}"
        );
        assert_eq!(cluster, legacy, "gnosis parity of {suffix}");

        // A client direction filter narrows by AND on both sides.
        let cluster = get(
            base,
            &format!("/api/v1/clusters/gnosis/interchain/{suffix}?src_chain_ids=100"),
        )
        .await;
        let legacy = get(
            base,
            &format!("/api/v1/interchain/{suffix}?{gnosis_params}&src_chain_ids=100"),
        )
        .await;
        assert_eq!(
            cluster, legacy,
            "gnosis parity of {suffix} with src_chain_ids"
        );

        // The unrestricted cluster is the bare legacy route.
        let cluster = get(base, &format!("/api/v1/clusters/all/interchain/{suffix}")).await;
        let legacy = get(base, &format!("/api/v1/interchain/{suffix}")).await;
        assert_eq!(cluster, legacy, "`all` parity of {suffix}");
    }
}

async fn check_stats_parity(base: &Url) {
    let cluster = get(
        base,
        "/api/v1/clusters/gnosis/stats/chain/1/messages-paths/sent",
    )
    .await;
    let legacy = get(
        base,
        "/api/v1/stats/chain/1/messages-paths/sent?counterparty_chain_ids=100&bridge_ids=1",
    )
    .await;
    assert!(
        !cluster["items"].as_array().unwrap().is_empty(),
        "the sent paths must not be vacuously empty: {cluster}"
    );
    assert_eq!(cluster, legacy, "gnosis parity of sent paths");

    let cluster = get(
        base,
        "/api/v1/clusters/gnosis/stats/chain/100/messages-paths/received",
    )
    .await;
    let legacy = get(
        base,
        "/api/v1/stats/chain/100/messages-paths/received?bridge_ids=1",
    )
    .await;
    assert!(
        !cluster["items"].as_array().unwrap().is_empty(),
        "the received paths must not be vacuously empty: {cluster}"
    );
    assert_eq!(cluster, legacy, "gnosis parity of received paths");

    let cluster = get(base, "/api/v1/clusters/gnosis/stats/chain/1/bridged-tokens").await;
    let legacy = get(
        base,
        "/api/v1/stats/chain/1/bridged-tokens?counterparty_chain_ids=100&bridge_ids=1",
    )
    .await;
    assert!(
        !cluster["items"].as_array().unwrap().is_empty(),
        "the bridged tokens must not be vacuously empty: {cluster}"
    );
    assert_eq!(cluster, legacy, "gnosis parity of bridged tokens");
}

async fn check_chain_directories(base: &Url) {
    let expected: BTreeSet<String> = ["1", "100"].into_iter().map(String::from).collect();

    let wide = get(base, "/api/v1/clusters/wide/interchain/chains").await;
    assert_eq!(id_set(&wide, "id"), expected);

    let wide_unindexed = get(
        base,
        "/api/v1/clusters/wide/interchain/chains?chain_ids=999",
    )
    .await;
    assert_eq!(wide_unindexed["items"], json!([]));
    let legacy_unindexed = get(
        base,
        "/api/v1/interchain/chains?chain_ids=999&include_unindexed_chains=true",
    )
    .await;
    assert_eq!(ids(&legacy_unindexed, "id"), vec!["999".to_string()]);

    let pair_one = get(base, "/api/v1/clusters/pair/interchain/chains?chain_ids=1").await;
    assert_eq!(ids(&pair_one, "id"), vec!["1".to_string()]);

    let wide_stats = get(base, "/api/v1/clusters/wide/stats/chains").await;
    assert_eq!(id_set(&wide_stats, "id"), expected);
    let legacy_stats = get(base, "/api/v1/stats/chains?include_unindexed_chains=true").await;
    assert!(
        ids(&legacy_stats, "id").contains(&UNINDEXED_CHAIN_ID.to_string()),
        "the legacy opt-in stats directory must contain 999: {legacy_stats}"
    );

    let pair_stats = get(base, "/api/v1/clusters/pair/stats/chains?chain_ids=999").await;
    assert_eq!(pair_stats["items"], json!([]));
    assert!(pair_stats["next_page_params"].is_null(), "{pair_stats}");
    assert!(pair_stats["prev_page_params"].is_null(), "{pair_stats}");
}

async fn check_path_chain_outside_the_cluster(base: &Url) {
    for suffix in [
        "bridged-tokens",
        "messages-paths/sent",
        "messages-paths/received",
    ] {
        let route = format!("/api/v1/clusters/pair/stats/chain/999/{suffix}");
        let (status, body) = helpers::get_raw(base, &route).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{route}: {body}");
        // tonic InvalidArgument == code 3.
        assert_eq!(body["code"], json!(3), "{route}: {body}");
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .contains("chain_id 999 is not in cluster pair"),
            "{route}: unexpected message: {body}"
        );
    }
}

async fn check_bridges(base: &Url) {
    let legacy = get(base, "/api/v1/interchain/bridges").await;
    let legacy_bridge_one = legacy["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|bridge| bridge["id"] == json!(1))
        .unwrap_or_else(|| panic!("legacy bridges must contain bridge 1: {legacy}"))
        .clone();

    let gnosis = get(base, "/api/v1/clusters/gnosis/interchain/bridges").await;
    let items = gnosis["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{gnosis}");
    assert_eq!(items[0]["id"], json!(1));
    assert_eq!(items[0], legacy_bridge_one);

    let all = get(base, "/api/v1/clusters/all/interchain/bridges").await;
    assert_eq!(all, legacy);
}

async fn check_details(base: &Url) {
    let details = |cluster: &str, message_id: i64, query: &str| {
        let route = format!(
            "/api/v1/clusters/{cluster}/interchain/messages/{}{query}",
            hex_id(message_id)
        );
        async move { helpers::get_raw(base, &route).await }
    };
    let legacy_details = |message_id: i64| {
        let route = format!("/api/v1/interchain/messages/{}", hex_id(message_id));
        async move { helpers::get_raw(base, &route).await }
    };

    // In the cluster: equal to the legacy lookup.
    let (status, cluster_body) = details("gnosis", 8101, "").await;
    assert_eq!(status, StatusCode::OK, "{cluster_body}");
    let (status, legacy_body) = legacy_details(8101).await;
    assert_eq!(status, StatusCode::OK, "{legacy_body}");
    assert_eq!(cluster_body, legacy_body);

    // Outside the cluster (bridge 2): the same 404 as a missing message.
    let (status, body) = details("gnosis", 8103, "").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], json!(5), "{body}");
    assert_eq!(body["message"], json!("Message not found"), "{body}");

    // An unindexed chain is hidden on the cluster route, but legacy details
    // are unrestricted.
    let (status, body) = details("all", 8104, "").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, body) = legacy_details(8104).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // The id shared with a bridge outside the cluster resolves unqualified.
    let (status, body) = details("gnosis", 8105, "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["bridge"]["id"], json!(1), "{body}");
    // Legacy and a cluster holding both bridges stay ambiguous: FailedPrecondition.
    let (status, body) = legacy_details(8105).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], json!(9), "{body}");
    let (status, body) = details("pair", 8105, "").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], json!(9), "{body}");

    // A bridge qualifier outside the cluster finds nothing.
    let (status, body) = details("gnosis", 8105, "?bridge_id=2").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

async fn check_unknown_cluster(base: &Url) {
    let details = hex_id(8101);
    let routes = [
        "interchain/chains".to_string(),
        "interchain/bridges".to_string(),
        "interchain/messages".to_string(),
        format!("interchain/messages/{details}"),
        format!("interchain/messages:byTx/{TX_HASH}"),
        format!("interchain/messages:byAddress/{ADDRESS}"),
        "interchain/transfers".to_string(),
        format!("interchain/transfers:byTx/{TX_HASH}"),
        format!("interchain/transfers:byAddress/{ADDRESS}"),
        "stats/chains".to_string(),
        "stats/chain/1/bridged-tokens".to_string(),
        "stats/chain/1/messages-paths/sent".to_string(),
        "stats/chain/1/messages-paths/received".to_string(),
        // The handler resolves the cluster before parsing these string params.
        "interchain/messages?page_token=not*a*token".to_string(),
        "interchain/messages:byTx/not-hex".to_string(),
    ];
    for suffix in routes {
        let route = format!("/api/v1/clusters/nope/{suffix}");
        let (status, body) = helpers::get_raw(base, &route).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{route}: {body}");
        // tonic NotFound == code 5.
        assert_eq!(body["code"], json!(5), "{route}: {body}");
        assert_eq!(
            body["message"],
            json!("cluster not found: nope"),
            "{route}: {body}"
        );
    }
}

async fn check_foreign_params_are_ignored(base: &Url) {
    let with_foreign = get(
        base,
        "/api/v1/clusters/gnosis/interchain/messages?home_chain_id=1&counterparty_chain_ids=999\
         &bridge_ids=999&include_unindexed_chains=true",
    )
    .await;
    let plain = get(base, "/api/v1/clusters/gnosis/interchain/messages").await;
    assert_eq!(with_foreign, plain);
}

async fn check_paging(base: &Url) {
    for (route, id_field) in [
        ("/api/v1/clusters/gnosis/interchain/messages", "message_id"),
        ("/api/v1/clusters/all/stats/chains", "id"),
    ] {
        let page1 = get(base, &format!("{route}?page_size=1")).await;
        let page1_ids = ids(&page1, id_field);
        assert_eq!(page1_ids.len(), 1, "{route} page 1: {page1}");
        let token = page1["next_page_params"]["page_token"]
            .as_str()
            .unwrap_or_else(|| panic!("{route}: no next page token: {page1}"));

        let page2 = get(base, &format!("{route}?page_size=1&page_token={token}")).await;
        let page2_ids = ids(&page2, id_field);
        assert!(!page2_ids.is_empty(), "{route} page 2 is empty: {page2}");
        assert_ne!(page1_ids, page2_ids, "{route}: page 2 must move forward");
    }
}

async fn check_serde_defaults(base: &Url) {
    // `sort` and `order` are non-optional enum query fields: omitting them
    // must not fail to deserialize.
    for route in [
        "/api/v1/clusters/all/stats/chains",
        "/api/v1/clusters/gnosis/stats/chain/1/bridged-tokens",
    ] {
        let (status, body) = helpers::get_raw(base, route).await;
        assert_eq!(status, StatusCode::OK, "{route}: {body}");
    }
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn cluster_routes_apply_the_cluster_scope() {
    let db = helpers::init_db("test", "cluster_routes_apply_the_cluster_scope").await;

    // Kept alive for the whole test: the server reads it once at startup.
    let mut clusters_file = tempfile::NamedTempFile::new().unwrap();
    clusters_file.write_all(CLUSTERS.as_bytes()).unwrap();
    clusters_file.flush().unwrap();
    let clusters_path = clusters_file.path().to_path_buf();

    let base = helpers::init_interchain_indexer_server(db.db_url(), |mut s| {
        // The periodic recomputation would race the seeded `stats_chains` rows.
        s.stats.chains_recalculation_period_secs = 0;
        s.clusters_config = Some(clusters_path.clone());
        s
    })
    .await;

    seed(&db.client()).await;

    check_self_validation(&base).await;
    check_list_parity(&base).await;
    check_stats_parity(&base).await;
    check_chain_directories(&base).await;
    check_path_chain_outside_the_cluster(&base).await;
    check_bridges(&base).await;
    check_details(&base).await;
    check_unknown_cluster(&base).await;
    check_foreign_params_are_ignored(&base).await;
    check_paging(&base).await;
    check_serde_defaults(&base).await;

    drop(clusters_file);
}
