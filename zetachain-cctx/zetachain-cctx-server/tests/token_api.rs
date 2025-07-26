use std::sync::Arc;
mod helpers;

use blockscout_service_launcher::test_server;
use pretty_assertions::assert_eq;
use sea_orm::{ActiveValue, EntityTrait};

use zetachain_cctx_entity::{token, sea_orm_active_enums::CoinType};
use zetachain_cctx_logic::{client::{Client, RpcSettings}, database::ZetachainCctxDatabase};
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::TokenInfoResponse
;

use sea_orm::{PaginatorTrait,QueryFilter, ColumnTrait};

use uuid::Uuid;

#[tokio::test]
async fn test_token_api_get_token_info() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        helpers::init_tests_logs().await;
    }
    let db = crate::helpers::init_db("test", "token_api_get_token_info").await;
    let db_conn = db.client();

    let asset = "0xaf88d065e77c8cC2239327C5EDb3A432268e5831";

    // Insert a test token manually
    let test_token = token::ActiveModel {
        id: ActiveValue::NotSet,
        zrc20_contract_address: ActiveValue::Set("0x0327f0660525b15Cdb8f1f5FBF0dD7Cd5Ba182aD".to_string()),
        asset: ActiveValue::Set(asset.to_string()),
        foreign_chain_id: ActiveValue::Set("42161".to_string()),
        decimals: ActiveValue::Set(6),
        name: ActiveValue::Set("ZetaChain ZRC20 USDC on Arbitrum One".to_string()),
        symbol: ActiveValue::Set("USDC.ARB".to_string()),
        coin_type: ActiveValue::Set(CoinType::Erc20),
        gas_limit: ActiveValue::Set("100000".to_string()),
        paused: ActiveValue::Set(false),
        liquidity_cap: ActiveValue::Set("750000000000".to_string()),
        created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
    };

    token::Entity::insert(test_token)
        .exec(db_conn.as_ref())
        .await
        .unwrap();
    let db_url = db.db_url();

    // Create the token service
    let server = helpers::init_zetachain_cctx_server(
        db_url,
        |mut x| {
            x.tracing.enabled = false;
            x.websocket.enabled = false;
            x
        },
        db.client(),
        Arc::new(Client::new(RpcSettings::default())),
    ).await;

    // Test successful token retrieval
    

    let token_info: TokenInfoResponse = test_server::send_get_request(&server, &format!("/api/v1/TokenInfo:get?asset={}", asset)).await;
    

    // Verify the response matches the inserted token
    assert_eq!(token_info.foreign_chain_id, "42161");
    assert_eq!(token_info.decimals, 6);
    assert_eq!(token_info.name, "ZetaChain ZRC20 USDC on Arbitrum One");
    assert_eq!(token_info.symbol, "USDC.ARB");

}

#[tokio::test]
async fn test_token_database_sync_and_query() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        helpers::init_tests_logs().await;
    }
    let db = crate::helpers::init_db("test", "token_database_sync_and_query").await;
    let db_conn = db.client();

    // Create database instance
    let database = Arc::new(ZetachainCctxDatabase::new(db_conn.clone()));

    // Create test tokens using the models
    let tokens = vec![
        zetachain_cctx_logic::models::Token {
            zrc20_contract_address: "0x1111111111111111111111111111111111111111".to_string(),
            asset: "0xasset1".to_string(),
            foreign_chain_id: "1".to_string(),
            decimals: 18,
            name: "Test Token 1".to_string(),
            symbol: "TEST1".to_string(),
            coin_type: "ERC20".to_string(),
            gas_limit: "100000".to_string(),
            paused: false,
            liquidity_cap: "1000000000".to_string(),
        },
        zetachain_cctx_logic::models::Token {
            zrc20_contract_address: "0x2222222222222222222222222222222222222222".to_string(),
            asset: "0xasset2".to_string(),
            foreign_chain_id: "42161".to_string(),
            decimals: 6,
            name: "Test Token 2".to_string(),
            symbol: "TEST2".to_string(),
            coin_type: "ERC20".to_string(),
            gas_limit: "80000".to_string(),
            paused: true,
            liquidity_cap: "2000000000".to_string(),
        },
    ];

    // Test sync_tokens method
    let job_id = Uuid::new_v4();
    database.sync_tokens(job_id, tokens).await.unwrap();

    // Verify tokens were inserted
    let token_count = token::Entity::find()
        .count(db_conn.as_ref())
        .await
        .unwrap();
    assert_eq!(token_count, 2);

    // Test get_token_by_asset method
    let token_info1 = database
        .get_token_by_asset("0xasset1")
        .await
        .unwrap()
        .expect("Token 1 should exist");

    assert_eq!(token_info1.foreign_chain_id, "1");
    assert_eq!(token_info1.decimals, 18);
    assert_eq!(token_info1.name, "Test Token 1");
    assert_eq!(token_info1.symbol, "TEST1");

    let token_info2 = database
        .get_token_by_asset("0xasset2")
        .await
        .unwrap()
        .expect("Token 2 should exist");

    assert_eq!(token_info2.foreign_chain_id, "42161");
    assert_eq!(token_info2.decimals, 6);
    assert_eq!(token_info2.name, "Test Token 2");
    assert_eq!(token_info2.symbol, "TEST2");

    // Test non-existent token
    let token_info3 = database.get_token_by_asset("0xnonexistent").await.unwrap();
    assert!(token_info3.is_none());

    // Test token update (sync the same tokens again with different data)
    let updated_tokens = vec![
        zetachain_cctx_logic::models::Token {
            zrc20_contract_address: "0x1111111111111111111111111111111111111111".to_string(),
            asset: "0xasset1".to_string(),
            foreign_chain_id: "1".to_string(),
            decimals: 18,
            name: "Updated Test Token 1".to_string(), // Changed name
            symbol: "UTEST1".to_string(), // Changed symbol
            coin_type: "ERC20".to_string(),
            gas_limit: "150000".to_string(), // Changed gas limit
            paused: true, // Changed paused status
            liquidity_cap: "1500000000".to_string(), // Changed liquidity cap
        },
    ];

    database.sync_tokens(Uuid::new_v4(), updated_tokens).await.unwrap();

    // Verify token count is still 2 (no new tokens added)
    let token_count = token::Entity::find()
        .count(db_conn.as_ref())
        .await
        .unwrap();
    assert_eq!(token_count, 2);

    // Verify the token was updated
    let updated_token_info = database
        .get_token_by_asset("0xasset1")
        .await
        .unwrap()
        .expect("Updated token should exist");

    assert_eq!(updated_token_info.name, "Updated Test Token 1");
    assert_eq!(updated_token_info.symbol, "UTEST1");

    // Verify the actual database record was updated
    let db_token = token::Entity::find()
        .filter(token::Column::Asset.eq("0xasset1"))
        .one(db_conn.as_ref())
        .await
        .unwrap()
        .expect("Token should exist in database");

    assert_eq!(db_token.name, "Updated Test Token 1");
    assert_eq!(db_token.symbol, "UTEST1");
    assert_eq!(db_token.gas_limit, "150000");
    assert_eq!(db_token.paused, true);
    assert_eq!(db_token.liquidity_cap, "1500000000");
} 