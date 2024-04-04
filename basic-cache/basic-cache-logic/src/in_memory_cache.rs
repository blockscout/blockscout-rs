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

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Mutex poisoned")]
    PoisonedMutex,
    #[error("In-memory solution does not support multithreading")]
    BlockingMutex,
}

impl<T> From<std::sync::TryLockError<T>> for Error {
    fn from(value: std::sync::TryLockError<T>) -> Self {
        match value {
            std::sync::TryLockError::Poisoned(_) => Self::PoisonedMutex,
            std::sync::TryLockError::WouldBlock => Self::BlockingMutex,
        }
    }
}

impl<K, V> CacheManager<K, V> for HashMapCache<K, V>
where
    K: Eq + Hash + Send + Sync,
    V: Clone + Send + Sync,
{
    type Error = Error;

    async fn set(&self, key: K, value: V) -> Result<(), Self::Error> {
        self.replace(key, value).await?;
        Ok(())
    }

    async fn replace(&self, key: K, value: V) -> Result<Option<V>, Self::Error> {
        Ok(self.inner.try_lock()?.insert(key, value))
    }

    async fn get(&self, key: &K) -> Result<Option<V>, Self::Error> {
        Ok(self.inner.try_lock()?.get(key).cloned())
    }

    async fn remove(&self, key: &K) -> Result<Option<V>, Self::Error> {
        Ok(self.inner.try_lock()?.remove(key))
    }
}
