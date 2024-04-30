use crate::logic::GithubClient;
use sea_orm::DatabaseConnection;
use std::{fmt::Debug, sync::Arc};
use tokio::sync::{OnceCell, RwLock, RwLockReadGuard};

pub struct Global<T> {
    cell: OnceCell<RwLock<Arc<T>>>,
}

impl<T: Debug + Send + Sync + 'static> Global<T> {
    pub const fn new() -> Self {
        Self {
            cell: OnceCell::const_new(),
        }
    }

    pub async fn init(&self, value: Arc<T>) -> Result<(), anyhow::Error> {
        if let Some(lock) = self.cell.get() {
            *lock.write().await = value;
        } else {
            self.cell.set(RwLock::new(value))?;
        }
        Ok(())
    }

    pub async fn get(&self) -> RwLockReadGuard<Arc<T>> {
        self.cell.get().expect("value not initialized").read().await
    }
}

pub static DATABASE: Global<DatabaseConnection> = Global::new();

pub static GITHUB: Global<GithubClient> = Global::new();
