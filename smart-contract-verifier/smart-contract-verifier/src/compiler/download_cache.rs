// SPDX-License-Identifier: LicenseRef-Blockscout

use super::fetcher::{FetchError, Fetcher, Version};
use crate::metrics;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::Instrument;

#[derive(Clone)]
enum CachedFile {
    // Validation is deliberately deferred until first use so a remote compiler executor outage
    // cannot make service startup depend on probing every preloaded compiler.
    Unvalidated(PathBuf),
    Validated {
        path: PathBuf,
        // Preloaded compilers are copied out of their potentially writable source directory before
        // validation. Keeping the guard alive binds all later executions to those checked bytes.
        _snapshot: Option<Arc<tempfile::TempPath>>,
    },
}

pub struct DownloadCache<T> {
    cache: parking_lot::Mutex<HashMap<T, Arc<tokio::sync::RwLock<Option<CachedFile>>>>>,
}

impl<T> Default for DownloadCache<T> {
    fn default() -> Self {
        Self {
            cache: parking_lot::Mutex::new(HashMap::new()),
        }
    }
}

impl<Ver: Version> DownloadCache<Ver> {
    async fn try_get(&self, ver: &Ver) -> Option<PathBuf> {
        let entry = {
            let cache = self.cache.lock();
            cache.get(ver).cloned()
        };
        match entry {
            Some(lock) => {
                let file = lock.read().await;
                match file.as_ref() {
                    Some(CachedFile::Validated { path, .. }) => Some(path.clone()),
                    Some(CachedFile::Unvalidated(_)) | None => None,
                }
            }
            None => None,
        }
    }
}

impl<Ver: Version> DownloadCache<Ver> {
    pub async fn get<D: Fetcher<Version = Ver> + ?Sized>(
        &self,
        fetcher: &D,
        ver: &Ver,
    ) -> Result<PathBuf, FetchError> {
        metrics::DOWNLOAD_CACHE_TOTAL.inc();
        match self.try_get(ver).await {
            Some(file) => {
                metrics::DOWNLOAD_CACHE_HITS.inc();
                Ok(file)
            }
            None => {
                let _timer = metrics::COMPILER_FETCH_TIME.start_timer();
                let span = tracing::debug_span!("fetch compiler", ver = ver.to_string());
                self.fetch(fetcher, ver).instrument(span).await
            }
        }
    }

    async fn fetch<D: Fetcher<Version = Ver> + ?Sized>(
        &self,
        fetcher: &D,
        ver: &Ver,
    ) -> Result<PathBuf, FetchError> {
        let lock = {
            let mut cache = self.cache.lock();
            Arc::clone(cache.entry(ver.clone()).or_default())
        };
        let mut entry = lock.write().await;
        match entry.as_ref().cloned() {
            Some(CachedFile::Validated { path, .. }) => {
                metrics::DOWNLOAD_CACHE_HITS.inc();
                Ok(path)
            }
            Some(CachedFile::Unvalidated(file)) => {
                tracing::info!(target: "compiler_cache", "validating preloaded file version {}", ver);
                let validation: Result<_, FetchError> = async {
                    let (path, guard) = snapshot_preloaded_file(&file).await?;
                    fetcher.validate_file(ver, &path).await?;
                    Ok((path, guard))
                }
                .await;
                match validation {
                    Ok((path, guard)) => {
                        *entry = Some(CachedFile::Validated {
                            path: path.clone(),
                            _snapshot: Some(guard),
                        });
                        metrics::DOWNLOAD_CACHE_HITS.inc();
                        Ok(path)
                    }
                    Err(error) => {
                        tracing::warn!(
                            target: "compiler_cache",
                            version = %ver,
                            path = %file.display(),
                            error = ?error,
                            "preloaded compiler validation failed; downloading a verified copy"
                        );
                        let file = fetcher.fetch(ver).await?;
                        *entry = Some(CachedFile::Validated {
                            path: file.clone(),
                            _snapshot: None,
                        });
                        Ok(file)
                    }
                }
            }
            None => {
                tracing::info!(target: "compiler_cache", "installing file version {}", ver);
                let file = fetcher.fetch(ver).await?;
                *entry = Some(CachedFile::Validated {
                    path: file.clone(),
                    _snapshot: None,
                });
                Ok(file)
            }
        }
    }

