use actix_phoenix_channel::ChannelCentral;
use blockscout_service_launcher::{test_database::TestDbGuard, test_server};
use chrono::Utc;
use rand::Rng;
use reqwest::Url;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;
use zetachain_cctx_logic::{
    channel::Channel,
    client::Client,
    models::{
        CallOptions, CctxStatus, CctxStatusStatus, CoinType, CrossChainTx, InboundParams,
        OutboundParams, RevertOptions, Token,
    },
};
use zetachain_cctx_server::Settings;

#[allow(dead_code)]
pub async fn init_db(db_prefix: &str, test_name: &str) -> TestDbGuard {
    // Initialize tracing for all tests that use this helper
    let db_name = format!("{db_prefix}_{test_name}");
    TestDbGuard::new::<migration::Migrator>(db_name.as_str()).await
}

#[allow(dead_code)]
pub async fn init_tests_logs() {
    blockscout_service_launcher::tracing::init_logs(
        "tests",
        &blockscout_service_launcher::tracing::TracingSettings {
            enabled: true,
            ..Default::default()
        },
        &blockscout_service_launcher::tracing::JaegerSettings::default(),
    )
    .unwrap();
}

#[allow(dead_code)]
pub async fn init_zetachain_cctx_server<F>(
    db_url: String,
    settings_setup: F,
    db: Arc<DatabaseConnection>,
    client: Arc<Client>,
) -> Url
where
    F: Fn(Settings) -> Settings,
{
    // Initialize tracing for server tests
    // init_tracing();

    let (settings, base) = {
        let mut settings = Settings::default(db_url);
        let (server_settings, base) = test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.metrics.enabled = false;
        settings.tracing.enabled = true;
        settings.jaeger.enabled = false;

        (settings_setup(settings), base)
    };
    let channel = Arc::new(ChannelCentral::new(Channel));
    let channel_clone = channel.clone();

    test_server::init_server(
        || zetachain_cctx_server::run(settings, db, client, channel_clone),
        &base,
    )
    .await;
    base
}
#[allow(dead_code)]
pub async fn init_zetachain_cctx_server_with_channel<F>(
    db_url: String,
    settings_setup: F,
    db: Arc<DatabaseConnection>,
    client: Arc<Client>,
    channel: Arc<ChannelCentral<Channel>>,
) -> Url
where
    F: Fn(Settings) -> Settings,
{
    // Initialize tracing for server tests
    // init_tracing();

    let (settings, base) = {
        let mut settings = Settings::default(db_url);
        let (server_settings, base) = test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.metrics.enabled = false;
        settings.tracing.enabled = true;
        settings.jaeger.enabled = false;

        (settings_setup(settings), base)
    };
    let channel_clone = channel.clone();

    test_server::init_server(
        || zetachain_cctx_server::run(settings, db, client, channel_clone),
        &base,
    )
    .await;
    base
}

