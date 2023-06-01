use lazy_static::lazy_static;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, RwLock};

lazy_static! {
    pub static ref UPDATE_MUTEX: RwLock<HashMap<String, Arc<Mutex<()>>>> = RwLock::default();
}

pub async fn get_global_update_mutex(key: &str) -> Arc<Mutex<()>> {
    let maybe_mutex = UPDATE_MUTEX.read().await.get(key).cloned();
    match maybe_mutex {
        Some(mutex) => mutex,
        None => {
            let mut map = UPDATE_MUTEX.write().await;
            let mutex = Arc::new(Mutex::default());
            map.insert(key.to_owned(), mutex.clone());
            mutex
        }
    }
}
