//! code for talking to DB and etc. I suppose...

pub mod db_cache;
// todo: test only
pub mod in_memory_cache;
pub mod types;

#[trait_variant::make(CacheManager: Send)]
pub trait LocalCacheManager<K, V> {
    async fn insert(&self, key: K, value: V) -> Option<V>;
    async fn get(&self, key: &K) -> Option<V>;
    async fn remove(&self, key: &K) -> Option<V>;
}
