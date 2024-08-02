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
    pub data: Vec<u8>,
}

pub async fn find(
    db: &DatabaseConnection,
    batch_header_hash: &[u8],
    blob_index: i32,
) -> Result<Option<Blob>, anyhow::Error> {
    let id = compute_id(batch_header_hash, blob_index);

    let blob = Blob::find_by_statement(
        Entity::find_by_id(id)
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

    Ok(blob)
}

pub async fn upsert_many<C: ConnectionTrait>(
    db: &C,
    start_index: i32,
    batch_header_hash: &[u8],
    blobs: Vec<Vec<u8>>,
) -> Result<(), anyhow::Error> {
    let blobs = blobs.into_iter().enumerate().map(|(i, data)| {
        let blob_index = start_index + i as i32;
        let model = Model {
            id: compute_id(batch_header_hash, blob_index),
            batch_header_hash: batch_header_hash.to_vec(),
            blob_index,
            data,
        };
        let active: ActiveModel = model.into();
        active
    });

    Entity::insert_many(blobs)
        .on_conflict(OnConflict::column(Column::Id).do_nothing().to_owned())
        .on_empty_do_nothing()
        .exec(db)
        .await?;
    Ok(())
}

fn compute_id(batch_header_hash: &[u8], blob_index: i32) -> Vec<u8> {
    Sha3_256::digest([batch_header_hash, &blob_index.to_be_bytes()[..]].concat())
        .as_slice()
        .to_vec()
}
