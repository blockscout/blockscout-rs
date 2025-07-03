use anyhow::Context;
use blockscout_service_launcher::{
    database, database::DatabaseSettings, launcher::ConfigSettings, tracing::TracingSettings,
};
use da_indexer_entity::{celestia_blobs, eigenda_blobs};
use da_indexer_logic::s3_storage::{S3Object, S3Storage, S3StorageSettings};
use migration::sea_orm::{EntityTrait, FromQueryResult};
use sea_orm::{ConnectionTrait, DatabaseConnection, Set, Statement, sea_query::OnConflict};
use serde::Deserialize;
use std::time::Duration;

macro_rules! call_retriable {
    ($function:expr) => {
        call_retriable(|| async { Ok::<_, anyhow::Error>($function) }).await?
    };
}

#[derive(Clone, Debug, Deserialize)]
struct Settings {
    pub da: DaLayer,
    pub database: DatabaseSettings,
    pub s3_storage: S3StorageSettings,
    #[serde(default = "default_migration_batch_size")]
    pub migration_batch_size: u64,
    #[serde(default)]
    pub tracing: TracingSettings,
}

fn default_migration_batch_size() -> u64 {
    1000
}

const SERVICE_NAME: &str = "data_s3_migrator";

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = SERVICE_NAME;
}

#[derive(Clone, Copy, Debug, Deserialize)]
enum DaLayer {
    Celestia,
    Eigenda,
}

impl DaLayer {
    pub fn table_name(&self) -> &'static str {
        match self {
            DaLayer::Celestia => "celestia_blobs",
            DaLayer::Eigenda => "eigenda_blobs",
        }
    }

    pub fn s3_prefix(&self) -> &'static str {
        match self {
            DaLayer::Celestia => "celestia",
            DaLayer::Eigenda => "eigenda",
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");
    blockscout_service_launcher::tracing::init_logs(
        SERVICE_NAME,
        &settings.tracing,
        &Default::default(),
    )?;

    let database = {
        let mut database_settings = settings.database;
        // Script must be run only on already setup database.
        database_settings.create_database = false;
        database_settings.run_migrations = false;
        database::initialize_postgres::<migration::Migrator>(&database_settings)
            .await
            .context("initialize database")?
    };

    let s3_storage = S3Storage::new(settings.s3_storage)
        .await
        .context("initialize s3-storage")?;

    let da_layer = settings.da;
    let mut iteration = 0;
    loop {
        iteration += 1;
        let blobs_to_migrate = call_retriable!(
            retrieve_blobs_to_migrate(&database, da_layer, settings.migration_batch_size)
                .await
                .context("retrieve blobs to migrate")?
        );

        if blobs_to_migrate.is_empty() {
            tracing::info!("finished on iteration {iteration}");
            break;
        }

        let blob_ids: Vec<_> = blobs_to_migrate
            .iter()
            .map(|blob| blob.id.clone())
            .collect();

        let s3_objects: Vec<_> = blobs_to_migrate
            .into_iter()
            .map(|blob| S3Object {
                key: blob_id_to_object_key(da_layer, &blob.id),
                content: blob.data,
            })
            .collect();
        call_retriable!(
            s3_storage
                .insert_many(s3_objects.clone())
                .await
                .context("insert blob data into s3 storage")?
        );

        call_retriable!(
            update_database_entries(&database, da_layer, blob_ids.clone())
                .await
                .context("update database entries")?
        );
    }

    Ok(())
}

#[derive(Clone, Debug, FromQueryResult)]
struct Blob {
    id: Vec<u8>,
    data: Vec<u8>,
}

async fn retrieve_blobs_to_migrate(
    database: &DatabaseConnection,
    da_layer: DaLayer,
    limit: u64,
) -> Result<Vec<Blob>, anyhow::Error> {
    let query = format!(
        r#"
        SELECT id, data FROM {} WHERE data IS NOT NULL LIMIT {limit};
    "#,
        da_layer.table_name()
    );

    let blobs = Blob::find_by_statement(Statement::from_string(
        database.get_database_backend(),
        query,
    ))
    .all(database)
    .await?;
    Ok(blobs)
}

async fn update_database_entries(
    database: &DatabaseConnection,
    da_layer: DaLayer,
    blob_ids: Vec<Vec<u8>>,
) -> Result<(), anyhow::Error> {
    match da_layer {
        DaLayer::Celestia => {
            let active_models = blob_ids
                .into_iter()
                .map(|id| {
                    let object_key = blob_id_to_object_key(da_layer, &id);
                    celestia_blobs::ActiveModel {
                        id: Set(id),
                        data: Set(None),
                        data_s3_object_key: Set(Some(object_key)),
                        ..Default::default()
                    }
                })
                .collect::<Vec<_>>();
            celestia_blobs::Entity::insert_many(active_models)
                .on_conflict(
                    OnConflict::column(celestia_blobs::Column::Id)
                        .update_columns([
                            celestia_blobs::Column::Data,
                            celestia_blobs::Column::DataS3ObjectKey,
                        ])
                        .to_owned(),
                )
                .exec(database)
                .await?;
        }
        DaLayer::Eigenda => {
            let active_models = blob_ids
                .into_iter()
                .map(|id| {
                    let object_key = blob_id_to_object_key(da_layer, &id);
                    eigenda_blobs::ActiveModel {
                        id: Set(id),
                        data: Set(None),
                        data_s3_object_key: Set(Some(object_key)),
                        ..Default::default()
                    }
                })
                .collect::<Vec<_>>();
            eigenda_blobs::Entity::insert_many(active_models)
                .on_conflict(
                    OnConflict::column(eigenda_blobs::Column::Id)
                        .update_columns([
                            eigenda_blobs::Column::Data,
                            eigenda_blobs::Column::DataS3ObjectKey,
                        ])
                        .to_owned(),
                )
                .exec(database)
                .await?;
        }
    }
    Ok(())
}

fn blob_id_to_object_key(da_layer: DaLayer, blob_id: &[u8]) -> String {
    format!("{}_{}", da_layer.s3_prefix(), hex::encode(blob_id))
}

async fn call_retriable<V, E, F, Fut>(function: F) -> Result<V, E>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<V, E>>,
    E: std::fmt::Debug,
{
    // I don't think we actually need any sophisticated timeout logic here,
    // so just hardcoded some sane values. In case it is not enough, should
    // probably be replaced with some library retry implementation which
    // supports exponential backoff and jitters.
    let timeouts: Vec<_> = [1, 3, 5].into_iter().map(Duration::from_secs).collect();
    let retries = timeouts.len() + 1;
    let mut attempt = 0;
    loop {
        attempt += 1;
        match function().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                if attempt == retries {
                    tracing::error!(
                        error = format!("{error:#?}"),
                        "no attempts left; request resulted in error"
                    );
                    return Err(error);
                }
                let timeout = timeouts[attempt - 1];
                tracing::warn!(
                    error = format!("{error:#?}"),
                    attempt = attempt,
                    timeout = timeout.as_secs(),
                    "attempt resulted in error; retrying.. "
                );
                tokio::time::sleep(timeout).await;
            }
        }
    }
}
