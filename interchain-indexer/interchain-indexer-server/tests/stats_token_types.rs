// SPDX-License-Identifier: LicenseRef-Blockscout

mod helpers;

use blockscout_service_launcher::test_server;
use chrono::Utc;
use interchain_indexer_entity::{
    crosschain_messages, crosschain_transfers,
    sea_orm_active_enums::{MessageStatus, TokenType, TransferAssetLinkage},
    stats_asset_tokens, tokens,
};
use interchain_indexer_logic::{InterchainDatabase, stats::IndexedChains};
use sea_orm::{ActiveValue::Set, EntityTrait, prelude::BigDecimal};

#[tokio::test]
#[ignore = "needs database"]
async fn stats_bridged_tokens_serialize_native_and_erc20_types_without_metadata() {
    assert_projected_native_token_contract(false).await;
}

#[tokio::test]
#[ignore = "needs database"]
async fn stats_bridged_tokens_preserve_seeded_native_type_through_projection() {
    assert_projected_native_token_contract(true).await;
}

async fn assert_projected_native_token_contract(seed_native: bool) {
    let db = helpers::init_db("test", &format!("stats_token_types_{seed_native}")).await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |mut settings| {
        settings.stats.chains_recalculation_period_secs = 0;
        settings
    })
    .await;
    let conn = db.client();
    let database = InterchainDatabase::new(conn.clone());
    if seed_native {
        database.upsert_token_info(native_metadata()).await.unwrap();
    }
    crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
        id: Set(9001),
        bridge_id: Set(1),
        status: Set(MessageStatus::Completed),
        init_timestamp: Set(Utc::now().naive_utc()),
        src_chain_id: Set(1),
        dst_chain_id: Set(Some(100)),
        src_tx_hash: Set(Some(vec![0xab; 32])),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();
    let transfer = crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
        message_id: Set(9001),
        bridge_id: Set(1),
        index: Set(0),
        token_src_chain_id: Set(1),
        token_dst_chain_id: Set(100),
        token_src_address: Set(Some(vec![0x11; 20])),
        token_dst_address: Set(Some(vec![0; 20])),
        src_amount: Set(Some(BigDecimal::from(10))),
        dst_amount: Set(Some(BigDecimal::from(10))),
        asset_linkage: Set(Some(TransferAssetLinkage::Conversion)),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();
    database
        .backfill_stats_until_idle(&IndexedChains::AllIndexed)
        .await
        .unwrap();
    let transfer = crosschain_transfers::Entity::find_by_id(transfer.last_insert_id)
        .one(conn.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(transfer.stats_processed, 1);
    let native_asset_id = transfer.dst_stats_asset_id.unwrap();
    assert_ne!(transfer.src_stats_asset_id, transfer.dst_stats_asset_id);

    let native_link = stats_asset_tokens::Entity::find_by_id((native_asset_id, 100))
        .one(conn.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(native_link.r#type, TokenType::Native);

    for (chain_id, expected_type, expected_address) in [
        (100, "NATIVE", serde_json::Value::Null),
        (
            1,
            "ERC20",
            serde_json::json!(format!("0x{}", "11".repeat(20))),
        ),
    ] {
        let body: serde_json::Value = test_server::send_get_request(
            &base,
            &format!("/api/v1/stats/chain/{chain_id}/bridged-tokens"),
        )
        .await;
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        let token = &items[0]["tokens"][0];
        assert_eq!(token["type"], expected_type);
        assert_eq!(token.get("token_address"), Some(&expected_address));
        if chain_id == 100 && seed_native {
            assert_eq!(token["name"], "xDai");
        } else {
            assert!(token["name"].is_null());
        }
    }

    // A metadata refresh also repairs a stale type in the stats projection.
    stats_asset_tokens::Entity::update(stats_asset_tokens::ActiveModel {
        stats_asset_id: sea_orm::ActiveValue::Unchanged(native_asset_id),
        chain_id: sea_orm::ActiveValue::Unchanged(100),
        r#type: Set(TokenType::Erc20),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();
    database.upsert_token_info(native_metadata()).await.unwrap();
    let native = tokens::Entity::find_by_id((100, vec![0; 20]))
        .one(conn.as_ref())
        .await
        .unwrap()
        .unwrap();
    database
        .propagate_token_info_to_stats_tables(100, &[0; 20], &native)
        .await
        .unwrap();
    let native_link = stats_asset_tokens::Entity::find_by_id((native_asset_id, 100))
        .one(conn.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(native_link.r#type, TokenType::Native);
    let body: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/stats/chain/100/bridged-tokens").await;
    let token = &body["items"][0]["tokens"][0];
    assert_eq!(token["type"], "NATIVE");
    assert_eq!(token.get("token_address"), Some(&serde_json::Value::Null));
    assert_eq!(token["name"], "xDai");
    assert_eq!(token["symbol"], "xDAI");
    assert_eq!(body["items"][0]["name"], "xDai");
}

fn native_metadata() -> tokens::ActiveModel {
    tokens::ActiveModel {
        chain_id: Set(100),
        address: Set(vec![0; 20]),
        r#type: Set(TokenType::Native),
        name: Set(Some("xDai".to_string())),
        symbol: Set(Some("xDAI".to_string())),
        decimals: Set(Some(18)),
        ..Default::default()
    }
}
