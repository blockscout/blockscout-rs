use crate::database::{
    ConnectionTrait, Database, DatabaseConnection, DbErr, MigratorTrait, Statement,
};
use std::{ops::Deref, sync::Arc};

#[derive(Clone, Debug)]
pub struct TestDbGuard {
    conn_with_db: Arc<DatabaseConnection>,
    conn_without_db: Arc<DatabaseConnection>,
    base_db_url: String,
    db_name: String,
}

impl TestDbGuard {
    pub async fn new<Migrator: MigratorTrait>(db_name: &str) -> Self {
        let base_db_url = std::env::var("DATABASE_URL")
            .expect("Database url must be set to initialize a test database");
        let conn_without_db = Database::connect(&base_db_url)
            .await
            .expect("Connection to postgres (without database) failed");
        // We use a hash, as the name itself may be quite long and be trimmed.
        let db_name = format!("_{:x}", keccak_hash::keccak(db_name));
        let mut guard = TestDbGuard {
            conn_with_db: Arc::new(DatabaseConnection::Disconnected),
            conn_without_db: Arc::new(conn_without_db),
            base_db_url,
            db_name,
        };

        guard.init_database().await;
        guard.run_migrations::<Migrator>().await;
        guard
    }

    pub fn client(&self) -> Arc<DatabaseConnection> {
        self.conn_with_db.clone()
    }

    pub fn db_url(&self) -> String {
        format!("{}/{}", self.base_db_url, self.db_name)
    }

    async fn init_database(&mut self) {
        // Create database
        self.drop_database().await;
        self.create_database().await;

        let db_url = self.db_url();
        let conn_with_db = Database::connect(&db_url)
            .await
            .expect("Connection to postgres (with database) failed");
        self.conn_with_db = Arc::new(conn_with_db);
    }

    pub async fn drop_database(&self) {
        Self::drop_database_internal(&self.conn_without_db, &self.db_name)
            .await
            .expect("Database drop failed");
    }

    async fn create_database(&self) {
        Self::create_database_internal(&self.conn_without_db, &self.db_name)
            .await
            .expect("Database creation failed");
    }

    async fn create_database_internal(db: &DatabaseConnection, db_name: &str) -> Result<(), DbErr> {
        tracing::info!(name = db_name, "creating database");
        db.execute(Statement::from_string(
            db.get_database_backend(),
            format!("CREATE DATABASE {db_name}"),
        ))
        .await?;
        Ok(())
    }

    async fn drop_database_internal(db: &DatabaseConnection, db_name: &str) -> Result<(), DbErr> {
        tracing::info!(name = db_name, "dropping database");
        db.execute(Statement::from_string(
            db.get_database_backend(),
            format!("DROP DATABASE IF EXISTS {db_name} WITH (FORCE)"),
        ))
        .await?;
        Ok(())
    }

    async fn run_migrations<Migrator: MigratorTrait>(&self) {
        Migrator::up(self.conn_with_db.as_ref(), None)
            .await
            .expect("Database migration failed");
    }
}

impl Deref for TestDbGuard {
    type Target = DatabaseConnection;
    fn deref(&self) -> &Self::Target {
        &self.conn_with_db
    }
}
