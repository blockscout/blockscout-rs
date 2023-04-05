use anyhow::Context;
use sea_orm::ConnectionTrait;
use sea_orm_migration::MigratorTrait;
use std::str::FromStr;

pub async fn initialize_postgres<Migrator: MigratorTrait>(
    db_url: &str,
    create_database: bool,
    run_migrations: bool,
) -> anyhow::Result<()> {
    // Create database if not exists
    if create_database {
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

        let db = sea_orm::Database::connect(db_base_url.to_string()).await?;
        // The problem is that PostgreSQL does not have `CREATE DATABASE IF NOT EXISTS` statement.
        // To create database only if it does not exist, we've used the following answer:
        // https://stackoverflow.com/a/55950456
        let create_database_statement = format!(
            r#"
            CREATE EXTENSION IF NOT EXISTS dblink;

            DO $$
            BEGIN
                PERFORM dblink_exec('', 'CREATE DATABASE {db_name}');
                EXCEPTION WHEN duplicate_database THEN RAISE NOTICE '%, skipping', SQLERRM USING ERRCODE = SQLSTATE;
            END$$;
        "#
        );
        db.execute_unprepared(create_database_statement.as_str())
            .await?;
    }

    let db = sea_orm::Database::connect(db_url).await?;
    if run_migrations {
        Migrator::up(&db, None).await?;
    }

    Ok(())
}
