use crate::{common, s3_storage::S3Storage};
use anyhow::Context;
use da_indexer_entity::{
    eigenda_batches,
    eigenda_blobs::{ActiveModel, Column, Entity, Model},
};
use sea_orm::{
    sea_query::OnConflict, ConnectionTrait, DatabaseConnection, EntityTrait, FromQueryResult,
    JoinType, QueryOrder, QuerySelect, QueryTrait, SelectColumns,
};
use sha3::{Digest, Sha3_256};

#[derive(FromQueryResult)]
pub struct Blob {
    pub batch_header_hash: Vec<u8>,
    pub batch_id: i64,
    pub blob_index: i32,
    pub l1_tx_hash: Vec<u8>,
    pub l1_block: i64,
    #[sea_orm(skip)]
    pub data: Vec<u8>,
    #[sea_orm(nested)]
    db_data: common::repository::DbData,
}

pub async fn find(
    db: &DatabaseConnection,
    s3_storage: Option<&S3Storage>,
    batch_header_hash: &[u8],
    blob_index: i32,
) -> Result<Option<Blob>, anyhow::Error> {
    let id = compute_id(batch_header_hash, blob_index);

    let mut blob = Blob::find_by_statement(
        Entity::find_by_id(id.clone())
            .join_rev(
                JoinType::LeftJoin,
                eigenda_batches::Entity::belongs_to(Entity)
                    .from(eigenda_batches::Column::BatchHeaderHash)
                    .to(Column::BatchHeaderHash)
                    .into(),
            )
            .order_by_desc(eigenda_batches::Column::BatchId)
            .select_column(eigenda_batches::Column::BatchId)
            .select_column(eigenda_batches::Column::L1TxHash)
            .select_column(eigenda_batches::Column::L1Block)
            .limit(1)
            .build(db.get_database_backend()),
    )
    .one(db)
    .await?;

    if let Some(blob) = &mut blob {
        blob.data = common::repository::extract_blob_data(
            s3_storage,
            &id,
            std::mem::take(&mut blob.db_data),
        )
        .await?;
    }

    Ok(blob)
}

pub async fn upsert_many<C: ConnectionTrait>(
    db: &C,
    s3_storage: Option<&S3Storage>,
    start_index: i32,
    batch_header_hash: &[u8],
    blobs: Vec<Vec<u8>>,
) -> Result<(), anyhow::Error> {
    let mut data_s3_objects = vec![];
    let blobs: Vec<_> = blobs
        .into_iter()
        .enumerate()
        .map(|(i, blob_data)| {
            let blob_index = start_index + i as i32;

            let id = compute_id(batch_header_hash, blob_index);

            let (db_data, s3_object) =
                common::repository::convert_blob_data_to_db_data_and_s3_object(
                    s3_storage, "eigenda", &id, blob_data,
                );
            if let Some(s3_object) = s3_object {
                data_s3_objects.push(s3_object);
            }

            let model = Model {
                id: compute_id(batch_header_hash, blob_index),
                batch_header_hash: batch_header_hash.to_vec(),
                blob_index,
                data: db_data.data,
                data_s3_object_key: db_data.data_s3_object_key,
            };
            let active: ActiveModel = model.into();
            active
        })
        .collect();

    let database_insert_future = async move {
        Entity::insert_many(blobs)
            .on_conflict(OnConflict::column(Column::Id).do_nothing().to_owned())
            .on_empty_do_nothing()
            .exec(db)
            .await
            .context("database insertion")
    };
    let s3_storage_insert_future = async move {
        match s3_storage {
            None => Ok(()),
            Some(s3_storage) => s3_storage
                .insert_many(data_s3_objects)
                .await
                .context("s3 storage insertion"),
        }
    };
    futures::future::try_join(database_insert_future, s3_storage_insert_future).await?;

    Ok(())
}

fn compute_id(batch_header_hash: &[u8], blob_index: i32) -> Vec<u8> {
    Sha3_256::digest([batch_header_hash, &blob_index.to_be_bytes()[..]].concat())
        .as_slice()
        .to_vec()
}
