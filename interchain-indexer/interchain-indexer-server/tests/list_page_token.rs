// SPDX-License-Identifier: LicenseRef-Blockscout

//! DB-backed HTTP contract test for token-only pagination on every list
//! family: messages, transfers, bridged-tokens and stats-chains.
//!
//! Pins what only a running server can: `next_page_params` /
//! `prev_page_params` carry exactly one key, `page_token`; following that
//! token reaches the rest of the list; a malformed `page_token` is a client
//! error (400 `InvalidArgument`), never a 500.
//!
//! One server, one database, deliberately: every check is read-only against
//! the same seed, and each extra server adds to the test-server port race
//! (`get_free_port()` drops its listener before `run()` binds the port).
//!
//! `helpers::init_interchain_indexer_server` boots from
//! `config/omnibridge/bridges.json` (bridge 1 on chains `{1, 100}`) and
//! `tests/fixtures/chains-offline.json` (chains `{1, 100}`).

mod helpers;

use blockscout_service_launcher::test_server;
use chrono::{DateTime, NaiveDateTime};
use interchain_indexer_entity::{
    crosschain_messages, crosschain_transfers,
    sea_orm_active_enums::{MessageStatus, TransferAssetLinkage},
    stats_chains,
};
use interchain_indexer_logic::{InterchainDatabase, stats::IndexedChains};
use reqwest::StatusCode;
use sea_orm::{ActiveValue::Set, EntityTrait, prelude::BigDecimal};
use std::collections::BTreeSet;

fn ts(secs: i64) -> NaiveDateTime {
    DateTime::from_timestamp(secs, 0)
        .expect("valid timestamp")
        .naive_utc()
}

/// Asserts `params` is an object whose only key is a non-empty `page_token`
/// string and returns the token.
fn token_only(params: &serde_json::Value, context: &str) -> String {
    let obj = params
        .as_object()
        .unwrap_or_else(|| panic!("{context}: expected an object, got {params}"));
    assert_eq!(
        obj.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["page_token"],
        "{context}: {params}"
    );
    let token = obj["page_token"]
        .as_str()
        .unwrap_or_else(|| panic!("{context}: page_token is not a string: {params}"));
    assert!(!token.is_empty(), "{context}: empty page_token");
    token.to_string()
}

fn ids(body: &serde_json::Value, id_field: &str) -> Vec<String> {
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

#[tokio::test]
#[ignore = "Needs database to run"]
async fn list_endpoints_paginate_by_page_token_only() {
    let db = helpers::init_db("test", "list_endpoints_paginate_by_page_token_only").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |mut s| {
        // The periodic recomputation would race the seeded `stats_chains` rows.
        s.stats.chains_recalculation_period_secs = 0;
        s
    })
    .await;
    let conn = db.client();

    // Two completed bridge-1 messages 1 -> 100, an hour apart, one transfer
    // each, with distinct token pairs so projection yields two assets on chain 1.
    for (message_id, init_secs, src_token, dst_token) in [
        (7101_i64, 1_767_313_800_i64, [0x11_u8; 20], [0x22_u8; 20]),
        (7102, 1_767_317_400, [0x33; 20], [0x44; 20]),
    ] {
        crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
            id: Set(message_id),
            bridge_id: Set(1),
            status: Set(MessageStatus::Completed),
            init_timestamp: Set(ts(init_secs)),
            src_chain_id: Set(1),
            dst_chain_id: Set(Some(100)),
            src_tx_hash: Set(Some(vec![0xab; 32])),
            ..Default::default()
        })
        .exec(conn.as_ref())
        .await
        .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            message_id: Set(message_id),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            token_src_address: Set(Some(src_token.to_vec())),
            token_dst_address: Set(Some(dst_token.to_vec())),
            src_amount: Set(Some(BigDecimal::from(10))),
            dst_amount: Set(Some(BigDecimal::from(10))),
            asset_linkage: Set(Some(TransferAssetLinkage::Conversion)),
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
    for (chain_id, users) in [(1_i64, 5_i64), (100, 99)] {
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

    // (route, field that identifies an item)
    let families = [
        ("/api/v1/interchain/messages", "message_id"),
        ("/api/v1/interchain/transfers", "message_id"),
        ("/api/v1/stats/chain/1/bridged-tokens", "stats_asset_id"),
        ("/api/v1/stats/chains", "id"),
    ];

    for (route, id_field) in families {
        let all: serde_json::Value = test_server::send_get_request(&base, route).await;
        let all_ids: BTreeSet<String> = ids(&all, id_field).into_iter().collect();
        assert_eq!(
            all_ids.len(),
            2,
            "{route}: the seed must yield two items: {all}"
        );

        let page1: serde_json::Value =
            test_server::send_get_request(&base, &format!("{route}?page_size=1")).await;
        let page1_ids = ids(&page1, id_field);
        assert_eq!(page1_ids.len(), 1, "{route} page 1: {page1}");
        let token = token_only(&page1["next_page_params"], &format!("{route} page 1 next"));

        let page2: serde_json::Value = test_server::send_get_request(
            &base,
            &format!("{route}?page_size=1&page_token={token}"),
        )
        .await;
        let page2_ids = ids(&page2, id_field);
        assert_eq!(page2_ids.len(), 1, "{route} page 2: {page2}");
        assert_ne!(page1_ids, page2_ids, "{route}: page 2 must move forward");
        let seen: BTreeSet<String> = page1_ids.into_iter().chain(page2_ids).collect();
        assert_eq!(seen, all_ids, "{route}: two pages must cover both items");

        for (page, body) in [("page 1", &page1), ("page 2", &page2)] {
            for side in ["next_page_params", "prev_page_params"] {
                if !body[side].is_null() {
                    token_only(&body[side], &format!("{route} {page} {side}"));
                }
            }
        }

        // `*` is outside the base64url alphabet.
        let (status, body) =
            helpers::get_raw(&base, &format!("{route}?page_token=not*a*token")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{route}: {body}");
        // tonic InvalidArgument == code 3.
        assert_eq!(body["code"], serde_json::json!(3), "{route}: {body}");
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .contains("invalid page_token"),
            "{route}: unexpected message: {body}"
        );
    }
}
