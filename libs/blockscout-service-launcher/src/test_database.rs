use crate::database::{
    ConnectionTrait, Database, DatabaseConnection, DbErr, MigratorTrait, Statement,
};
use anyhow::anyhow;
use async_dropper::derive::AsyncDrop;
use async_trait::async_trait;
use std::{ops::Deref, sync::Arc, time::Duration};

#[derive(AsyncDrop, Clone, Debug, Default)]
pub struct TestDbGuard {
    conn_with_db: Arc<DatabaseConnection>,
    base_db_url: String,
    db_name: String,
    should_drop: bool,
}

impl PartialEq<Self> for TestDbGuard {
    fn eq(&self, other: &Self) -> bool {
        self.base_db_url == other.base_db_url
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
        let base_db_url = std::env::var("DATABASE_URL")
            .expect("Database url must be set to initialize a test database");
        // We use a hash, as the name itself may be quite long and be trimmed.
        let db_name = format!("_{:x}", keccak_hash::keccak(db_name));
        let mut guard = TestDbGuard {
            conn_with_db: Arc::new(DatabaseConnection::Disconnected),
            base_db_url,
            db_name,
            should_drop: true,
        };

        guard.init_database::<Migrator>().await;
        guard
    }

    pub fn without_drop(mut self) -> Self {
        self.should_drop = false;
        self
    }

    pub fn client(&self) -> Arc<DatabaseConnection> {
        self.conn_with_db.clone()
    }

    pub fn db_url(&self) -> String {
        format!("{}/{}", self.base_db_url, self.db_name)
    }

    async fn init_database<Migrator: MigratorTrait>(&mut self) {
        // Create database
        let conn_without_db = Database::connect(&self.base_db_url)
            .await
            .expect("Connection to postgres (without database) failed");
        Self::drop_database(&conn_without_db, &self.db_name)
            .await
            .expect("Database drop failed");
        Self::create_database(&conn_without_db, &self.db_name)
            .await
            .expect("Database creation failed");

        // Migrate database
        let db_url = self.db_url();
        let conn_with_db = Database::connect(&db_url)
            .await
            .expect("Connection to postgres (with database) failed");
        Self::run_migrations::<Migrator>(&conn_with_db)
            .await
            .expect("Database migration failed");
        self.conn_with_db = Arc::new(conn_with_db);
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

    async fn drop_internal(&mut self) -> Result<(), anyhow::Error> {
        if self.should_drop {
            // Workaround for postgres error `cannot drop the currently open database`:
            // We need to create another connection without the database to drop it
            let conn_without_db = Database::connect(self.base_db_url.as_str())
                .await
                .map_err(|_| anyhow!("Failed to connect to the database"))?;
            TestDbGuard::drop_database(&conn_without_db, self.db_name.as_str())
                .await
                .map_err(|e| anyhow!("Failed to drop the database: {}", e))?;
        }
        Ok(())
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
        self.drop_internal().await.map_err(|e| {
            eprintln!("Failed to drop TestDbGuard: {:?}", e);
            AsyncDropError::UnexpectedError(e.into())
        })
    }

    fn drop_fail_action(&self) -> DropFailAction {
        DropFailAction::Panic
    }

    fn drop_timeout(&self) -> Duration {
        Duration::from_secs(1)
    }
}
