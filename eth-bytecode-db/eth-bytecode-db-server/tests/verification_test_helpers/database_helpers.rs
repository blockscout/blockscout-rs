#![allow(dead_code)]

use migration::MigratorTrait;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr, Statement};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct TestDbGuard {
    conn_with_db: Arc<DatabaseConnection>,
    db_url: String,
}

impl TestDbGuard {
    pub async fn new(db_name: &str, db_url: Option<String>) -> Self {
        let db_url = db_url.unwrap_or_else(|| {
            std::env::var("DATABASE_URL").expect(
                "Database url should either be provided explicitly, or \
            `DATABASE_URL` must be set to initialize a test database",
            )
        });

        // Create database
        let conn_without_db = Database::connect(&db_url)
            .await
            .expect("Connection to postgres (without database) failed");
        Self::drop_database(&conn_without_db, db_name)
            .await
            .expect("Database drop failed");
        Self::create_database(&conn_without_db, db_name)
            .await
            .expect("Database creation failed");

        // Migrate database
        let db_url = format!("{}/{}", db_url, db_name);
        let conn_with_db = Database::connect(&db_url)
            .await
            .expect("Connection to postgres (with database) failed");
        Self::run_migrations(&conn_with_db)
            .await
            .expect("Database migration failed");

        TestDbGuard {
            conn_with_db: Arc::new(conn_with_db),
            db_url,
        }
    }

    pub fn client(&self) -> Arc<DatabaseConnection> {
        self.conn_with_db.clone()
    }

    pub fn db_url(&self) -> &str {
        &self.db_url
    }

    async fn create_database(db: &DatabaseConnection, db_name: &str) -> Result<(), DbErr> {
        tracing::info!(name = db_name, "creating database");
        db.execute(Statement::from_string(
            db.get_database_backend(),
            format!("CREATE DATABASE {}", db_name),
        ))
        .await?;
        Ok(())
    }

    async fn drop_database(db: &DatabaseConnection, db_name: &str) -> Result<(), DbErr> {
        tracing::info!(name = db_name, "dropping database");
        db.execute(Statement::from_string(
            db.get_database_backend(),
            format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", db_name),
        ))
        .await?;
        Ok(())
    }

    async fn run_migrations(db: &DatabaseConnection) -> Result<(), DbErr> {
        <migration::Migrator as MigratorTrait>::up(db, None).await
    }
}
