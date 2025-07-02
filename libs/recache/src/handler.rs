use crate::stores::AsyncCacheStore;
use bon::Builder;
use dashmap::DashMap;
use futures::{
    FutureExt,
    future::{BoxFuture, Shared},
};
use std::{cmp::Eq, fmt::Display, hash::Hash, sync::Arc, time::Duration};
use thiserror::Error;

type SharedFuture<V, E> = Shared<BoxFuture<'static, Result<V, E>>>;
type EventHandler<V> = Arc<dyn Fn(&V) + Send + Sync>;

#[derive(Builder)]
pub struct CacheHandler<C, K, V>
where
    C: AsyncCacheStore<K, V>,
    K: Sync + Hash + Eq,
{
    #[builder(start_fn)]
    cache: Arc<C>,
    #[builder(default)]
    inflight: Arc<DashMap<K, SharedFuture<V, CacheRequestError<C::Error>>>>,
    on_hit: Option<EventHandler<V>>,
    on_computed: Option<EventHandler<V>>,
    on_refresh_computed: Option<EventHandler<V>>,
    pub default_ttl: Duration,
    pub default_refresh_ahead: Option<Duration>,
}

impl<C, K, V> CacheHandler<C, K, V>
where
    C: AsyncCacheStore<K, V> + Send + Sync + 'static,
    K: Sync + Hash + Eq,
{
    pub fn request(&self) -> CacheRequestBuilder<C, K, V>
    where
        Self: Sized,
    {
        CacheRequest::builder(
            self.cache.clone(),
            self.inflight.clone(),
            self.on_hit.clone(),
            self.on_computed.clone(),
            self.on_refresh_computed.clone(),
        )
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("stringified error: {inner}")]
pub struct CachedError {
    inner: String,
}

impl CachedError {
    pub fn new<E: std::fmt::Display>(e: E) -> Self {
        Self {
            inner: e.to_string(),
        }
    }
}

#[derive(Builder)]
#[builder(finish_fn(vis = "", name = build_internal))]
pub struct CacheRequest<C, K, V>
where
    C: AsyncCacheStore<K, V>,
    K: Sync,
{
    #[builder(start_fn)]
    cache: Arc<C>,
    #[builder(start_fn)]
    inflight: Arc<DashMap<K, SharedFuture<V, CacheRequestError<C::Error>>>>,
    #[builder(start_fn)]
    on_hit: Option<EventHandler<V>>,
    #[builder(start_fn)]
    on_computed: Option<EventHandler<V>>,
    #[builder(start_fn)]
    on_refresh_computed: Option<EventHandler<V>>,
    #[builder(into)]
    key: K,
    ttl: Option<Duration>,
    refresh_ahead: Option<Duration>,
}

#[derive(Error, Debug, Clone)]
pub enum CacheRequestError<CacheError> {
    #[error("compute error: {0}")]
    ComputeError(String),
    #[error("cache error: {0}")]
    CacheError(#[from] CacheError),
}

impl<C, K, V> CacheRequest<C, K, V>
where
    C: AsyncCacheStore<K, V> + Send + Sync + 'static,
    C::Error: Send + Sync + Clone,
    K: Send + Sync + Clone + 'static + Hash + Eq,
    V: Send + Sync + Clone + 'static,
{
    pub async fn execute<F, Fut, E>(self, compute: F) -> Result<V, CacheRequestError<C::Error>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, E>> + Send + 'static,
        E: Display,
    {
        if let Some((val, maybe_ttl)) = self.cache.get_with_ttl(&self.key).await? {
            if let Some(on_hit) = &self.on_hit {
                on_hit(&val);
            }

            if let Some(value_ttl) = maybe_ttl {
                self.handle_refresh(compute, value_ttl);
            }
            return Ok(val);
        }

        if let Some(fut) = self.inflight.get(&self.key) {
            let res = fut.clone().await;
            return res;
        }

        let key = self.key.clone();
        let cache = self.cache;
        let ttl = self.ttl;

        let fut = compute();
        let fut_shared: SharedFuture<V, CacheRequestError<C::Error>> = async move {
            let val = fut
                .await
                .map_err(|e| CacheRequestError::ComputeError(e.to_string()))?;
            cache.set(&key, &val, ttl).await?;
            if let Some(on_computed) = self.on_computed {
                on_computed(&val);
            }
            Ok(val)
        }
        .boxed()
        .shared();

        self.inflight.insert(self.key.clone(), fut_shared.clone());
        let result = fut_shared.await;
        self.inflight.remove(&self.key);
        result
    }

    pub fn handle_refresh<F, Fut, E>(self, compute: F, value_ttl: Duration)
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, E>> + Send + 'static,
        E: Display,
    {
        if self.ttl.is_none() {
            return;
        }

        let refresh_threshold = match self.refresh_ahead {
            Some(refresh_threshold) => refresh_threshold,
            None => return,
        };

        if value_ttl >= refresh_threshold {
            return;
        }

        let entry = self.inflight.entry(self.key.clone());
        if let dashmap::Entry::Vacant(entry) = entry {
            let cache = Arc::clone(&self.cache);
            let key = self.key.clone();
            let fut = compute();
            let fut_shared: SharedFuture<V, CacheRequestError<C::Error>> = async move {
                let val = fut
                    .await
                    .map_err(|e| CacheRequestError::ComputeError(e.to_string()))?;
                cache.set(&key, &val, self.ttl).await?;
                if let Some(on_refresh_computed) = self.on_refresh_computed {
                    on_refresh_computed(&val);
                }
                Ok(val)
            }
            .boxed()
            .shared();

            entry.insert(fut_shared.clone());

            let inflight = self.inflight.clone();
            let key = self.key.clone();
            tokio::spawn(async move {
                let _ = fut_shared.await;
                inflight.remove(&key);
            });
        }
    }
}

impl<C, K, V, S: cache_request_builder::IsComplete> CacheRequestBuilder<C, K, V, S>
where
    C: AsyncCacheStore<K, V> + Send + Sync + 'static,
    C::Error: Send + Sync + Clone,
    K: Send + Sync + Clone + 'static + Hash + Eq,
    V: Send + Sync + Clone + 'static,
{
    pub async fn execute<F, Fut, E>(self, compute: F) -> Result<V, CacheRequestError<C::Error>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, E>> + Send + 'static,
        E: Display,
    {
        let req = self.build_internal();
        req.execute(compute).await
    }
}
