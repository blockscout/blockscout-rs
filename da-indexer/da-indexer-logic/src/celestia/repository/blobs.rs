use crate::{common, s3_storage::S3Storage};
use anyhow::Context;
use celestia_types::Blob as CelestiaBlob;
use da_indexer_entity::{
    celestia_blobs::{ActiveModel, Column, Entity, Model},
    celestia_blocks,
};
use sea_orm::{
    sea_query::OnConflict, ConnectionTrait, DatabaseConnection, EntityTrait, FromQueryResult,
    JoinType, QuerySelect, QueryTrait, SelectColumns,
};
use sha3::{Digest, Sha3_256};

#[derive(FromQueryResult)]
pub struct Blob {
    pub height: i64,
    pub namespace: Vec<u8>,
    pub commitment: Vec<u8>,
    #[sea_orm(skip)]
    pub data: Vec<u8>,
    pub timestamp: i64,
    #[sea_orm(nested)]
    db_data: common::repository::DbData,
}

pub async fn find_by_height_and_commitment(
    db: &DatabaseConnection,
    s3_storage: Option<&S3Storage>,
    height: u64,
    commitment: &[u8],
) -> Result<Option<Blob>, anyhow::Error> {
    let id = compute_id(height, commitment);

    let mut blob = Blob::find_by_statement(
        Entity::find_by_id(id.clone())
            .join_rev(
                JoinType::LeftJoin,
                celestia_blocks::Entity::belongs_to(Entity)
                    .from(celestia_blocks::Column::Height)
                    .to(Column::Height)
                    .into(),
            )
            .select_column(celestia_blocks::Column::Timestamp)
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
    height: u64,
    blobs: Vec<CelestiaBlob>,
) -> Result<(), anyhow::Error> {
    let mut data_s3_objects = vec![];
    let blobs: Vec<_> = blobs
        .into_iter()
        .map(|blob| {
            let id = compute_id(height, blob.commitment.hash());

            let (db_data, s3_object) =
                common::repository::convert_blob_data_to_db_data_and_s3_object(
                    s3_storage, "celestia", &id, blob.data,
                );
            if let Some(s3_object) = s3_object {
                data_s3_objects.push(s3_object);
            }

            let model = Model {
                id,
                height: height as i64,
                namespace: blob.namespace.as_bytes().to_vec(),
                commitment: blob.commitment.hash().to_vec(),
                data: db_data.data,
                data_s3_object_key: db_data.data_s3_object_key,
            };
            let active: ActiveModel = model.into();
            active
        })
        .collect();

    // id is the hash of height, namespace and data
    // so if we have a conflict, we can assume that the blob is the same
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

fn compute_id(height: u64, commitment: &[u8]) -> Vec<u8> {
    // commitment is not unique, but the combination of the height and commitment is
    Sha3_256::digest([&height.to_be_bytes()[..], commitment].concat())
        .as_slice()
        .to_vec()
}
