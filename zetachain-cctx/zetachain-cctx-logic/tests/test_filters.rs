mod helpers;

use crate::helpers::*;
use blockscout_service_launcher::tracing::init_logs;
use migration::sea_orm::TransactionTrait;
use uuid::Uuid;
use zetachain_cctx_logic::{
    database::ZetachainCctxDatabase,
    models::{CoinType, Filters, Token},
};
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::Direction;

#[tokio::test]
async fn query_cctxs_with_filters() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        init_logs(
            "tests",
            &blockscout_service_launcher::tracing::TracingSettings {
                enabled: true,
                ..Default::default()
            },
            &blockscout_service_launcher::tracing::JaegerSettings::default(),
        )
        .unwrap();
    }

    let db = init_db("test", "query_cctxs_with_filters").await;

    let mut dummy_cctx_1 = dummy_cross_chain_tx("dummy_cctx_1", "PendingInbound");
    dummy_cctx_1.inbound_params.sender = "0x1234567890123456789012345678901234567890".to_string();
    dummy_cctx_1.inbound_params.sender_chain_id = "1".to_string();

    let token_eth = Token {
        name: "dummy_token_1".to_string(),
        symbol: "ETH".to_string(),
        asset: "".to_string(),
        foreign_chain_id: "1".to_string(),
        coin_type: CoinType::Gas,
        decimals: 18,
        gas_limit: "1000000000000000000".to_string(),
        paused: false,
        liquidity_cap: "1000000000000000000".to_string(),
        icon_url: Some("https://example.com/icon.png".to_string()),
        zrc20_contract_address: Uuid::new_v4().to_string(),
    };

    let token_usdc = Token {
        name: "dummy_token_2".to_string(),
        symbol: "USDC".to_string(),
        asset: Uuid::new_v4().to_string(),
        foreign_chain_id: "2".to_string(),
        coin_type: CoinType::ERC20,
        decimals: 18,
        gas_limit: "1000000000000000000".to_string(),
        paused: false,
        liquidity_cap: "1000000000000000000".to_string(),
        icon_url: Some("https://example.com/icon.png".to_string()),
        zrc20_contract_address: Uuid::new_v4().to_string(),
    };

    let database = ZetachainCctxDatabase::new(db.client(), 7001);
    database
        .sync_tokens(Uuid::new_v4(), vec![token_eth.clone(), token_usdc.clone()])
        .await
        .unwrap();

    let mut dummy_cctx_2 = dummy_cross_chain_tx("dummy_cctx_2", "OutboundMined");
    dummy_cctx_2.inbound_params.asset = token_usdc.asset.clone();
    dummy_cctx_2.inbound_params.coin_type = token_usdc.coin_type;
    dummy_cctx_2.inbound_params.sender_chain_id = token_usdc.foreign_chain_id.clone();

    let database = ZetachainCctxDatabase::new(db.client(), 7001);
    let tx = db.client().begin().await.unwrap();
    database.setup_db().await.unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &vec![dummy_cctx_1, dummy_cctx_2], &tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let cctxs = database
        .list_cctxs(
            10,
            None,
            Filters {
                status_reduced: vec!["Pending".to_string()],
                sender_address: vec!["0x1234567890123456789012345678901234567890".to_string()],
                receiver_address: vec![],
                asset: vec![],
                coin_type: vec![],
                source_chain_id: vec![],
                target_chain_id: vec![],
                start_timestamp: None,
                end_timestamp: None,
                token_symbol: vec![],
            },
            Direction::Desc,
        )
        .await
        .unwrap();

    assert_eq!(cctxs.items.len(), 1);

    let cctxs = database
        .list_cctxs(
            10,
            None,
            Filters {
                status_reduced: vec!["Pending".to_string(), "Success".to_string()],
                sender_address: vec![],
                receiver_address: vec![],
                asset: vec![],
                coin_type: vec![],
                source_chain_id: vec![],
                target_chain_id: vec![],
                start_timestamp: None,
                end_timestamp: None,
                token_symbol: vec![],
            },
            Direction::Desc,
        )
        .await
        .unwrap();

    assert_eq!(cctxs.items.len(), 2);
}

#[tokio::test]
async fn query_cctxs_with_token_symbol_filter() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        init_logs(
            "tests",
            &blockscout_service_launcher::tracing::TracingSettings {
                enabled: true,
                ..Default::default()
            },
            &blockscout_service_launcher::tracing::JaegerSettings::default(),
        )
        .unwrap();
    }

    let db = init_db("test", "query_cctxs_with_token_symbol_filter").await;

    let eth = dummy_token("dummy_token_1", "ETH", None, "11155111", CoinType::Gas);
    let usdc = dummy_token(
        "dummy_token_2",
        "USDC",
        Some(Uuid::new_v4().to_string()),
        "11155111",
        CoinType::ERC20,
    );

    let database = ZetachainCctxDatabase::new(db.client(), 7001);

    database.setup_db().await.unwrap();
    database
        .sync_tokens(Uuid::new_v4(), vec![eth.clone(), usdc.clone()])
        .await
        .unwrap();

    let mut dummy_cctx_1 = dummy_cross_chain_tx("dummy_cctx_1", "PendingInbound");
    dummy_cctx_1.inbound_params.asset = eth.asset.clone();
    dummy_cctx_1.inbound_params.coin_type = eth.coin_type;
    dummy_cctx_1.inbound_params.sender_chain_id = "11155111".to_string();
    dummy_cctx_1.outbound_params[0].receiver_chain_id = "7001".to_string();

    let mut dummy_cctx_2 = dummy_cross_chain_tx("dummy_cctx_2", "OutboundMined");
    dummy_cctx_2.inbound_params.asset = usdc.asset.clone();
    dummy_cctx_2.inbound_params.coin_type = usdc.coin_type;
    dummy_cctx_2.inbound_params.sender_chain_id = "7001".to_string();
    dummy_cctx_2.outbound_params[0].receiver_chain_id = "11155111".to_string();

    let tx = db.client().begin().await.unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &vec![dummy_cctx_1, dummy_cctx_2], &tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let cctxs = database
        .list_cctxs(
            10,
            None,
            Filters {
                status_reduced: vec!["Pending".to_string()],
                sender_address: vec![],
                receiver_address: vec![],
                asset: vec![],
                coin_type: vec![],
                source_chain_id: vec![],
                target_chain_id: vec![],
                start_timestamp: None,
                end_timestamp: None,
                token_symbol: vec!["ETH".to_string()],
            },
            Direction::Desc,
        )
        .await
        .unwrap();

    assert_eq!(cctxs.items.len(), 1);

    let cctxs = database
        .list_cctxs(
            10,
            None,
            Filters {
                status_reduced: vec!["Success".to_string()],
                sender_address: vec![],
                receiver_address: vec![],
                asset: vec![],
                coin_type: vec![],
                source_chain_id: vec![],
                target_chain_id: vec![],
                start_timestamp: None,
                end_timestamp: None,
                token_symbol: vec!["USDC".to_string()],
            },
            Direction::Desc,
        )
        .await
        .unwrap();

    assert_eq!(cctxs.items.len(), 1);
}
