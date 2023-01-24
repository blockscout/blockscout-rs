use migration::MigratorTrait;
use sea_orm::{prelude::*, ConnectionTrait, Database, Statement};
use std::{collections::HashSet, sync::Mutex};
use url::Url;

pub async fn init_db<M: MigratorTrait>(name: &str, db_url: Option<String>) -> DatabaseConnection {
    lazy_static::lazy_static! {
        static ref DB_NAMES: Mutex<HashSet<String>> = Default::default();
    }

    let db_not_created = {
        let mut guard = DB_NAMES.lock().unwrap();
        guard.insert(name.to_owned())
    };
    assert!(db_not_created, "db with name {} already was created", name);

    let db_url =
        db_url.unwrap_or_else(|| std::env::var("DATABASE_URL").expect("no DATABASE_URL env"));
    let url = Url::parse(&db_url).expect("unvalid database url");
    let db_url = url.join("/").unwrap().to_string();
    let raw_conn = Database::connect(db_url)
        .await
        .expect("failed to connect to postgres");

    raw_conn
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", name),
        ))
        .await
        .expect("failed to drop test database");
    raw_conn
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!("CREATE DATABASE {}", name),
        ))
        .await
        .expect("failed to create test database");

    let db_url = url.join(&format!("/{name}")).unwrap().to_string();
    let conn = Database::connect(db_url.clone())
        .await
        .expect("failed to connect to test db");
    M::up(&conn, None).await.expect("failed to run migrations");

    conn
}

pub async fn init_db_all(
    name: &str,
    db_url: Option<String>,
) -> (DatabaseConnection, DatabaseConnection) {
    let db = init_db::<migration::Migrator>(name, db_url.clone()).await;
    let blockscout =
        init_db::<blockscout_db::migration::Migrator>(&(name.to_owned() + "_blockscout"), db_url)
            .await;
    (db, blockscout)
}
