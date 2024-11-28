use da_indexer_entity::{
    celestia_blobs::{ActiveModel, Column, Entity, Model},
    celestia_blocks,
};
use sea_orm::{
    sea_query::OnConflict, ConnectionTrait, DatabaseConnection, EntityTrait, FromQueryResult,
    JoinType, QuerySelect, QueryTrait, SelectColumns,
};
use sha3::{Digest, Sha3_256};

use celestia_types::Blob as CelestiaBlob;

#[derive(FromQueryResult)]
pub struct Blob {
    pub height: i64,
    pub namespace: Vec<u8>,
    pub commitment: Vec<u8>,
    pub data: Vec<u8>,
    pub timestamp: i64,
}

pub async fn find_by_height_and_commitment(
    db: &DatabaseConnection,
    height: u64,
    commitment: &[u8],
) -> Result<Option<Blob>, anyhow::Error> {
    let id = compute_id(height, commitment);

    let blob = Blob::find_by_statement(
        Entity::find_by_id(id)
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
    Ok(blob)
}

pub async fn upsert_many<C: ConnectionTrait>(
    db: &C,
    height: u64,
    blobs: Vec<CelestiaBlob>,
) -> Result<(), anyhow::Error> {
    let blobs = blobs.into_iter().map(|blob| {
        let model = Model {
            id: compute_id(height, &blob.commitment.0),
            height: height as i64,
            namespace: blob.namespace.as_bytes().to_vec(),
            commitment: blob.commitment.0.to_vec(),
            data: blob.data,
        };
        let active: ActiveModel = model.into();
        active
    });

    // id is the hash of height, namespace and data
    // so if we have a conflict, we can assume that the blob is the same
    Entity::insert_many(blobs)
        .on_conflict(OnConflict::column(Column::Id).do_nothing().to_owned())
        .on_empty_do_nothing()
        .exec(db)
        .await?;
    Ok(())
}

fn compute_id(height: u64, commitment: &[u8]) -> Vec<u8> {
    // commitment is not unique, but the combination of the height and commitment is
    Sha3_256::digest([&height.to_be_bytes()[..], commitment].concat())
        .as_slice()
        .to_vec()
}
