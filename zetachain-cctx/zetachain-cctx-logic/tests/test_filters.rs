mod helpers;

use blockscout_service_launcher::tracing::init_logs;
use zetachain_cctx_logic::{database::ZetachainCctxDatabase, models::Filters};
use migration::sea_orm::TransactionTrait;
use crate::helpers::*;
use uuid::Uuid;


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

    let dummy_cctx_2 = dummy_cross_chain_tx("dummy_cctx_2", "OutboundMined");
    

    let database = ZetachainCctxDatabase::new(db.client());
    let tx = db.client().begin().await.unwrap();
    database.batch_insert_transactions(Uuid::new_v4(), &vec![dummy_cctx_1, dummy_cctx_2], &tx).await.unwrap();
    tx.commit().await.unwrap();

    
    let cctxs = database.list_cctxs(10, 0, Filters {
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
    }).await.unwrap();

    assert_eq!(cctxs.len(), 1);

    let cctxs = database.list_cctxs(10, 0, Filters {
        status_reduced: vec!["Pending".to_string(),"Success".to_string()],
        sender_address: vec![],
        receiver_address: vec![],
        asset: vec![],
        coin_type: vec![],
        source_chain_id: vec![],
        target_chain_id: vec![],
        start_timestamp: None,
        end_timestamp: None,
        token_symbol: vec![],
    }).await.unwrap();

    assert_eq!(cctxs.len(), 2);


}