    pub async fn load_from_dir(&self, dir: &PathBuf) -> std::io::Result<()> {
        let paths = read_dir_paths(dir)?;
        let versions = filter_versions(paths);
        self.add_versions(versions).await;
        Ok(())
    }

    async fn add_versions(&self, versions: HashMap<Ver, PathBuf>) {
        for (version, path) in versions {
            let solc_path = path.join("solc");
            let version_is_directory = std::fs::symlink_metadata(&path)
                .map(|metadata| metadata.file_type().is_dir())
                .unwrap_or(false);
            let compiler_is_file = std::fs::symlink_metadata(&solc_path)
                .map(|metadata| metadata.file_type().is_file())
                .unwrap_or(false);
            if version_is_directory && compiler_is_file {
                tracing::info!("found preloaded compiler candidate version {}", version);
                let lock = {
                    let mut cache = self.cache.lock();
                    Arc::clone(cache.entry(version.clone()).or_default())
                };
                let mut entry = lock.write().await;
                if entry.is_none() {
                    *entry = Some(CachedFile::Unvalidated(solc_path));
                }
            } else {
                tracing::warn!(
                    "ignoring preloaded compiler version {} because {:?} is not a regular file in a regular directory",
                    version,
                    solc_path
                );
            }
        }
    }
}

async fn snapshot_preloaded_file(
    source: &Path,
) -> Result<(PathBuf, Arc<tempfile::TempPath>), FetchError> {
    let source = source.to_path_buf();
    Ok(tokio::task::spawn_blocking(move || {
        let metadata = std::fs::symlink_metadata(&source)?;
        if !metadata.file_type().is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "preloaded compiler must be a regular file",
            ));
        }

        let mut source = std::fs::File::open(source)?;
        let mut snapshot = tempfile::NamedTempFile::new()?;
        std::io::copy(&mut source, &mut snapshot)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            snapshot
                .as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o500))?;
        }
        let guard = Arc::new(snapshot.into_temp_path());
        let path = guard.to_path_buf();
        Ok((path, guard))
    })
    .await??)
}

fn read_dir_paths(dir: &PathBuf) -> std::io::Result<impl Iterator<Item = PathBuf>> {
    let paths = std::fs::read_dir(dir)?.filter_map(|r| r.ok().map(|e| e.path()));
    Ok(paths)
}

