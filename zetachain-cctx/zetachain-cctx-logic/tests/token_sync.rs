use std::{sync::Arc, time::Duration};
mod helpers;

use actix_phoenix_channel::ChannelCentral;
use pretty_assertions::assert_eq;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::json;
use wiremock::{
    matchers::{method, path, path_regex, query_param},
    Mock, MockServer, ResponseTemplate,
};

use zetachain_cctx_entity::{
    sea_orm_active_enums::{Kind, ProcessingStatus},
    token, watermark,
};
use zetachain_cctx_logic::{
    channel::Channel,
    client::{Client, RpcSettings},
    database::ZetachainCctxDatabase,
    indexer::Indexer,
    models::{CctxStatusStatus, Token},
    settings::IndexerSettings,
};

fn dummy_token_response(tokens: &[Token], next_key: Option<&str>) -> serde_json::Value {
    let foreign_coins: Vec<serde_json::Value> = tokens
        .iter()
        .map(|token| {
            json!({
                "zrc20_contract_address": token.zrc20_contract_address,
                "asset": token.asset,
                "foreign_chain_id": token.foreign_chain_id,
                "decimals": token.decimals,
                "name": token.name,
                "symbol": token.symbol,
                "coin_type": token.coin_type,
                "gas_limit": token.gas_limit,
                "paused": token.paused,
                "liquidity_cap": token.liquidity_cap
            })
        })
        .collect();

    let pagination = match next_key {
        Some(key) => json!({
            "next_key": key,
            "total": "0"
        }),
        None => json!({
            "next_key": null,
            "total": "0"
        }),
    };

    json!({
        "foreignCoins": foreign_coins,
        "pagination": pagination
    })
}

#[tokio::test]
async fn test_token_sync_stream_works() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        helpers::init_tests_logs().await;
    }
    let db = crate::helpers::init_db("test", "token_sync_stream_works").await;

    // Setup mock server
    let mock_server = MockServer::start().await;

    // Mock empty responses for CCTX endpoints to suppress other streams
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .respond_with(ResponseTemplate::new(200).set_body_json(helpers::empty_cctx_response()))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"/zeta-chain/crosschain/cctx/.+"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(helpers::dummy_cross_chain_tx(
                "dummy_index",
                CctxStatusStatus::OutboundMined,
            )),
        )
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(
            r"/zeta-chain/crosschain/inboundHashToCctxData/.+",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(
            {"CrossChainTxs": []}
        )))
        .mount(&mock_server)
        .await;

    // Mock token response with test data
    Mock::given(method("GET"))
        .and(path("/zeta-chain/fungible/foreign_coins"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(dummy_token_response(
                &[
                    Token {
                        zrc20_contract_address: "0x0327f0660525b15Cdb8f1f5FBF0dD7Cd5Ba182aD"
                            .to_string(),
                        asset: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831".to_string(),
                        foreign_chain_id: "42161".to_string(),
                        decimals: 6,
                        name: "ZetaChain ZRC20 USDC on Arbitrum One".to_string(),
                        symbol: "USDC.ARB".to_string(),
                        coin_type: zetachain_cctx_logic::models::CoinType::ERC20,
                        gas_limit: "100000".to_string(),
                        paused: false,
                        liquidity_cap: "750000000000".to_string(),
                        icon_url: None,
                    },
                    Token {
                        zrc20_contract_address: "0x1234567890123456789012345678901234567890"
                            .to_string(),
                        asset: "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599".to_string(),
                        foreign_chain_id: "1".to_string(),
                        decimals: 8,
                        name: "ZetaChain ZRC20 WBTC on Ethereum Mainnet".to_string(),
                        symbol: "WBTC.ETH".to_string(),
                        coin_type: zetachain_cctx_logic::models::CoinType::ERC20,
                        gas_limit: "80000".to_string(),
                        paused: false,
                        liquidity_cap: "100000000000".to_string(),
                        icon_url: None,
                    },
                ],
                None, // No next page
            )),
        )
        .mount(&mock_server)
        .await;

    // Create client pointing to mock server
    let client = Client::new(RpcSettings {
        url: mock_server.uri().to_string(),
        request_per_second: 100,
        ..Default::default()
    });

    let db_conn = db.client();

    // Initialize database with historical watermark to prevent historical sync
    let watermark_model = watermark::ActiveModel {
        id: ActiveValue::NotSet,
        kind: ActiveValue::Set(Kind::Historical),
        pointer: ActiveValue::Set("COMPLETED".to_string()),
        processing_status: ActiveValue::Set(ProcessingStatus::Done),
        created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_by: ActiveValue::Set("test".to_string()),
        ..Default::default()
    };

    watermark::Entity::insert(watermark_model)
        .exec(db_conn.as_ref())
        .await
        .unwrap();

    // Create indexer with fast token polling
    let channel = Arc::new(ChannelCentral::new(Channel));

    let indexer = Indexer::new(
        IndexerSettings {
            polling_interval: 100,
            token_polling_interval: 50, // Very fast for testing
            token_batch_size: 100,
            concurrency: 1,
            enabled: true,
            ..Default::default()
        },
        Arc::new(client),
        Arc::new(ZetachainCctxDatabase::new(db_conn.clone(), 7001)),
        Arc::new(channel.channel_broadcaster()),
    );

    // Run indexer for a short time to process token data
    let indexer_handle = tokio::spawn(async move {
        let timeout_duration = Duration::from_millis(500);
        tokio::time::timeout(timeout_duration, async {
            let _ = indexer.run().await;
        })
        .await
        .unwrap_or_else(|_| {
            tracing::info!("Timeout occurred");
        });
    });

    // Wait for indexer to process
    tokio::time::sleep(Duration::from_millis(600)).await;
    indexer_handle.abort();

    // Verify tokens were inserted
    let token_count = token::Entity::find()
        .filter(token::Column::Asset.ne("UNKNOWN"))
        .count(db_conn.as_ref())
        .await
        .unwrap();

    assert_eq!(token_count, 2, "Expected 2 tokens to be inserted");

    // Verify specific token data
    let usdc_token = token::Entity::find()
        .filter(
            token::Column::Zrc20ContractAddress.eq("0x0327f0660525b15Cdb8f1f5FBF0dD7Cd5Ba182aD"),
        )
        .one(db_conn.as_ref())
        .await
        .unwrap()
        .expect("USDC token should exist");

    assert_eq!(
        usdc_token.asset,
        "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"
    );
    assert_eq!(usdc_token.foreign_chain_id, 42161);
    assert_eq!(usdc_token.decimals, 6);
    assert_eq!(usdc_token.name, "ZetaChain ZRC20 USDC on Arbitrum One");
    assert_eq!(usdc_token.symbol, "USDC.ARB");
    assert_eq!(usdc_token.gas_limit, "100000");
    assert_eq!(usdc_token.paused, false);
    assert_eq!(usdc_token.liquidity_cap, "750000000000");

    let wbtc_token = token::Entity::find()
        .filter(
            token::Column::Zrc20ContractAddress.eq("0x1234567890123456789012345678901234567890"),
        )
        .one(db_conn.as_ref())
        .await
        .unwrap()
        .expect("WBTC token should exist");

    assert_eq!(
        wbtc_token.asset,
        "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599"
    );
    assert_eq!(wbtc_token.foreign_chain_id, 1);
    assert_eq!(wbtc_token.decimals, 8);
    assert_eq!(wbtc_token.name, "ZetaChain ZRC20 WBTC on Ethereum Mainnet");
    assert_eq!(wbtc_token.symbol, "WBTC.ETH");
    assert_eq!(wbtc_token.gas_limit, "80000");
    assert_eq!(wbtc_token.paused, false);
    assert_eq!(wbtc_token.liquidity_cap, "100000000000");
}

