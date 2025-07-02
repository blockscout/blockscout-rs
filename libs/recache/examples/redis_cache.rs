use recache::{handler::CacheHandler, stores::redis::RedisStore};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct A {
    a: i32,
    b: String,
}

impl A {
    pub fn new(a: i32, b: String) -> Self {
        Self { a, b }
    }
}

type RedisCacheHandler<V> = CacheHandler<RedisStore, String, V>;
static REFRESH_HOOK_COUNTER: AtomicUsize = AtomicUsize::new(0);

async fn call(cache: &RedisCacheHandler<A>) {
    let res = cache
        .request()
        .key("test")
        .ttl(cache.default_ttl)
        .maybe_refresh_ahead(cache.default_refresh_ahead)
        .execute(|| async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok::<A, String>(A::new(1, "2".to_string()))
        })
        .await
        .unwrap();
    println!("{res:?}");
}

#[tokio::main]
pub async fn main() {
    let cache = RedisStore::builder()
        .connection_string("redis://127.0.0.1:6379")
        .prefix("test")
        .build()
        .await
        .unwrap();

    let cache = CacheHandler::builder(Arc::new(cache))
        .on_refresh_computed(Arc::new(move |v| {
            REFRESH_HOOK_COUNTER.fetch_add(1, Ordering::Relaxed);
            println!("refreshed: {v:?}")
        }))
        .default_ttl(Duration::from_secs(10))
        .maybe_default_refresh_ahead(Some(Duration::from_secs(5)))
        .build();

    call(&cache).await;
    tokio::time::sleep(Duration::from_secs(6)).await;
    call(&cache).await;
    tokio::time::sleep(Duration::from_secs(6)).await;
    call(&cache).await;
}
