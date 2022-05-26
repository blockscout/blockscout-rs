use anyhow::Result;
use async_trait::async_trait;
use semver::Version;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

#[async_trait]
pub trait Fetcher {
    async fn fetch(&self, ver: &Version) -> Result<PathBuf>;
}

pub struct DownloadCache<D> {
    cache: parking_lot::Mutex<HashMap<Version, Arc<tokio::sync::RwLock<Option<PathBuf>>>>>,
    downloader: D,
}

impl<D> DownloadCache<D> {
    pub fn new(downloader: D) -> Self {
        DownloadCache {
            cache: Default::default(),
            downloader,
        }
    }

    async fn try_get(&self, ver: &Version) -> Option<PathBuf> {
        let entry = {
            let cache = self.cache.lock();
            cache.get(ver).cloned()
        };
        match entry {
            Some(lock) => {
                let solc = lock.read().await;
                solc.as_ref().cloned()
            }
            None => None,
        }
    }
}

impl<D: Fetcher> DownloadCache<D> {
    pub async fn get(&self, ver: &Version) -> Result<PathBuf> {
        match self.try_get(ver).await {
            Some(solc) => Ok(solc),
            None => self.fetch(ver).await,
        }
    }

    async fn fetch(&self, ver: &Version) -> Result<PathBuf> {
        let lock = {
            let mut cache = self.cache.lock();
            Arc::clone(cache.entry(ver.clone()).or_default())
        };
        let mut entry = lock.write().await;
        match entry.as_ref() {
            Some(solc) => Ok(solc.clone()),
            None => {
                log::info!(target: "compiler_cache", "installing solc version {}", ver);
                let solc = self.downloader.fetch(ver).await?;
                *entry = Some(solc.clone());
                Ok(solc)
            }
        }
    }
}

impl<D: Default> Default for DownloadCache<D> {
    fn default() -> Self {
        DownloadCache::new(D::default())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use anyhow::anyhow;
    use futures::{executor::block_on, join, pin_mut};
    use tokio::{spawn, task::yield_now, time::timeout};

    #[test]
    fn caching() {
        struct MockFetcher {
            pub vals: HashMap<Version, PathBuf>,
        }

        #[async_trait]
        impl Fetcher for MockFetcher {
            async fn fetch(&self, ver: &Version) -> Result<PathBuf> {
                self.vals
                    .get(ver)
                    .cloned()
                    .ok_or(anyhow!("no mock result for this version: {}", ver))
            }
        }

        let vers: Vec<_> = vec![(0, 1, 2), (1, 2, 3), (3, 3, 3)]
            .into_iter()
            .map(|ver| Version::new(ver.0, ver.1, ver.2))
            .collect();
        let new_ver = Version::new(9, 9, 9);
        let fetcher = MockFetcher {
            vals: vers
                .iter()
                .map(|ver| (ver.clone(), PathBuf::from(ver.to_string())))
                .collect(),
        };
        let mut cache = DownloadCache::new(fetcher);

        for ver in &vers {
            assert_eq!(
                block_on(cache.get(ver)).expect("expected to find value but got error"),
                PathBuf::from(ver.to_string()),
                "wrong value"
            );
        }
        block_on(cache.get(&new_ver)).expect_err("expected err but got value");

        cache.downloader.vals = cache
            .downloader
            .vals
            .into_iter()
            .map(|(ver, _)| (ver, PathBuf::from("downloaded again")))
            .collect();

        for ver in &vers {
            assert_eq!(
                block_on(cache.get(ver)).expect("expected to find value but got error"),
                PathBuf::from(ver.to_string()),
                "value not cached"
            );
        }
        block_on(cache.get(&Version::new(9, 9, 9))).expect_err("expected err but got value");

        cache
            .downloader
            .vals
            .insert(new_ver.clone(), PathBuf::from("9.9.9"));
        assert_eq!(
            block_on(cache.get(&new_ver)).expect("expected to find value but got error"),
            PathBuf::from(new_ver.to_string()),
            "wrong value"
        );
    }

    #[tokio::test]
    async fn not_blocking() {
        const TIMEOUT: Duration = Duration::from_secs(10);

        struct MockFetcher {
            sync: Arc<tokio::sync::Mutex<()>>,
        }

        #[async_trait]
        impl Fetcher for MockFetcher {
            async fn fetch(&self, _: &Version) -> Result<PathBuf> {
                self.sync.lock().await;
                Ok(PathBuf::from("path"))
            }
        }

        let sync = Arc::<tokio::sync::Mutex<()>>::default();
        let fetcher = MockFetcher { sync: sync.clone() };
        let cache = Arc::new(DownloadCache::new(fetcher));

        let vers: Vec<_> = (0..3).map(|x| Version::new(x, 0, 0)).collect();

        // fill cache
        cache.get(&vers[1]).await.unwrap();

        // lock the fetcher
        let guard = sync.lock().await;

        // try to download (it will block on mutex)
        let handle = {
            let cache = cache.clone();
            let vers = vers.clone();
            spawn(async move { join!(cache.get(&vers[0]), cache.get(&vers[2])) })
        };
        // so we could rerun future after timeout
        pin_mut!(handle);
        // give the thread to the scheduler so it could run "handle" task
        yield_now().await;

        // check, that while we're downloading we don't block the cache
        timeout(TIMEOUT, cache.get(&vers[1]))
            .await
            .expect("should not block")
            .expect("expected value not error");

        // check, that we're blocked on downloading
        timeout(Duration::from_millis(100), &mut handle)
            .await
            .expect_err("should block");

        // release the lock
        std::mem::drop(guard);

        // now we can finish downloading
        let vals = timeout(TIMEOUT, handle)
            .await
            .expect("should not block")
            .unwrap();
        vals.0.expect("expected value got error");
        vals.1.expect("expected value got error");
    }
}
