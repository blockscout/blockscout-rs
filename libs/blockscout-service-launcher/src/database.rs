use anyhow::Context;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::serde_as;
use std::{str::FromStr, time::Duration};
use tracing::log::LevelFilter;

cfg_if::cfg_if! {
    if #[cfg(feature = "database-1_0")] {
        pub use sea_orm_1_0::{ConnectOptions, ConnectionTrait, Database, DatabaseBackend, Statement, DatabaseConnection, DbErr};
        pub use sea_orm_migration_1_0::MigratorTrait;
    } else if #[cfg(feature = "database-0_12")] {
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
            "one of the features ['database-1_0', 'database-0_12', 'database-0_11', 'database-0_10'] \
             must be enabled"
        );
    }
}

const DEFAULT_DB: &str = "postgres";

pub async fn initialize_postgres<Migrator: MigratorTrait>(
    settings: &DatabaseSettings,
) -> anyhow::Result<DatabaseConnection> {
    let db_url = settings.connect.clone().url();

    // Create database if not exists
    if settings.create_database {
        let (db_base_url, db_name) = {
            let mut db_url: url::Url = db_url.parse().context("invalid database url")?;
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

        let create_database_options = settings.connect_options.apply_to(db_base_url.into());
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

    let connect_options = settings.connect_options.apply_to(db_url.into());
    let db = Database::connect(connect_options).await?;
    if settings.run_migrations {
        Migrator::up(&db, None).await?;
    }

    Ok(db)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DatabaseSettings {
    pub connect: DatabaseConnectSettings,
    #[serde(default)]
    pub connect_options: DatabaseConnectOptionsSettings,
    #[serde(default)]
    pub create_database: bool,
    #[serde(default)]
    pub run_migrations: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
pub enum DatabaseConnectSettings {
    Url(String),
    Kv(DatabaseKvConnection),
}

impl DatabaseConnectSettings {
    pub fn url(self) -> String {
        match self {
            DatabaseConnectSettings::Url(s) => s,
            DatabaseConnectSettings::Kv(kv) => kv.url(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DatabaseKvConnection {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    #[serde(default)]
    pub dbname: Option<String>,
    #[serde(default)]
    pub options: Option<String>,
}

impl DatabaseKvConnection {
    pub fn url(self) -> String {
        let dbname = self
            .dbname
            .map(|dbname| format!("/{dbname}"))
            .unwrap_or_default();
        let options = self
            .options
            .map(|options| format!("?{options}"))
            .unwrap_or_default();
        format!(
            "postgresql://{}:{}@{}:{}{}{}",
            self.user, self.password, self.host, self.port, dbname, options
        )
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct DatabaseConnectOptionsSettings {
    /// Maximum number of connections for a pool
    pub max_connections: Option<u32>,
    /// Minimum number of connections for a pool
    pub min_connections: Option<u32>,
    /// The connection timeout for a packet connection
    pub connect_timeout: Option<Duration>,
    /// Maximum idle time for a particular connection to prevent
    /// network resource exhaustion
    pub idle_timeout: Option<Duration>,
    /// Set the maximum amount of time to spend waiting for acquiring a connection
    pub acquire_timeout: Option<Duration>,
    /// Set the maximum lifetime of individual connections
    pub max_lifetime: Option<Duration>,
    /// Enable SQLx statement logging
    pub sqlx_logging: bool,
    #[serde(
        deserialize_with = "string_to_level_filter",
        serialize_with = "level_filter_to_string"
    )]
    /// SQLx statement logging level (ignored if `sqlx_logging` is false)
    pub sqlx_logging_level: LevelFilter,
    #[cfg(feature = "database-1_0")]
    #[serde(
        deserialize_with = "string_to_level_filter",
        serialize_with = "level_filter_to_string"
    )]
    /// SQLx slow statements logging level (ignored if `sqlx_logging` is false)
    pub sqlx_slow_statements_logging_level: LevelFilter,
    #[cfg(feature = "database-1_0")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    /// SQLx slow statements duration threshold (ignored if `sqlx_logging` is false)
    pub sqlx_slow_statements_logging_threshold: Duration,
}

impl Default for DatabaseConnectOptionsSettings {
    fn default() -> Self {
        Self {
            max_connections: None,
            min_connections: None,
            connect_timeout: None,
            idle_timeout: None,
            acquire_timeout: None,
            max_lifetime: None,
            sqlx_logging: true,
            sqlx_logging_level: LevelFilter::Debug,
            #[cfg(feature = "database-1_0")]
            sqlx_slow_statements_logging_level: LevelFilter::Off,
            #[cfg(feature = "database-1_0")]
            sqlx_slow_statements_logging_threshold: Duration::from_secs(1),
        }
    }
}

impl DatabaseConnectOptionsSettings {
    fn apply_to(&self, mut options: ConnectOptions) -> ConnectOptions {
        if let Some(value) = self.max_connections {
            options.max_connections(value);
        }
        if let Some(value) = self.min_connections {
            options.min_connections(value);
        }
        if let Some(value) = self.connect_timeout {
            options.connect_timeout(value);
        }
        if let Some(value) = self.idle_timeout {
            options.idle_timeout(value);
        }
        if let Some(value) = self.acquire_timeout {
            options.acquire_timeout(value);
        }
        if let Some(value) = self.max_lifetime {
            options.max_lifetime(value);
        }
        options.sqlx_logging(self.sqlx_logging);
        options.sqlx_logging_level(self.sqlx_logging_level);
        #[cfg(feature = "database-1_0")]
        options.sqlx_slow_statements_logging_settings(
            self.sqlx_slow_statements_logging_level,
            self.sqlx_slow_statements_logging_threshold,
        );
        options
    }
}

fn string_to_level_filter<'de, D>(deserializer: D) -> Result<LevelFilter, D::Error>
where
    D: Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    LevelFilter::from_str(&string).map_err(<D::Error as serde::de::Error>::custom)
}

pub fn level_filter_to_string<S>(x: &LevelFilter, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(x.as_str().to_lowercase().as_str())
}
