use crate::database::{
    ConnectionTrait, Database, DatabaseConnection, DbErr, MigratorTrait, Statement,
};
use async_dropper::derive::AsyncDrop;
use async_trait::async_trait;
use std::{ops::Deref, sync::Arc, time::Duration};

#[derive(AsyncDrop, Clone, Debug, Default)]
pub struct TestDbGuard {
    conn_with_db: Arc<DatabaseConnection>,
    db_url: String,
    db_name: String,
    should_drop: bool,
}

impl PartialEq<Self> for TestDbGuard {
    fn eq(&self, other: &Self) -> bool {
        self.db_url == other.db_url
            && self.db_name == other.db_name
            && matches!(
                (self.conn_with_db.deref(), other.conn_with_db.deref()),
                (
                    DatabaseConnection::Disconnected,
                    DatabaseConnection::Disconnected
                )
            )
    }
}

impl Eq for TestDbGuard {}

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
            db_name,
            should_drop: true,
        }
    }

    pub fn without_drop(mut self) -> Self {
        self.should_drop = false;
        self
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

#[async_trait]
impl AsyncDrop for TestDbGuard {
    async fn async_drop(&mut self) -> Result<(), AsyncDropError> {
        if self.should_drop {
            // Workaround for postgres error `cannot drop the currently open database`
            self.conn_with_db.drop();
            let conn_without_db = Database::connect(self.db_url())
                .await
                .expect("Connection to postgres (without database) failed");
            self.conn_with_db = Arc::new(conn_without_db);

            TestDbGuard::drop_database(self, self.db_name.as_str())
                .await
                .expect("Failed to drop database");
        }
        Ok(())
    }
}
