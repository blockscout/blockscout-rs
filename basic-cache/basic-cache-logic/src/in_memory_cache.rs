use std::{collections::HashMap, hash::Hash};

use crate::CacheManager;

#[derive(Debug, Default, Clone)]
struct HashMapCache<K, V> {
    inner: HashMap<K, V>,
}

impl<K, V> From<HashMap<K, V>> for HashMapCache<K, V> {
    fn from(value: HashMap<K, V>) -> Self {
        Self { inner: value }
    }
}

impl<K, V> CacheManager<K, V> for HashMapCache<K, V>
where
    K: Eq + Hash + Send + Sync,
    V: Clone + Send + Sync,
{
    async fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.inner.insert(key, value)
    }

    async fn get(&self, key: &K) -> Option<V> {
        self.inner.get(key).cloned()
    }

    async fn remove(&mut self, key: &K) -> Option<V> {
        self.inner.remove(&key)
    }
}
