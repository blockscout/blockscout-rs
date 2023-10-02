use migration::MigratorTrait;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr, Statement};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct TestDbGuard {
    base_db_name: String,
    base_db_url: String,
    db_conn: Arc<DatabaseConnection>,
    alliance_db_conn: Option<Arc<DatabaseConnection>>,
}

impl TestDbGuard {
    pub async fn new(base_db_name: &str, db_url: Option<String>) -> Self {
        let base_db_url = db_url.unwrap_or_else(|| {
            std::env::var("DATABASE_URL").expect(
                "Database url should either be provided explicitly, or \
            `DATABASE_URL` must be set to initialize a test database",
            )
        });
        let db_conn = Self::init_database::<migration::Migrator>(&base_db_url, base_db_name).await;

        TestDbGuard {
            base_db_name: base_db_name.to_string(),
            base_db_url,
            db_conn: Arc::new(db_conn),
            alliance_db_conn: None,
        }
    }

    pub async fn with_alliance_db(mut self) -> Self {
        if self.alliance_db_conn.is_none() {
            let db_name = format!("{}_alliance", self.base_db_name);
            let db_conn = Self::init_database::<verifier_alliance_migration::Migrator>(
                &self.base_db_url,
                &db_name,
            )
            .await;
            self.alliance_db_conn = Some(Arc::new(db_conn));
        }

        self
    }

    pub fn client(&self) -> Arc<DatabaseConnection> {
        self.db_conn.clone()
    }

    pub fn alliance_client(&self) -> Option<Arc<DatabaseConnection>> {
        self.alliance_db_conn.as_ref().cloned()
    }

    async fn init_database<Migrator: MigratorTrait>(
        base_db_url: &str,
        db_name: &str,
    ) -> DatabaseConnection {
        // Create database
        let conn_without_db = Database::connect(base_db_url)
            .await
            .expect("Connection to postgres (without database) failed");

        // We use a hash, as the name itself may be quite long and be trimmed.
        let hashed_db_name = format!("_{:x}", keccak_hash::keccak(db_name));

        Self::drop_database(&conn_without_db, &hashed_db_name)
            .await
            .expect("Database drop failed");
        Self::create_database(&conn_without_db, &hashed_db_name)
            .await
            .expect("Database creation failed");

        // Migrate database
        let db_url = format!("{base_db_url}/{hashed_db_name}");
        let conn_with_db = Database::connect(&db_url)
            .await
            .expect("Connection to postgres (with database) failed");
        Self::run_migrations::<Migrator>(&conn_with_db)
            .await
            .expect("Database migration failed");

        conn_with_db
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
