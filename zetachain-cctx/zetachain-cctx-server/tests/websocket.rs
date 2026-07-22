mod helpers;
use actix_phoenix_channel::ChannelCentral;
use blockscout_service_launcher::tracing::{init_logs, JaegerSettings, TracingSettings};
use futures::StreamExt;

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{sync::Arc, time::Duration};

use uuid::Uuid;
use wiremock::{
    matchers::{method, path, path_regex, query_param},
    Mock, MockServer, ResponseTemplate,
};
use zetachain_cctx_logic::{
    channel::Channel,
    client::{Client, RpcSettings},
    database::ZetachainCctxDatabase,
    models::{CctxStatusStatus, CoinType},
};

use crate::helpers::{dummy_cross_chain_tx, dummy_token};
use futures::SinkExt;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::CctxListItem;

#[derive(Deserialize, Serialize)]
struct ReplyOk {
    join_ref: Option<String>,
    ref_number: Option<String>,
    topic: String,
    event: String,
    payload: serde_json::Value,
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn emit_imported_cctxs() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        init_logs(
            "emit_imported_cctxs",
            &TracingSettings::default(),
            &JaegerSettings::default(),
        )
        .unwrap();
    }
    let db = crate::helpers::init_db("test", "websocket").await;
    let database = Arc::new(ZetachainCctxDatabase::new(db.client().clone(), 7001));
    let zeta_token = dummy_token("Test Token", "TEST", None, "1", CoinType::Zeta);
    let erc20_token = dummy_token(
        "Test Token",
        "TEST",
        Some("0x123".to_string()),
        "1",
        CoinType::ERC20,
    );
    let gas_token = dummy_token("Test Token", "TEST", None, "1", CoinType::Gas);

    database
        .sync_tokens(
            Uuid::new_v4(),
            vec![zeta_token.clone(), erc20_token.clone(), gas_token.clone()],
        )
        .await
        .unwrap();

    let mock_server = MockServer::start().await;

    let dummy_cctxs = [zeta_token, erc20_token, gas_token]
        .iter()
        .map(|token| {
            let mut cctx = dummy_cross_chain_tx(
                &Uuid::new_v4().to_string(),
                CctxStatusStatus::PendingOutbound,
            );
            cctx.inbound_params.coin_type = token.coin_type.clone();
            cctx.inbound_params.asset = token.asset.clone();
            cctx.inbound_params.sender_chain_id = token.foreign_chain_id.clone();
            cctx
        })
        .collect::<Vec<_>>();

    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "CrossChainTx": dummy_cctxs,
            "pagination": {
                "next_key": "",
                "total": "3"
            }
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "MH=="))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "CrossChainTx": [],
            "pagination": {
                "next_key": "MH==",
                "total": "0"
            }
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex("/zeta-chain/crosschain/cctx/.+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "CrossChainTx": dummy_cctxs[0]
        })))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(
            "/zeta-chain/crosschain/inboundHashToCctxData/.+",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "CrossChainTxs": []
        })))
        .mount(&mock_server)
        .await;
    let rpc_client = Client::new(RpcSettings {
        url: mock_server.uri().to_string(),
        ..Default::default()
    });
    let channel = Arc::new(ChannelCentral::new(Channel));

    let channel_clone = channel.clone();
    // Start the full server with websocket support
    let server_handle = tokio::spawn(async move {
        let base_url = crate::helpers::init_zetachain_cctx_server_with_channel(
            db.db_url(),
            |mut settings| {
                settings.tracing.enabled = false;
                settings.websocket.enabled = true;
                settings.indexer.enabled = true;
                settings.indexer.zetachain_id = 7001;
                settings.indexer.token_sync_enabled = false;
                settings.indexer.polling_interval = 100;
                settings
            },
            db.client(),
            Arc::new(rpc_client),
            channel_clone,
        )
        .await;
        tracing::info!("server started");
        base_url
    });

    // Wait for server to start and get the base URL
    let base_url = server_handle.await.unwrap();
    let mut result = true;
    timeout(Duration::from_millis(500), async move {
        tracing::info!("waiting for cctxs");

        // Connect to the websocket server
        let ws_url = format!(
            "ws://{}:{}/socket/websocket?vsn=2.0.0",
            base_url.host_str().unwrap(),
            base_url.port().unwrap()
        );
        tracing::info!("connecting to websocket: {}", ws_url);

        let (ws_stream, _) = connect_async(&ws_url).await.unwrap();
        let (mut write, mut read) = ws_stream.split();

        let join_message = serde_json::to_string(&(
            Some("1".to_string()), // join_ref
            Some("1".to_string()), // ref
            "cctxs:new_cctxs",     // topic
            "phx_join",            // event
            "{}",                  // payload
        ))
        .unwrap();

        tracing::info!("sending join message: {}", join_message);
        write.send(Message::Text(join_message)).await.unwrap();

        loop {
            if let Some(message) = read.next().await {
                if let Message::Text(text) = message.unwrap() {
                    tracing::info!("received message: {}", text);
                    if let Ok(ReplyOk {
                        join_ref: _,
                        ref_number: _,
                        topic,
                        event,
                        payload,
                    }) = serde_json::from_str::<ReplyOk>(&text)
                    {
                        if topic == "cctxs:new_cctxs" && event == "new_cctxs" {
                            if let Ok(cctxs) =
                                serde_json::from_value::<Vec<CctxListItem>>(payload.clone())
                            {
                                let count = cctxs.len();
                                for cctx in cctxs {
                                    result = result
                                        && cctx.decimals.is_some()
                                        && cctx.token_symbol.is_some()
                                        && cctx.zrc20_contract_address.is_some();
                                }

                                result = result && count == 3;
                                tracing::info!("all cctxs received");
                                assert!(result);
                                break;
                            }
                        }
                    } else {
                        tracing::error!("failed to parse Phoenix Channel message: {}", text);
                    }
                }
            }
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!("Test timed out, events not received");
    });
}