#[allow(dead_code)]
pub fn dummy_token(
    name: &str,
    symbol: &str,
    asset: Option<String>,
    chain_id: &str,
    coin_type: CoinType,
) -> Token {
    Token {
        foreign_chain_id: chain_id.to_string(),
        symbol: symbol.to_string(),
        name: name.to_string(),
        decimals: 18,
        zrc20_contract_address: Uuid::new_v4().to_string(),
        asset: asset.unwrap_or("".to_string()),
        coin_type,
        gas_limit: "1000000000000000000".to_string(),
        paused: false,
        liquidity_cap: "1000000000000000000".to_string(),
        icon_url: Some("https://example.com/icon.png".to_string()),
    }
}
#[allow(dead_code)]
pub fn dummy_cross_chain_tx(index: &str, status: CctxStatusStatus) -> CrossChainTx {
    CrossChainTx {
        creator: "creator".to_string(),
        index: index.to_string(),
        zeta_fees: "0".to_string(),
        relayed_message: "msg".to_string(),
        cctx_status: CctxStatus {
            status,
            status_message: "".to_string(),
            error_message: "".to_string(),
            last_update_timestamp: (Utc::now().timestamp() - rand::rng().random_range(1000..10000))
                .to_string(),
            is_abort_refunded: false,
            created_timestamp: "0".to_string(),
            error_message_revert: "".to_string(),
            error_message_abort: "".to_string(),
        },
        inbound_params: InboundParams {
            sender: "sender".to_string(),
            sender_chain_id: "1".to_string(),
            tx_origin: "origin".to_string(),
            coin_type: CoinType::ERC20,
            asset: "0x0000000000000000000000000000000000000001".to_string(),
            amount: "8504".to_string(),
            observed_hash: index.to_string(),
            observed_external_height: "0".to_string(),
            ballot_index: index.to_string(),
            finalized_zeta_height: "0".to_string(),
            tx_finalization_status: "NotFinalized".to_string(),
            is_cross_chain_call: false,
            status: "SUCCESS".to_string(),
            confirmation_mode: "SAFE".to_string(),
        },
        outbound_params: vec![
            OutboundParams {
                receiver: "receiver".to_string(),
                receiver_chain_id: "2".to_string(),
                coin_type: CoinType::Zeta,
                amount: "1000000000000000000".to_string(),
                tss_nonce: "42".to_string(),
                gas_limit: "1337".to_string(),
                gas_price: "0".to_string(),
                gas_priority_fee: "0".to_string(),
                hash: format!("{index}_1"),
                ballot_index: "".to_string(),
                observed_external_height: "0".to_string(),
                gas_used: "0".to_string(),
                effective_gas_price: "0".to_string(),
                effective_gas_limit: "0".to_string(),
                tss_pubkey: "".to_string(),
                tx_finalization_status: "NotFinalized".to_string(),
                call_options: Some(CallOptions {
                    gas_limit: "0".to_string(),
                    is_arbitrary_call: false,
                }),
                confirmation_mode: "SAFE".to_string(),
            },
            OutboundParams {
                receiver: "receiver2".to_string(),
                receiver_chain_id: "3".to_string(),
                coin_type: CoinType::ERC20,
                amount: "42691234567890".to_string(),
                tss_nonce: "0".to_string(),
                gas_limit: "0".to_string(),
                gas_price: "0".to_string(),
                gas_priority_fee: "0".to_string(),
                hash: format!("{index}_2"),
                ballot_index: "".to_string(),
                observed_external_height: "0".to_string(),
                gas_used: "0".to_string(),
                effective_gas_price: "0".to_string(),
                effective_gas_limit: "0".to_string(),
                tss_pubkey: "".to_string(),
                tx_finalization_status: "NotFinalized".to_string(),
                call_options: Some(CallOptions {
                    gas_limit: "0".to_string(),
                    is_arbitrary_call: false,
                }),
                confirmation_mode: "SAFE".to_string(),
            },
        ],
        protocol_contract_version: "V2".to_string(),
        revert_options: RevertOptions {
            revert_address: "".to_string(),
            call_on_revert: false,
            abort_address: "".to_string(),
            revert_message: None,
            revert_gas_limit: "0".to_string(),
        },
    }
}

#[allow(dead_code)]
pub fn dummy_related_cctxs_response(indices: &[&str]) -> serde_json::Value {
    let cctxs = indices
        .iter()
        .map(|index| dummy_cross_chain_tx(index, CctxStatusStatus::PendingOutbound))
        .map(|cctx| serde_json::json!(cctx))
        .collect::<Vec<Value>>();

    let cctxs_arr = serde_json::json!({
        "CrossChainTxs": cctxs,
    });

    cctxs_arr
}

#[allow(dead_code)]
pub fn dummy_cctx_with_pagination_response(indices: &[&str], next_key: &str) -> serde_json::Value {
    let cctxs = indices
        .iter()
        .map(|index| dummy_cross_chain_tx(index, CctxStatusStatus::PendingOutbound))
        .map(|cctx| serde_json::json!(cctx))
        .collect::<Vec<Value>>();

    let cctxs_arr = serde_json::json!({
        "CrossChainTx": cctxs,
        "pagination": {
        "next_key": next_key,
        "total": "0"
    }
    });

    cctxs_arr
}

#[allow(dead_code)]
pub fn empty_response() -> serde_json::Value {
    serde_json::json!({
    "CrossChainTx": [],
    "pagination": {
        "next_key": "end",
        "total": "0"
    }
    })
}