fn filter_versions<Ver: Version>(dirs: impl Iterator<Item = PathBuf>) -> HashMap<Ver, PathBuf> {
    dirs.filter_map(|path| {
        let name = path.file_name()?.to_str()?;
        let version = Ver::from_str(name).ok()?;
        (version.to_string() == name).then_some((version, path))
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::{super::version_detailed as evm_version, *};
    use async_trait::async_trait;
    use futures::{executor::block_on, join, pin_mut};
    use pretty_assertions::assert_eq;
    use std::{
        collections::HashSet,
        str::FromStr,
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };
    use tokio::{spawn, task::yield_now, time::timeout};

    fn new_version(major: u64) -> evm_version::DetailedVersion {
        evm_version::DetailedVersion::Release(evm_version::ReleaseVersion {
            version: semver::Version::new(major, 0, 0),
            commit: "00010203".to_string(),
        })
    }

    /// Tests, that caching works, meaning that cache downloads each version only once
    #[test]
    fn value_is_cached() {
        #[derive(Default)]
        struct MockFetcher {
            counter: parking_lot::Mutex<HashMap<evm_version::DetailedVersion, u32>>,
        }

        #[async_trait]
        impl Fetcher for MockFetcher {
            type Version = evm_version::DetailedVersion;

            async fn fetch(&self, ver: &Self::Version) -> Result<PathBuf, FetchError> {
                *self.counter.lock().entry(ver.clone()).or_default() += 1;
                Ok(PathBuf::from(ver.to_string()))
            }

            fn all_versions(&self) -> Vec<Self::Version> {
                vec![]
            }
        }

        let fetcher = MockFetcher::default();
        let cache = DownloadCache::default();

        let vers: Vec<_> = (0..3).map(new_version).collect();

        let get_and_check = |ver: &evm_version::DetailedVersion| {
            let value = block_on(cache.get(&fetcher, ver)).unwrap();
            assert_eq!(value, PathBuf::from(ver.to_string()));
        };

        get_and_check(&vers[0]);
        get_and_check(&vers[1]);
        get_and_check(&vers[0]);
        get_and_check(&vers[0]);
        get_and_check(&vers[1]);
        get_and_check(&vers[1]);
        get_and_check(&vers[2]);
        get_and_check(&vers[2]);
        get_and_check(&vers[1]);
        get_and_check(&vers[0]);

        let counter = fetcher.counter.lock();
        assert_eq!(counter.len(), 3);
        assert!(counter.values().all(|&count| count == 1));
    }

    /// Tests, that cache will not block requests for already downloaded values,
    /// while it downloads others
    #[tokio::test]
    async fn downloading_not_blocks() {
        const TIMEOUT: Duration = Duration::from_secs(10);

        #[derive(Clone)]
        struct MockBlockingFetcher {
            sync: Arc<tokio::sync::Mutex<()>>,
        }

        #[async_trait]
        impl Fetcher for MockBlockingFetcher {
            type Version = evm_version::DetailedVersion;

            async fn fetch(&self, ver: &Self::Version) -> Result<PathBuf, FetchError> {
                let _guard = self.sync.lock().await;
                Ok(PathBuf::from(ver.to_string()))
            }

            fn all_versions(&self) -> Vec<Self::Version> {
                vec![]
            }
        }

        let sync = Arc::<tokio::sync::Mutex<()>>::default();
        let fetcher = MockBlockingFetcher { sync: sync.clone() };
        let cache = Arc::new(DownloadCache::default());

        let vers: Vec<_> = (0..3).map(new_version).collect();

        // fill the cache
        cache.get(&fetcher, &vers[1]).await.unwrap();

        // lock the fetcher
        let guard = sync.lock().await;

        // try to download (it will block on mutex)
        let handle = {
            let cache = cache.clone();
            let vers = vers.clone();
            let fetcher = fetcher.clone();
            spawn(
                async move { join!(cache.get(&fetcher, &vers[0]), cache.get(&fetcher, &vers[2])) },
            )
        };
        // so we could rerun future after timeout
        pin_mut!(handle);
        // give the thread to the scheduler so it could run "handle" task
        yield_now().await;

        // check, that while we're downloading we don't block the cache
        timeout(TIMEOUT, cache.get(&fetcher, &vers[1]))
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

    #[tokio::test]
    async fn filter_versions() {
        let versions: HashSet<evm_version::DetailedVersion> =
            vec![1, 2, 3, 4, 5].into_iter().map(new_version).collect();

        let paths = versions.iter().map(|v| v.to_string().into()).chain(vec![
            "some_random_dir".into(),
            ".".into(),
            "..".into(),
            "�0.7.0+commit.9e61f92b".into(),
        ]);

        let versions_map = super::filter_versions(paths);
        let filtered_versions = HashSet::from_iter(versions_map.into_keys());
        assert_eq!(versions, filtered_versions,);
    }

    #[tokio::test]
    async fn preloaded_compiler_is_validated_once_before_becoming_a_cache_hit() {
        let ver = evm_version::DetailedVersion::from_str("0.7.0+commit.9e61f92b").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let version_dir = dir.path().join(ver.to_string());
        std::fs::create_dir(&version_dir).unwrap();
        let preloaded_path = version_dir.join("solc");
        std::fs::write(&preloaded_path, b"trusted compiler").unwrap();

        struct PreloadFetcher {
            validation_calls: AtomicUsize,
            fetch_calls: AtomicUsize,
        }

        #[async_trait]
        impl Fetcher for PreloadFetcher {
            type Version = evm_version::DetailedVersion;

            async fn fetch(&self, _ver: &Self::Version) -> Result<PathBuf, FetchError> {
                self.fetch_calls.fetch_add(1, Ordering::Relaxed);
                panic!("valid preloaded compiler must not be downloaded")
            }

            async fn validate_file(
                &self,
                _ver: &Self::Version,
                path: &std::path::Path,
            ) -> Result<(), FetchError> {
                self.validation_calls.fetch_add(1, Ordering::Relaxed);
                assert_eq!(std::fs::read(path).unwrap(), b"trusted compiler");
                Ok(())
            }

            fn all_versions(&self) -> Vec<Self::Version> {
                vec![]
            }
        }

        let fetcher = PreloadFetcher {
            validation_calls: AtomicUsize::new(0),
            fetch_calls: AtomicUsize::new(0),
        };

        let cache = DownloadCache::default();
        cache
            .load_from_dir(&dir.path().to_path_buf())
            .await
            .expect("cannot load compilers");
        assert!(
            cache.try_get(&ver).await.is_none(),
            "preloaded compiler must remain quarantined until validation"
        );

        let snapshot_path = cache.get(&fetcher, &ver).await.unwrap();
        assert_ne!(snapshot_path, preloaded_path);
        assert_eq!(std::fs::read(&snapshot_path).unwrap(), b"trusted compiler");

        std::fs::write(&preloaded_path, b"changed after validation").unwrap();
        assert_eq!(cache.get(&fetcher, &ver).await.unwrap(), snapshot_path);
        assert_eq!(
            std::fs::read(&snapshot_path).unwrap(),
            b"trusted compiler",
            "cache hits must keep using the authenticated snapshot"
        );
        assert_eq!(fetcher.validation_calls.load(Ordering::Relaxed), 1);
        assert_eq!(fetcher.fetch_calls.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn invalid_preloaded_compiler_is_replaced_by_verified_fetch() {
        let ver = new_version(7);
        let dir = tempfile::tempdir().unwrap();
        let version_dir = dir.path().join(ver.to_string());
        std::fs::create_dir(&version_dir).unwrap();
        std::fs::write(version_dir.join("solc"), b"tampered compiler").unwrap();
        let downloaded_path = dir.path().join("verified-compiler");
        std::fs::write(&downloaded_path, b"verified compiler").unwrap();

        struct ReplacingFetcher {
            downloaded_path: PathBuf,
            validation_calls: AtomicUsize,
            fetch_calls: AtomicUsize,
        }

        #[async_trait]
        impl Fetcher for ReplacingFetcher {
            type Version = evm_version::DetailedVersion;

            async fn fetch(&self, _ver: &Self::Version) -> Result<PathBuf, FetchError> {
                self.fetch_calls.fetch_add(1, Ordering::Relaxed);
                Ok(self.downloaded_path.clone())
            }

            async fn validate_file(
                &self,
                _ver: &Self::Version,
                _path: &std::path::Path,
            ) -> Result<(), FetchError> {
                self.validation_calls.fetch_add(1, Ordering::Relaxed);
                Err(FetchError::Validation(anyhow::anyhow!("tampered")))
            }

            fn all_versions(&self) -> Vec<Self::Version> {
                vec![]
            }
        }

        let fetcher = ReplacingFetcher {
            downloaded_path: downloaded_path.clone(),
            validation_calls: AtomicUsize::new(0),
            fetch_calls: AtomicUsize::new(0),
        };
        let cache = DownloadCache::default();
        cache
            .load_from_dir(&dir.path().to_path_buf())
            .await
            .unwrap();

        assert_eq!(cache.get(&fetcher, &ver).await.unwrap(), downloaded_path);
        assert_eq!(cache.get(&fetcher, &ver).await.unwrap(), downloaded_path);
        assert_eq!(fetcher.validation_calls.load(Ordering::Relaxed), 1);
        assert_eq!(fetcher.fetch_calls.load(Ordering::Relaxed), 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn preload_rejects_symlinked_version_directories_and_compilers() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("solc"), b"compiler").unwrap();

        let symlinked_version = new_version(8);
        symlink(
            outside.path(),
            dir.path().join(symlinked_version.to_string()),
        )
        .unwrap();

        let symlinked_compiler = new_version(9);
        let version_dir = dir.path().join(symlinked_compiler.to_string());
        std::fs::create_dir(&version_dir).unwrap();
        symlink(outside.path().join("solc"), version_dir.join("solc")).unwrap();

        let cache = DownloadCache::default();
        cache
            .load_from_dir(&dir.path().to_path_buf())
            .await
            .unwrap();

        assert!(cache.try_get(&symlinked_version).await.is_none());
        assert!(cache.try_get(&symlinked_compiler).await.is_none());
    }
}
