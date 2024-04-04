use std::fmt::Debug;

pub mod db_cache;
pub mod in_memory_cache;
pub mod types;

#[trait_variant::make(CacheManager: Send)]
pub trait LocalCacheManager<K, V> {
    type Error: Debug;

    // todo: provide default implementation through `replace()` when
    // https://github.com/rust-lang/impl-trait-utils/pull/20 is fixed
    async fn set(&self, key: K, value: V) -> Result<(), Self::Error>;
    async fn replace(&self, key: K, value: V) -> Result<Option<V>, Self::Error>;
    async fn get(&self, key: &K) -> Result<Option<V>, Self::Error>;
    async fn remove(&self, key: &K) -> Result<Option<V>, Self::Error>;
}
