use crate::stores::AsyncCacheStore;
use bon::bon;
use redis::{AsyncCommands, RedisError};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use thiserror::Error;

pub struct RedisStore {
    connection: redis::aio::ConnectionManager,
    prefix: String,
}

#[bon]
impl RedisStore {
    #[builder]
    pub async fn new(
        connection_string: impl Into<String>,
        prefix: impl Into<String>,
    ) -> Result<Self, RedisError> {
        let client = redis::Client::open(connection_string.into())?;
        let connection = redis::aio::ConnectionManager::new(client).await?;

        let prefix = prefix.into();

        Ok(Self { connection, prefix })
    }

    fn format_key(&self, key: &str) -> String {
        format!("{}:{}", self.prefix, key)
    }
}

#[derive(Error, Debug, Clone)]
pub enum RedisStoreError {
    #[error("redis error")]
    RedisCacheError(#[from] Arc<redis::RedisError>),
    #[error("error deserializing cached value {cached_value:?}: {error:?}")]
    CacheDeserializationError {
        cached_value: String,
        error: Arc<serde_json::Error>,
    },
    #[error("error serializing cached value: {error:?}")]
    CacheSerializationError { error: Arc<serde_json::Error> },
}

#[async_trait::async_trait]
impl<V> AsyncCacheStore<String, V> for RedisStore
where
    V: Send + Sync + Serialize + for<'de> Deserialize<'de>,
{
    type Error = RedisStoreError;

    async fn get(&self, key: &String) -> Result<Option<V>, Self::Error> {
        let mut conn = self.connection.clone();
        let val: Option<String> = conn.get(self.format_key(key)).await.map_err(Arc::new)?;
        match val {
            Some(s) => {
                let val = serde_json::from_str(&s).map_err(|e| {
                    RedisStoreError::CacheDeserializationError {
                        cached_value: s,
                        error: Arc::new(e),
                    }
                })?;
                Ok(Some(val))
            }
            None => Ok(None),
        }
    }

    async fn get_with_ttl(
        &self,
        key: &String,
    ) -> Result<Option<(V, Option<Duration>)>, Self::Error> {
        let script =
            redis::Script::new(r#"return {redis.call("GET",KEYS[1]),redis.call("PTTL",KEYS[1])}"#);
        let mut conn = self.connection.clone();

        let (val, ttl): (Option<String>, i64) = script
            .key(self.format_key(key))
            .invoke_async(&mut conn)
            .await
            .map_err(Arc::new)?;

        match val {
            Some(json) => {
                let value = serde_json::from_str(&json).map_err(|e| {
                    RedisStoreError::CacheDeserializationError {
                        cached_value: json,
                        error: Arc::new(e),
                    }
                })?;
                let ttl_duration = if ttl >= 0 {
                    Some(Duration::from_millis(ttl as u64))
                } else {
                    None
                };
                Ok(Some((value, ttl_duration)))
            }
            None => Ok(None),
        }
    }

    async fn set(&self, key: &String, value: &V, ttl: Option<Duration>) -> Result<(), Self::Error> {
        let mut conn = self.connection.clone();
        let json = serde_json::to_string(value)
            .map_err(|e| RedisStoreError::CacheSerializationError { error: Arc::new(e) })?;
        let redis_key = self.format_key(key);

        match ttl {
            Some(ttl) => {
                let _: () = conn
                    .set_ex(redis_key, json, ttl.as_secs())
                    .await
                    .map_err(Arc::new)?;
            }
            None => {
                let _: () = conn.set(redis_key, json).await.map_err(Arc::new)?;
            }
        }
        Ok(())
    }

    async fn delete(&self, key: &String) -> Result<(), Self::Error> {
        let mut conn = self.connection.clone();
        let _: () = conn.del(self.format_key(key)).await.map_err(Arc::new)?;
        Ok(())
    }
}