#[tokio::test]
async fn test_token_sync_pagination() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        helpers::init_tests_logs().await;
    }
    let db = crate::helpers::init_db("test", "token_sync_pagination").await;

    let database = ZetachainCctxDatabase::new(db.client(), 7001);
    database.setup_db().await.unwrap();

    // Setup mock server
    let mock_server = MockServer::start().await;

    // Mock empty responses for CCTX endpoints to suppress other streams
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .respond_with(ResponseTemplate::new(200).set_body_json(helpers::empty_cctx_response()))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"/crosschain/cctx/.+"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(helpers::dummy_cross_chain_tx(
                "dummy_index",
                CctxStatusStatus::OutboundMined,
            )),
        )
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"/crosschain/inboundHashToCctxData/.+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(
            {"CrossChainTxs": []}
        )))
        .mount(&mock_server)
        .await;

    // Mock first page of tokens (no pagination key in query)

    // Mock second page of tokens (with pagination key)
    Mock::given(method("GET"))
        .and(path("/zeta-chain/fungible/foreign_coins"))
        .and(query_param("pagination.limit", "100"))
        .and(query_param("pagination.key", "page2key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(dummy_token_response(
                &[Token {
                    zrc20_contract_address: "0x3333333333333333333333333333333333333333"
                        .to_string(),
                    asset: "0xpage2asset1".to_string(),
                    foreign_chain_id: "42161".to_string(),
                    decimals: 6,
                    name: "Page 2 Token 1".to_string(),
                    symbol: "P2T1".to_string(),
                    coin_type: zetachain_cctx_logic::models::CoinType::ERC20,
                    gas_limit: "80000".to_string(),
                    paused: false,
                    liquidity_cap: "3000000000".to_string(),
                    icon_url: None,
                }],
                Some("page3key"), // Has next page
            )),
        )
        .mount(&mock_server)
        .await;
    // Mock third page of tokens (final page)
    Mock::given(method("GET"))
        .and(path("/zeta-chain/fungible/foreign_coins"))
        .and(query_param("pagination.limit", "100"))
        .and(query_param("pagination.key", "page3key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(dummy_token_response(
                &[Token {
                    zrc20_contract_address: "0x4444444444444444444444444444444444444444"
                        .to_string(),
                    asset: "0xpage3asset1".to_string(),
                    foreign_chain_id: "137".to_string(),
                    decimals: 8,
                    name: "Page 3 Token 1".to_string(),
                    symbol: "P3T1".to_string(),
                    coin_type: zetachain_cctx_logic::models::CoinType::ERC20,
                    gas_limit: "120000".to_string(),
                    paused: true,
                    liquidity_cap: "4000000000".to_string(),
                    icon_url: None,
                }],
                None, // No next page
            )),
        )
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/zeta-chain/fungible/foreign_coins"))
        .and(query_param("pagination.limit", "100"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(dummy_token_response(
                &[
                    Token {
                        zrc20_contract_address: "0x1111111111111111111111111111111111111111"
                            .to_string(),
                        asset: "0xpage1asset1".to_string(),
                        foreign_chain_id: "1".to_string(),
                        decimals: 18,
                        name: "Page 1 Token 1".to_string(),
                        symbol: "P1T1".to_string(),
                        coin_type: zetachain_cctx_logic::models::CoinType::ERC20,
                        gas_limit: "100000".to_string(),
                        paused: false,
                        liquidity_cap: "1000000000".to_string(),
                        icon_url: None,
                    },
                    Token {
                        zrc20_contract_address: "0x2222222222222222222222222222222222222222"
                            .to_string(),
                        asset: "0xpage1asset2".to_string(),
                        foreign_chain_id: "1".to_string(),
                        decimals: 18,
                        name: "Page 1 Token 2".to_string(),
                        symbol: "P1T2".to_string(),
                        coin_type: zetachain_cctx_logic::models::CoinType::ERC20,
                        gas_limit: "100000".to_string(),
                        paused: false,
                        liquidity_cap: "2000000000".to_string(),
                        icon_url: None,
                    },
                ],
                Some("page2key"), // Has next page
            )),
        )
        .mount(&mock_server)
        .await;

    // Create client pointing to mock server
    let client = Client::new(RpcSettings {
        url: mock_server.uri().to_string(),
        request_per_second: 100,
        ..Default::default()
    });

    let db_conn = db.client();

    let channel = Arc::new(ChannelCentral::new(Channel));

    let indexer = Indexer::new(
        IndexerSettings {
            polling_interval: 100,
            token_polling_interval: 50, // Very fast for testing
            token_batch_size: 100,
            concurrency: 1,
            enabled: true,
            ..Default::default()
        },
        Arc::new(client),
        Arc::new(ZetachainCctxDatabase::new(db_conn.clone(), 7001)),
        Arc::new(channel.channel_broadcaster()),
    );

    // Run indexer for a short time to process token data
    let indexer_handle = tokio::spawn(async move {
        let timeout_duration = Duration::from_millis(800); // Longer timeout for multiple pages
        tokio::time::timeout(timeout_duration, async {
            let _ = indexer.run().await;
        })
        .await
        .unwrap_or_else(|_| {
            tracing::info!("Timeout occurred");
        });
    });

    // Wait for indexer to process all pages
    tokio::time::sleep(Duration::from_millis(900)).await;
    indexer_handle.abort();

    // Verify all tokens from all pages were inserted
    let token_count = token::Entity::find()
        .filter(token::Column::Asset.ne("UNKNOWN"))
        .count(db_conn.as_ref())
        .await
        .unwrap();

    assert_eq!(
        token_count, 4,
        "Expected 4 tokens to be inserted from all pages"
    );

    // Verify tokens from each page
    let page1_token1 = token::Entity::find()
        .filter(
            token::Column::Zrc20ContractAddress.eq("0x1111111111111111111111111111111111111111"),
        )
        .one(db_conn.as_ref())
        .await
        .unwrap();
    assert!(page1_token1.is_some(), "Page 1 Token 1 should exist");

    let page1_token2 = token::Entity::find()
        .filter(
            token::Column::Zrc20ContractAddress.eq("0x2222222222222222222222222222222222222222"),
        )
        .one(db_conn.as_ref())
        .await
        .unwrap();
    assert!(page1_token2.is_some(), "Page 1 Token 2 should exist");

    let page2_token1 = token::Entity::find()
        .filter(
            token::Column::Zrc20ContractAddress.eq("0x3333333333333333333333333333333333333333"),
        )
        .one(db_conn.as_ref())
        .await
        .unwrap();
    assert!(page2_token1.is_some(), "Page 2 Token 1 should exist");

    let page3_token1 = token::Entity::find()
        .filter(
            token::Column::Zrc20ContractAddress.eq("0x4444444444444444444444444444444444444444"),
        )
        .one(db_conn.as_ref())
        .await
        .unwrap();
    assert!(page3_token1.is_some(), "Page 3 Token 1 should exist");

    // Verify specific data from different pages
    let page2_token = page2_token1.unwrap();
    assert_eq!(page2_token.asset, "0xpage2asset1");
    assert_eq!(page2_token.foreign_chain_id, 42161);
    assert_eq!(page2_token.name, "Page 2 Token 1");

    let page3_token = page3_token1.unwrap();
    assert_eq!(page3_token.asset, "0xpage3asset1");
    assert_eq!(page3_token.foreign_chain_id, 137);
    assert_eq!(page3_token.paused, true);
    assert_eq!(page3_token.name, "Page 3 Token 1");
}
