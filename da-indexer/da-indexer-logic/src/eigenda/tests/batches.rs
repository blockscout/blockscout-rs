use sea_orm::DatabaseConnection;

use crate::eigenda::{repository::batches, tests::init_db};

#[tokio::test]
async fn find_gaps() {
    let db = init_db("eigenda_batches_find_gaps_test").await;

    let heights = vec![7, 12, 13, 14, 15, 17, 94, 156, 157];
    insert_batches(&db.client(), heights).await;

    let gaps = batches::find_gaps(&db.client(), 53, 20009).await.unwrap();
    assert!(gaps[0].start == 53 && gaps[0].end == 700);
    assert!(gaps[1].start == 700 && gaps[1].end == 1200);
    assert!(gaps[2].start == 1500 && gaps[2].end == 1700);
    assert!(gaps[3].start == 1700 && gaps[3].end == 9400);
    assert!(gaps[4].start == 9400 && gaps[4].end == 15600);
    assert!(gaps[5].start == 15700 && gaps[5].end == 20009);
}

#[tokio::test]
async fn find_gaps_empty_database() {
    let db = init_db("eigenda_batches_find_gaps_empty_test").await;

    let gaps = batches::find_gaps(&db.client(), 0, 200).await.unwrap();
    assert!(gaps[0].start == 0 && gaps[0].end == 200);
}

async fn insert_batches(db: &DatabaseConnection, batches: Vec<i64>) {
    // for simplicity l1_blocks = batch_id * 100
    let l1_tx_hash = vec![5, 6, 7, 8];
    for batch_id in batches.iter() {
        batches::upsert(
            db,
            &vec![(*batch_id % 256) as u8, 0, 0][..],
            *batch_id,
            10,
            &l1_tx_hash,
            *batch_id * 100,
        )
        .await
        .unwrap();
    }
}
