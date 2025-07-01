use crate::s3_storage::{S3Object, S3Storage};
use sea_orm::FromQueryResult;

#[derive(FromQueryResult, Default, Debug)]
pub struct DbData {
    pub data: Option<Vec<u8>>,
    pub data_s3_object_key: Option<String>,
}

pub fn convert_blob_data_to_db_data_and_s3_object(
    maybe_s3_storage: Option<&S3Storage>,
    da_prefix: &str,
    blob_id: &[u8],
    data: Vec<u8>,
) -> (DbData, Option<S3Object>) {
    if maybe_s3_storage.is_none() {
        (
            DbData {
                data: Some(data),
                data_s3_object_key: None,
            },
            None,
        )
    } else {
        let object_key = format!("{da_prefix}_{}", hex::encode(blob_id));
        let s3_object = S3Object {
            key: object_key.clone(),
            content: data,
        };
        (
            DbData {
                data: None,
                data_s3_object_key: Some(object_key),
            },
            Some(s3_object),
        )
    }
}

pub async fn extract_blob_data(
    s3_storage: Option<&S3Storage>,
    blob_id: &[u8],
    db_data: DbData,
) -> Result<Vec<u8>, anyhow::Error> {
    if let Some(data) = db_data.data {
        return Ok(data);
    }

    let data_s3_object_key = db_data.data_s3_object_key.ok_or_else(|| {
        // Must never be the case as we added a constraint on the postgres table
        // ensuring exactly one of `data` or `data_s3_object_key` fields is not null.
        tracing::error!(
            blob_id = hex::encode(blob_id),
            "both blob data and object key are missing from the database. \
            The database is corrupted!"
        );
        anyhow::anyhow!("both blob data and object key are missing from the database")
    })?;

    let s3_storage = s3_storage.ok_or_else(|| {
        tracing::error!(
            blob_id = hex::encode(blob_id),
            "blob data is located in s3 storage, but the storage is not available. \
                Please check your configuration."
        );
        anyhow::anyhow!("blob data was stored in s3 storage but the storage is not available")
    })?;

    let object = s3_storage.find_object_by_key(&data_s3_object_key).await?;
    object.map(|object| object.content).ok_or_else(|| {
        tracing::error!(
            blob_id = hex::encode(blob_id),
            data_s3_object_key = data_s3_object_key,
            "blob data is missing from s3 storage"
        );
        anyhow::anyhow!("blob data is missing from s3 storage")
    })
}
