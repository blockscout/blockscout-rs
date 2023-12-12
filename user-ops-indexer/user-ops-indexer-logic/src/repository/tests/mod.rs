use blockscout_service_launcher::test_database::TestDbGuard;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use std::sync::Arc;
use tokio::sync::Mutex;

static SHARED_DB_MUTEX: Mutex<Option<String>> = Mutex::const_new(None);
pub async fn get_shared_db() -> Arc<DatabaseConnection> {
    let mut initialized = SHARED_DB_MUTEX.lock().await;
    match initialized.clone() {
        None => {
            let client = init_db("shared").await;
            *initialized = Some(client.db_url());
            client.client()
        }
        Some(db_url) => Arc::new(Database::connect(db_url).await.unwrap()),
    }
}

pub async fn init_db(name: &str) -> TestDbGuard {
    let db = TestDbGuard::new::<migration::Migrator>(name).await;

    db.execute_unprepared(include_str!("blockscout_tables.sql"))
        .await
        .unwrap();

    db.execute_unprepared(include_str!("fixtures.sql"))
        .await
        .unwrap();

    db
}
