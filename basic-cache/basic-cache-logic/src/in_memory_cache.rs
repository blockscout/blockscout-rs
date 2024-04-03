use std::{collections::HashMap, hash::Hash, sync::Mutex};

use crate::CacheManager;

#[derive(Debug)]
pub struct HashMapCache<K, V> {
    // need interior mutability because usual work with DB is done
    // via immutable reference
    inner: Mutex<HashMap<K, V>>,
}

impl<K, V> Default for HashMapCache<K, V> {
    fn default() -> Self {
        Self {
            inner: Default::default(),
        }
    }
}

impl<K, V> From<HashMap<K, V>> for HashMapCache<K, V> {
    fn from(value: HashMap<K, V>) -> Self {
        Self {
            inner: Mutex::new(value),
        }
    }
}

impl<K, V> CacheManager<K, V> for HashMapCache<K, V>
where
    K: Eq + Hash + Send + Sync,
    V: Clone + Send + Sync,
{
    async fn insert(&self, key: K, value: V) -> Option<V> {
        // test-only code, ok to unwrap
        self.inner.try_lock().unwrap().insert(key, value)
    }

    async fn get(&self, key: &K) -> Option<V> {
        // test-only code, ok to unwrap
        self.inner.try_lock().unwrap().get(key).cloned()
    }

    async fn remove(&self, key: &K) -> Option<V> {
        // test-only code, ok to unwrap
        self.inner.try_lock().unwrap().remove(&key)
    }
}
