use std::sync::Arc;
use sea_orm::DatabaseConnection;
use tokio::sync::OnceCell;
use crate::logic::GithubClient;

static DATABASE: OnceCell<Arc<DatabaseConnection>> = OnceCell::const_new();

pub fn init_db_connection(db: Arc<DatabaseConnection>) {
    DATABASE.set(db).expect("database already initialized");
}

pub fn get_db_connection() -> Arc<DatabaseConnection> {
    DATABASE.get().expect("database not initialized").clone()
}

static GITHUB: OnceCell<Arc<GithubClient>> = OnceCell::const_new();

pub fn init_github_client(github: Arc<GithubClient>) {
    GITHUB.set(github).expect("github client already initialized");
}

pub fn get_github_client() -> Arc<GithubClient> {
    GITHUB.get().expect("github client not initialized").clone()
}

