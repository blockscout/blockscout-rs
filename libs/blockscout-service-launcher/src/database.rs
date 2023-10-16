use anyhow::Context;
use std::str::FromStr;

cfg_if::cfg_if! {
    if #[cfg(feature = "database-0_12")] {
        pub use sea_orm_0_12::{ConnectOptions, ConnectionTrait, Database, DatabaseBackend, Statement, DatabaseConnection, DbErr};
        pub use sea_orm_migration_0_12::MigratorTrait;
    } else if #[cfg(feature = "database-0_11")] {
        pub use sea_orm_0_11::{ConnectOptions, ConnectionTrait, Database, DatabaseBackend, Statement, DatabaseConnection, DbErr};
        pub use sea_orm_migration_0_11::MigratorTrait;
    } else if #[cfg(feature = "database-0_10")] {
        pub use sea_orm_0_10::{ConnectOptions, ConnectionTrait, Database, DatabaseBackend, Statement, DatabaseConnection, DbErr};
        pub use sea_orm_migration_0_10::MigratorTrait;
    } else {
        compile_error!(
            "one of the features ['database-0_12', 'database-0_11', 'database-0_10'] \
             must be enabled"
        );
    }
}

const DEFAULT_DB: &str = "postgres";

pub async fn initialize_postgres<Migrator: MigratorTrait>(
    connect_options: impl Into<ConnectOptions>,
    create_database: bool,
    run_migrations: bool,
) -> anyhow::Result<DatabaseConnection> {
    let connect_options = connect_options.into();

    // Create database if not exists
    if create_database {
        let db_url = connect_options.get_url();
        let (db_base_url, db_name) = {
            let mut db_url = url::Url::from_str(db_url).context("invalid database url")?;
            let db_name = db_url
                .path_segments()
                .and_then(|mut segments| segments.next())
                .ok_or(anyhow::anyhow!("missing database name"))?
                .to_string();
            db_url.set_path("");
            if db_name.is_empty() {
                Err(anyhow::anyhow!("database name is empty"))?
            }
            (db_url, db_name)
        };
        tracing::info!("creating database '{db_name}'");
        let db_base_url = format!("{db_base_url}/{DEFAULT_DB}");

        let create_database_options = with_connect_options(db_base_url, &connect_options);
        let db = Database::connect(create_database_options).await?;

        let result = db
            .execute(Statement::from_string(
                DatabaseBackend::Postgres,
                format!(r#"CREATE DATABASE "{db_name}""#),
            ))
            .await;
        match result {
            Ok(_) => {
                tracing::info!("database '{db_name}' created");
            }
            Err(e) => {
                if e.to_string().contains("already exists") {
                    tracing::info!("database '{db_name}' already exists");
                } else {
                    return Err(anyhow::anyhow!(e));
                }
            }
        };
    }

    let db = Database::connect(connect_options).await?;
    if run_migrations {
        Migrator::up(&db, None).await?;
    }

    Ok(db)
}

fn with_connect_options(url: impl Into<String>, source_options: &ConnectOptions) -> ConnectOptions {
    let mut options = ConnectOptions::new(url.into());
    if let Some(value) = source_options.get_max_connections() {
        options.max_connections(value);
    }
    if let Some(value) = source_options.get_min_connections() {
        options.min_connections(value);
    }
    if let Some(value) = source_options.get_connect_timeout() {
        options.connect_timeout(value);
    }
    if let Some(value) = source_options.get_idle_timeout() {
        options.idle_timeout(value);
    }
    if let Some(value) = source_options.get_acquire_timeout() {
        options.acquire_timeout(value);
    }
    if let Some(value) = source_options.get_max_lifetime() {
        options.max_lifetime(value);
    }
    options.sqlx_logging(source_options.get_sqlx_logging());
    options.sqlx_logging_level(source_options.get_sqlx_logging_level());
    options
}
