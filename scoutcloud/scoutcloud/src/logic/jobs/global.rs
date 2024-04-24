use crate::logic::GithubClient;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::OnceCell;

static DATABASE: OnceCell<Arc<DatabaseConnection>> = OnceCell::const_new();

pub fn init_db_connection(db: Arc<DatabaseConnection>) -> Result<(), anyhow::Error> {
    DATABASE.set(db)?;
    Ok(())
}

pub fn get_db_connection() -> Arc<DatabaseConnection> {
    DATABASE.get().expect("database not initialized").clone()
}

static GITHUB: OnceCell<Arc<GithubClient>> = OnceCell::const_new();

pub fn init_github_client(github: Arc<GithubClient>) -> Result<(), anyhow::Error> {
    GITHUB.set(github)?;
    Ok(())
}

pub fn get_github_client() -> Arc<GithubClient> {
    GITHUB.get().expect("github client not initialized").clone()
}
