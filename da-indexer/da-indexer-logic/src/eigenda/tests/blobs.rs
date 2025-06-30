use crate::{
    common::tests::{init_db, initialize_s3_storage, is_s3_storage_empty},
    eigenda::repository::{batches, blobs},
    s3_storage::S3Storage,
};
use blockscout_service_launcher::test_database::TestDbGuard;

#[tokio::test]
async fn eigenda_blobs_smoke_test_without_s3_storage() {
    let test_name = "eigenda_blobs_smoke_test_without_s3_storage";
    let db = init_db(test_name).await;
    smoke_test(db, None).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn eigenda_blobs_smoke_test_with_s3_storage() {
    let test_name = "eigenda_blobs_smoke_test_with_s3_storage";
    let db = init_db(test_name).await;
    let s3_storage = initialize_s3_storage(test_name).await;

    smoke_test(db, Some(s3_storage)).await;
    assert!(!is_s3_storage_empty(test_name).await);
}

async fn smoke_test(db: TestDbGuard, s3_storage: Option<S3Storage>) {
    let batch_header_hash =
        hex::decode("64C309747219667F2BF2F095B587E887DC066892FAF4DD035A31C7EA06577FA6").unwrap();
    let tx_hash =
        hex::decode("6d4aa4e79188a814b7a7788d2067004a57a04b0323a191662b7cfce9f6b8d8f4").unwrap();

    batches::upsert(
        db.client().as_ref(),
        &batch_header_hash,
        42,
        3,
        &tx_hash,
        1723129,
    )
    .await
    .expect("upsert failed");

    blobs::upsert_many(
        db.client().as_ref(),
        s3_storage.as_ref(),
        0,
        &batch_header_hash,
        vec![vec![0_u8; 32], vec![1_u8; 32], vec![2_u8; 32]],
    )
    .await
    .expect("upsert failed");

    let blob = blobs::find(
        db.client().as_ref(),
        s3_storage.as_ref(),
        &batch_header_hash,
        2,
    )
    .await
    .expect("find failed")
    .unwrap();
    assert_eq!(blob.batch_id, 42);
    assert_eq!(blob.blob_index, 2);
    assert_eq!(blob.l1_tx_hash, tx_hash);
    assert_eq!(blob.l1_block, 1723129);
    assert_eq!(blob.data, vec![2_u8; 32]);
    assert_eq!(blob.batch_header_hash, batch_header_hash);
}
