use lazy_static::lazy_static;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

lazy_static! {
    pub static ref UPDATE_MUTEX: Mutex<HashMap<String, Arc<Mutex<()>>>> = Default::default();
}

pub async fn get_global_update_mutex(key: &str) -> Arc<Mutex<()>> {
    let mut map = UPDATE_MUTEX.lock().await;
    if let Some(mutex) = map.get(key) {
        Arc::clone(mutex)
    } else {
        let mutex = Arc::new(Mutex::new(()));
        map.insert(key.to_owned(), Arc::clone(&mutex));
        mutex
    }
}
