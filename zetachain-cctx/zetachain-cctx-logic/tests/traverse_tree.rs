use blockscout_service_launcher::test_database::TestDbGuard;
use chrono::Utc;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use uuid::Uuid;
use zetachain_cctx_entity::{
    cctx_status, cross_chain_tx,
    sea_orm_active_enums::{CctxStatusStatus, ProcessingStatus, ProtocolContractVersion},
};
use zetachain_cctx_logic::database::ZetachainCctxDatabase;

mod helpers;

// Helper that creates a brand-new temporary database, runs migrations and returns the guard
async fn init_db(test_name: &str) -> TestDbGuard {
    let db_name = format!("cctx_logic_{test_name}");
    TestDbGuard::new::<migration::Migrator>(&db_name).await
}

#[tokio::test]
async fn test_traverse_and_update_tree_relationships() {
    let db = init_db("tree_relationships").await;
    let db_conn = db.client();
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        helpers::init_tests_logs().await;
    }
    let database = ZetachainCctxDatabase::new(db_conn.clone(), 7001);
    database.setup_db().await.unwrap();

    // Insert ROOT CCTX (has no parent/root links yet)
    let root_index = "root";
    let root_tx = cross_chain_tx::ActiveModel {
        id: ActiveValue::NotSet,
        creator: ActiveValue::Set("creator".into()),
        index: ActiveValue::Set(root_index.into()),
        zeta_fees: ActiveValue::Set("0".into()),
        retries_number: ActiveValue::Set(0),
        token_id: ActiveValue::Set(None),
        receiver: ActiveValue::Set(Some("0xdeadbeef".to_string())),
        receiver_chain_id: ActiveValue::Set(Some(111555111)),
        processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
        relayed_message: ActiveValue::Set(Some("msg".into())),
        last_status_update_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
        protocol_contract_version: ActiveValue::Set(ProtocolContractVersion::V1),
        root_id: ActiveValue::Set(None),
        parent_id: ActiveValue::Set(None),
        depth: ActiveValue::Set(0),
        updated_by: ActiveValue::Set("test".into()),
    };
    let insert_res = cross_chain_tx::Entity::insert(root_tx)
        .exec(db_conn.as_ref())
        .await
        .unwrap();
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
    let child1_tx = helpers::dummy_cross_chain_tx(child1_index, "PendingOutbound");
    let tx = db_conn.begin().await.unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &vec![child1_tx.clone()], &tx, None)
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
    let grandchild_tx = helpers::dummy_cross_chain_tx(grandchild_index, "PendingOutbound");
    // insert grandchild first
    let tx2 = db_conn.begin().await.unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &vec![grandchild_tx], &tx2, None)
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
        token_id: Some(1),
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
