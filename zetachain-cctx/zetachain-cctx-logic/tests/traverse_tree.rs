
use blockscout_service_launcher::test_database::TestDbGuard;
use chrono::Utc;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;
use zetachain_cctx_entity::{
    cctx_status, cross_chain_tx,
    sea_orm_active_enums::{CctxStatusStatus, ProcessingStatus, ProtocolContractVersion},
};
use sea_orm::TransactionTrait;
use zetachain_cctx_logic::{
    database::ZetachainCctxDatabase,
    models::{CallOptions, CctxStatus, CrossChainTx, InboundParams, OutboundParams, RevertOptions},
};

// Helper that creates a brand-new temporary database, runs migrations and returns the guard
async fn init_db(test_name: &str) -> TestDbGuard {
    let db_name = format!("cctx_logic_{test_name}");
    TestDbGuard::new::<migration::Migrator>(&db_name).await
}


#[allow(dead_code)]
pub fn dummy_cross_chain_tx(index: &str, status: &str) -> CrossChainTx {
    CrossChainTx {
        creator: "creator".to_string(),
        index: index.to_string(),
        zeta_fees: "0".to_string(),
        relayed_message: "msg".to_string(),
        cctx_status: CctxStatus {
            status: status.to_string(),
            status_message: "".to_string(),
            error_message: "".to_string(),
            last_update_timestamp: (Utc::now().timestamp() - 1000).to_string(),
            is_abort_refunded: false,
            created_timestamp: "0".to_string(),
            error_message_revert: "".to_string(),
            error_message_abort: "".to_string(),
        },
        inbound_params: InboundParams {
            sender: "sender".to_string(),
            sender_chain_id: "1".to_string(),
            tx_origin: "origin".to_string(),
            coin_type: zetachain_cctx_logic::models::CoinType::Zeta,
            asset: "".to_string(),
            amount: "0".to_string(),
            observed_hash: index.to_string(),
            observed_external_height: "0".to_string(),
            ballot_index: index.to_string(),
            finalized_zeta_height: "0".to_string(),
            tx_finalization_status: "NotFinalized".to_string(),
            is_cross_chain_call: false,
            status: "SUCCESS".to_string(),
            confirmation_mode: "SAFE".to_string(),
        },
        outbound_params: vec![OutboundParams {
            receiver: "receiver".to_string(),
            receiver_chain_id: "2".to_string(),
            coin_type: zetachain_cctx_logic::models::CoinType::Zeta,
            amount: "1000000000000000000".to_string(),
            tss_nonce: "0".to_string(),
            gas_limit: "0".to_string(),
            gas_price: "0".to_string(),
            gas_priority_fee: "0".to_string(),
            hash: index.to_string(),
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
        }],
        protocol_contract_version: "V1".to_string(),
        revert_options: RevertOptions {
            revert_address: "".to_string(),
            call_on_revert: false,
            abort_address: "".to_string(),
            revert_message: None,
            revert_gas_limit: "0".to_string(),
        },
    }
}

#[tokio::test]
async fn test_traverse_and_update_tree_relationships() {
    let db = init_db("tree_relationships").await;
    let db_conn = db.client();
    let database = ZetachainCctxDatabase::new(db_conn.clone());

    // Insert ROOT CCTX (has no parent/root links yet)
    let root_index = "root";
    let root_tx = cross_chain_tx::ActiveModel {
        id: ActiveValue::NotSet,
        creator: ActiveValue::Set("creator".into()),
        index: ActiveValue::Set(root_index.into()),
        zeta_fees: ActiveValue::Set("0".into()),
        retries_number: ActiveValue::Set(0),
        processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
        relayed_message: ActiveValue::Set(Some("msg".into())),
        last_status_update_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
        protocol_contract_version: ActiveValue::Set(ProtocolContractVersion::V1),
        root_id: ActiveValue::Set(None),
        parent_id: ActiveValue::Set(None),
        depth: ActiveValue::Set(0),
        updated_by: ActiveValue::Set("test".into()),
    };
    let insert_res = cross_chain_tx::Entity::insert(root_tx).exec(db_conn.as_ref()).await.unwrap();
    let root_id = insert_res.last_insert_id;

    // Status row for root (OutboundMined so that the function marks processed)
    cctx_status::Entity::insert(cctx_status::ActiveModel {
        id: ActiveValue::NotSet,
        cross_chain_tx_id: ActiveValue::Set(root_id),
        status: ActiveValue::Set(CctxStatusStatus::OutboundMined),
        status_message: ActiveValue::Set(None),
        error_message: ActiveValue::Set(None),
        last_update_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
        is_abort_refunded: ActiveValue::Set(false),
        created_timestamp: ActiveValue::Set(0),
        error_message_revert: ActiveValue::Set(None),
        error_message_abort: ActiveValue::Set(None),
    })
    .exec(db_conn.as_ref())
    .await
    .unwrap();

    // Insert CHILD1 without parent/root links
    let child1_index = "child1";
    let child1_tx = dummy_cross_chain_tx(child1_index, "PendingOutbound");
    let tx = db_conn.begin().await.unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &vec![child1_tx.clone()], &tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Retrieve child1 id
    let child1_row = cross_chain_tx::Entity::find()
        .filter(cross_chain_tx::Column::Index.eq(child1_index))
        .one(db_conn.as_ref())
        .await
        .unwrap()
        .unwrap();

    // Insert GRANDCHILD that references CHILD1 as its parent, but root is unknown
    let grandchild_index = "grandchild";
    let grandchild_tx = dummy_cross_chain_tx(grandchild_index, "PendingOutbound");
    // insert grandchild first
    let tx2 = db_conn.begin().await.unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &vec![grandchild_tx], &tx2)
        .await
        .unwrap();
    tx2.commit().await.unwrap();

    // update grandchild.parent_id to child1
    cross_chain_tx::Entity::update_many()
        .filter(cross_chain_tx::Column::Index.eq(grandchild_index))
        .set(cross_chain_tx::ActiveModel {
            parent_id: ActiveValue::Set(Some(child1_row.id)),
            ..Default::default()
        })
        .exec(db_conn.as_ref())
        .await
        .unwrap();

    // Prepare inputs for the function under test
    let root_short = zetachain_cctx_logic::models::CctxShort {
        id: root_id,
        index: root_index.to_string(),
        root_id: None,
        depth: 0,
        retries_number: 0,
    };

    let job_id = Uuid::new_v4();

    database
        .traverse_and_update_tree_relationships(vec![child1_tx], &root_short, job_id)
        .await
        .unwrap();

    // Assertions
    let updated_child1 = cross_chain_tx::Entity::find()
        .filter(cross_chain_tx::Column::Index.eq(child1_index))
        .one(db_conn.as_ref())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(updated_child1.parent_id, Some(root_id));
    assert_eq!(updated_child1.root_id, Some(root_id));

    let updated_grandchild = cross_chain_tx::Entity::find()
        .filter(cross_chain_tx::Column::Index.eq(grandchild_index))
        .one(db_conn.as_ref())
        .await
        .unwrap()
        .unwrap();

    // Root link should point to ROOT after traversal
    assert_eq!(updated_grandchild.root_id, Some(root_id));
} 