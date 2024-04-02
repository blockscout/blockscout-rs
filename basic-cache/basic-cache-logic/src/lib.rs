//! code for talking to DB and etc. I suppose...

mod db_cache;
mod in_memory_cache;

#[trait_variant::make(CacheManager: Send)]
trait LocalCacheManager<K, V> {
    async fn insert(&mut self, key: K, value: V) -> Option<V>;
    async fn get(&self, key: &K) -> Option<V>;
    async fn remove(&mut self, key: &K) -> Option<V>;
}
