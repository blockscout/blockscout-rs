use anyhow::Context;
use blockscout_service_launcher::{
    database, database::DatabaseSettings, launcher::ConfigSettings, tracing::TracingSettings,
};
use da_indexer_logic::s3_storage::{S3Object, S3Storage, S3StorageSettings};
use migration::sea_orm::FromQueryResult;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
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
    let mut processed = 0;
    loop {
        iteration += 1;
        let blobs_to_migrate = call_retriable!(
            retrieve_blobs_to_migrate(&database, da_layer, settings.migration_batch_size)
                .await
                .context("retrieve blobs to migrate")?
        );
        let currently_processing = blobs_to_migrate.len();

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

        processed += currently_processing;
        if iteration % 100 == 0 {
            tracing::info!(iteration = iteration, processed = processed, "progress")
        }
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
    let mut sql_values = String::new();
    let mut bind_values = vec![];

    for (i, blob_id) in blob_ids.into_iter().enumerate() {
        if i > 0 {
            sql_values.push_str(", ");
        }
        sql_values.push_str(&format!("(${}, ${})", 2 * i + 1, 2 * i + 2));
        let object_key = blob_id_to_object_key(da_layer, &blob_id);
        bind_values.push(blob_id.into());
        bind_values.push(object_key.into());
    }

    let query = format!(
        r#"
        UPDATE {} as t SET
            data = NULL,
            data_s3_object_key = v.data_s3_object_key
        FROM (values {}) as v(id, data_s3_object_key)
        WHERE v.id = t.id;
    "#,
        da_layer.table_name(),
        sql_values
    );

    database
        .execute(Statement::from_sql_and_values(
            database.get_database_backend(),
            query,
            bind_values,
        ))
        .await?;

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
