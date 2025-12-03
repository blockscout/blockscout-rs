use crate::{common, common::repository::DbData, s3_storage::S3Storage};
use anyhow::Context;
use da_indexer_entity::{eigenda_v2_commitments, prelude::EigendaV2Commitments};
use sea_orm::{sea_query::OnConflict, DatabaseConnection, EntityTrait};
use sha3::{Digest, Keccak256};

pub async fn find_by_commitment(
    db: &DatabaseConnection,
    s3_storage: Option<&S3Storage>,
    commitment: &[u8],
) -> Result<Option<Vec<u8>>, anyhow::Error> {
    let commitment_hash = compute_commitment_hash(commitment);

    let commitment = EigendaV2Commitments::find_by_id(commitment_hash.clone())
        .one(db)
        .await
        .context("try to find commitment into database")?;

    if let Some(commitment) = commitment {
        let blob_data = common::repository::extract_blob_data(
            s3_storage,
            &commitment_hash,
            DbData {
                data: commitment.blob_data,
                data_s3_object_key: commitment.blob_data_s3_object_key,
            },
        )
        .await?;

        return Ok(Some(blob_data));
    }

    Ok(None)
}

pub async fn insert_commitment_with_data(
    db: &DatabaseConnection,
    s3_storage: Option<&S3Storage>,
    commitment: &[u8],
    blob_data: Vec<u8>,
) -> Result<(), anyhow::Error> {
    let commitment_hash = compute_commitment_hash(commitment);

    let (blob_db_data, blob_s3_object) =
        common::repository::convert_blob_data_to_db_data_and_s3_object(
            s3_storage,
            "eigenda_v2_commitment_blob",
            &commitment_hash,
            blob_data,
        );
    let data_s3_objects = blob_s3_object
        .map(|s3_object| vec![s3_object])
        .unwrap_or_default();

    let model = eigenda_v2_commitments::Model {
        commitment_hash,
        commitment_data: commitment.to_vec(),
        blob_data: blob_db_data.data.clone(),
        blob_data_s3_object_key: blob_db_data.data_s3_object_key,
    };
    let active_model: eigenda_v2_commitments::ActiveModel = model.into();

    let database_insert_future = async move {
        eigenda_v2_commitments::Entity::insert_many([active_model])
            .on_conflict(
                OnConflict::column(eigenda_v2_commitments::Column::CommitmentHash)
                    .do_nothing()
                    .to_owned(),
            )
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

fn compute_commitment_hash(commitment: &[u8]) -> Vec<u8> {
    Keccak256::digest(commitment).as_slice().to_vec()
}
