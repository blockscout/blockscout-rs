use crate::database::{
    ConnectionTrait, Database, DatabaseConnection, DbErr, MigratorTrait, Statement,
};
use std::{ops::Deref, sync::Arc};

#[derive(Clone, Debug)]
pub struct TestDbGuard {
    conn_with_db: Arc<DatabaseConnection>,
    db_url: String,
}

impl TestDbGuard {
    pub async fn new<Migrator: MigratorTrait>(db_name: &str) -> Self {
        let db_url = std::env::var("DATABASE_URL")
            .expect("Database url must be set to initialize a test database");
        // We use a hash, as the name itself may be quite long and be trimmed.
        let db_name = format!("_{:x}", keccak_hash::keccak(db_name));
        let (db_url, conn_with_db) = Self::init_database::<Migrator>(&db_url, &db_name).await;

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

    async fn init_database<Migrator: MigratorTrait>(
        base_db_url: &str,
        db_name: &str,
    ) -> (String, DatabaseConnection) {
        // Create database
        let conn_without_db = Database::connect(base_db_url)
            .await
            .expect("Connection to postgres (without database) failed");
        Self::drop_database(&conn_without_db, db_name)
            .await
            .expect("Database drop failed");
        Self::create_database(&conn_without_db, db_name)
            .await
            .expect("Database creation failed");

        // Migrate database
        let db_url = format!("{base_db_url}/{db_name}");
        let conn_with_db = Database::connect(&db_url)
            .await
            .expect("Connection to postgres (with database) failed");
        Self::run_migrations::<Migrator>(&conn_with_db)
            .await
            .expect("Database migration failed");

        (db_url, conn_with_db)
    }

    async fn create_database(db: &DatabaseConnection, db_name: &str) -> Result<(), DbErr> {
        tracing::info!(name = db_name, "creating database");
        db.execute(Statement::from_string(
            db.get_database_backend(),
            format!("CREATE DATABASE {db_name}"),
        ))
        .await?;
        Ok(())
    }

    async fn drop_database(db: &DatabaseConnection, db_name: &str) -> Result<(), DbErr> {
        tracing::info!(name = db_name, "dropping database");
        db.execute(Statement::from_string(
            db.get_database_backend(),
            format!("DROP DATABASE IF EXISTS {db_name} WITH (FORCE)"),
        ))
        .await?;
        Ok(())
    }

    async fn run_migrations<Migrator: MigratorTrait>(db: &DatabaseConnection) -> Result<(), DbErr> {
        Migrator::up(db, None).await
    }
}

impl Deref for TestDbGuard {
    type Target = DatabaseConnection;
    fn deref(&self) -> &Self::Target {
        &self.conn_with_db
    }
}
