use sea_orm::DatabaseConnection;

use crate::celestia::{repository::blocks, tests::init_db};

#[tokio::test]
async fn upsert_test() {
    let db = init_db("blocks_db_upsert").await;

    for height in 1..=5 {
        let hash = [height as u8; 32];
        let blobs_count = height as u32;
        let timestamp = height as i64;
        blocks::upsert(&db.client(), height, &hash, blobs_count, timestamp)
            .await
            .unwrap();
        assert!(blocks::exists(&db.client(), height).await.unwrap());
    }
}

#[tokio::test]
async fn find_gaps_test() {
    let db = init_db("blocks_db_find_gaps").await;

    let heights = vec![0, 7, 12, 13, 14, 15, 17, 94, 156, 157];
    insert_heights(&db.client(), heights).await;

    let gaps = blocks::find_gaps(&db.client(), 200).await.unwrap();
    assert!(gaps[0].gap_start == 1 && gaps[0].gap_end == 6);
    assert!(gaps[1].gap_start == 8 && gaps[1].gap_end == 11);
    assert!(gaps[2].gap_start == 16 && gaps[2].gap_end == 16);
    assert!(gaps[3].gap_start == 18 && gaps[3].gap_end == 93);
    assert!(gaps[4].gap_start == 95 && gaps[4].gap_end == 155);
    assert!(gaps[5].gap_start == 158 && gaps[5].gap_end == 200);
}

#[tokio::test]
async fn find_gaps_empty_database_test() {
    let db = init_db("blocks_db_find_gaps_empty").await;

    let heights = vec![0];
    insert_heights(&db.client(), heights).await;

    let gaps = blocks::find_gaps(&db.client(), 200).await.unwrap();
    assert!(gaps[0].gap_start == 1 && gaps[0].gap_end == 200);
}

async fn insert_heights(db: &DatabaseConnection, heights: Vec<u64>) {
    for height in heights {
        let hash = [height as u8; 32];
        let blobs_count = height as u32;
        let timestamp = height as i64;
        blocks::upsert(db, height, &hash, blobs_count, timestamp)
            .await
            .unwrap();
    }
}
