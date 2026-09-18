// SPDX-License-Identifier: LicenseRef-Blockscout

use super::fetcher::{FetchError, Fetcher, Version};
use crate::metrics;
use std::{
    collections::{hash_map::Entry, HashMap},
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
        // Preloaded compilers that another user could change are copied out of their source
        // directory before validation. Keeping the guard alive binds later executions to those
        // checked bytes.
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
                    let (path, guard) = stable_file(&file).await?;
                    fetcher.validate_file(ver, &path).await?;
                    Ok((path, guard))
                }
                .await;
                match validation {
                    Ok((path, guard)) => {
                        *entry = Some(CachedFile::Validated {
                            path: path.clone(),
                            _snapshot: guard,
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
                        let (path, snapshot) = Self::fetch_stable(fetcher, ver).await?;
                        *entry = Some(CachedFile::Validated {
                            path: path.clone(),
                            _snapshot: snapshot,
                        });
                        Ok(path)
                    }
                }
            }
            None => {
                tracing::info!(target: "compiler_cache", "installing file version {}", ver);
                let (path, snapshot) = Self::fetch_stable(fetcher, ver).await?;
                *entry = Some(CachedFile::Validated {
                    path: path.clone(),
                    _snapshot: snapshot,
                });
                Ok(path)
            }
        }
    }

    /// Downloads a compiler and pins the bytes later jobs use, as for preloaded compilers. A file
    /// other users could change is snapshotted, and the snapshot is validated again because the
    /// download may have been replaced between the fetcher's validation and the copy.
    async fn fetch_stable<D: Fetcher<Version = Ver> + ?Sized>(
        fetcher: &D,
        ver: &Ver,
    ) -> Result<(PathBuf, Option<Arc<tempfile::TempPath>>), FetchError> {
        let file = fetcher.fetch(ver).await?;
        let (path, snapshot) = stable_file(&file).await?;
        if snapshot.is_some() {
            fetcher.validate_file(ver, &path).await?;
        }
        Ok((path, snapshot))
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

/// Returns a path whose bytes cannot change after validation: the compiler file itself when only
/// this user or root can change it, otherwise a private snapshot kept alive by the returned guard.
async fn stable_file(
    source: &Path,
) -> Result<(PathBuf, Option<Arc<tempfile::TempPath>>), FetchError> {
    let source = source.to_path_buf();
    Ok(tokio::task::spawn_blocking(move || {
        let metadata = std::fs::symlink_metadata(&source)?;
        if !metadata.file_type().is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "preloaded compiler must be a regular file",
            ));
        }
        // Check and return the resolved path: a symlink along the configured path could be
        // retargeted after validation by whoever controls it.
        let canonical = std::fs::canonicalize(&source)?;
        if only_this_user_or_root_can_change(&canonical)? {
            return Ok((canonical, None));
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
        Ok((path, Some(guard)))
    })
    .await??)
}

/// Whether neither `path` nor any directory leading to it can be changed or replaced by a user
/// other than this process's user or root. `path` must be canonical, so no symlink is skipped.
#[cfg(unix)]
fn only_this_user_or_root_can_change(path: &Path) -> std::io::Result<bool> {
    use std::os::unix::fs::MetadataExt;

    // SAFETY: `geteuid` has no preconditions and always succeeds.
    let euid = unsafe { libc::geteuid() };
    for component in path.ancestors() {
        let metadata = std::fs::metadata(component)?;
        let trusted_owner = metadata.uid() == euid || metadata.uid() == 0;
        // Other users may add entries to a sticky directory, but cannot rename or remove ours.
        let sticky_directory = metadata.is_dir() && metadata.mode() & 0o1000 != 0;
        let writable_by_others = metadata.mode() & 0o022 != 0 && !sticky_directory;
        if !trusted_owner || writable_by_others {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(not(unix))]
fn only_this_user_or_root_can_change(_path: &Path) -> std::io::Result<bool> {
    Ok(false)
}

fn read_dir_paths(dir: &PathBuf) -> std::io::Result<impl Iterator<Item = PathBuf>> {
    let paths = std::fs::read_dir(dir)?.filter_map(|r| r.ok().map(|e| e.path()));
    Ok(paths)
}

/// Keeps every directory whose name parses as a version. When several names parse to the same
/// version (e.g. with and without the `v` prefix), the canonical one wins.
fn filter_versions<Ver: Version>(dirs: impl Iterator<Item = PathBuf>) -> HashMap<Ver, PathBuf> {
    let mut versions = HashMap::new();
    for path in dirs {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Ok(version) = Ver::from_str(name) else {
            continue;
        };
        let canonical = version.to_string() == name;
        match versions.entry(version) {
            Entry::Vacant(entry) => {
                entry.insert(path);
            }
            Entry::Occupied(mut entry) if canonical => {
                entry.insert(path);
            }
            Entry::Occupied(_) => {}
        }
    }
    versions
}

#[cfg(test)]
mod tests {
    use super::{super::version_detailed as evm_version, *};
    use async_trait::async_trait;
    use futures::{join, pin_mut};
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

    /// Writes a downloaded compiler into a private test directory.
    fn downloaded_file(dir: &Path, ver: &evm_version::DetailedVersion) -> PathBuf {
        let path = dir.join(ver.to_string());
        std::fs::write(&path, b"compiler").unwrap();
        path
    }

    /// Tests, that caching works, meaning that cache downloads each version only once
    #[tokio::test]
    async fn value_is_cached() {
        struct MockFetcher {
            dir: tempfile::TempDir,
            counter: parking_lot::Mutex<HashMap<evm_version::DetailedVersion, u32>>,
        }

        #[async_trait]
        impl Fetcher for MockFetcher {
            type Version = evm_version::DetailedVersion;

            async fn fetch(&self, ver: &Self::Version) -> Result<PathBuf, FetchError> {
                *self.counter.lock().entry(ver.clone()).or_default() += 1;
                Ok(downloaded_file(self.dir.path(), ver))
            }

            fn all_versions(&self) -> Vec<Self::Version> {
                vec![]
            }
        }

        let fetcher = MockFetcher {
            dir: tempfile::tempdir().unwrap(),
            counter: Default::default(),
        };
        let cache = DownloadCache::default();

        let vers: Vec<_> = (0..3).map(new_version).collect();

        for index in [0, 1, 0, 0, 1, 1, 2, 2, 1, 0] {
            let ver = &vers[index];
            let value = cache.get(&fetcher, ver).await.unwrap();
            let expected = std::fs::canonicalize(fetcher.dir.path().join(ver.to_string()));
            assert_eq!(value, expected.unwrap());
        }

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
            dir: Arc<tempfile::TempDir>,
            sync: Arc<tokio::sync::Mutex<()>>,
        }

        #[async_trait]
        impl Fetcher for MockBlockingFetcher {
            type Version = evm_version::DetailedVersion;

            async fn fetch(&self, ver: &Self::Version) -> Result<PathBuf, FetchError> {
                let _guard = self.sync.lock().await;
                Ok(downloaded_file(self.dir.path(), ver))
            }

            fn all_versions(&self) -> Vec<Self::Version> {
                vec![]
            }
        }

        let sync = Arc::<tokio::sync::Mutex<()>>::default();
        let fetcher = MockBlockingFetcher {
            dir: Arc::new(tempfile::tempdir().unwrap()),
            sync: sync.clone(),
        };
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
    async fn shared_preloaded_compiler_is_snapshotted_and_validated_once() {
        use std::os::unix::fs::PermissionsExt;

        let ver = evm_version::DetailedVersion::from_str("0.7.0+commit.9e61f92b").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let version_dir = dir.path().join(ver.to_string());
        std::fs::create_dir(&version_dir).unwrap();
        // Another user could replace the compiler in a world-writable directory.
        std::fs::set_permissions(&version_dir, std::fs::Permissions::from_mode(0o777)).unwrap();
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

        let downloaded_path = std::fs::canonicalize(&downloaded_path).unwrap();
        assert_eq!(cache.get(&fetcher, &ver).await.unwrap(), downloaded_path);
        assert_eq!(cache.get(&fetcher, &ver).await.unwrap(), downloaded_path);
        assert_eq!(fetcher.validation_calls.load(Ordering::Relaxed), 1);
        assert_eq!(fetcher.fetch_calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn downloaded_compiler_in_a_shared_directory_is_snapshotted_and_revalidated() {
        use std::os::unix::fs::PermissionsExt;

        struct SharedDirectoryFetcher {
            dir: PathBuf,
            validation_calls: AtomicUsize,
        }

        #[async_trait]
        impl Fetcher for SharedDirectoryFetcher {
            type Version = evm_version::DetailedVersion;

            async fn fetch(&self, ver: &Self::Version) -> Result<PathBuf, FetchError> {
                let path = self.dir.join(ver.to_string());
                std::fs::write(&path, b"verified compiler").unwrap();
                Ok(path)
            }

            async fn validate_file(
                &self,
                _ver: &Self::Version,
                path: &std::path::Path,
            ) -> Result<(), FetchError> {
                self.validation_calls.fetch_add(1, Ordering::Relaxed);
                assert_eq!(std::fs::read(path).unwrap(), b"verified compiler");
                Ok(())
            }

            fn all_versions(&self) -> Vec<Self::Version> {
                vec![]
            }
        }

        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("shared");
        std::fs::create_dir(&dir).unwrap();
        // Another user could replace a download in a world-writable directory.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        let fetcher = SharedDirectoryFetcher {
            dir: dir.clone(),
            validation_calls: AtomicUsize::new(0),
        };
        let ver = new_version(5);
        let cache = DownloadCache::default();

        let snapshot_path = cache.get(&fetcher, &ver).await.unwrap();
        assert!(!snapshot_path.starts_with(&dir));
        assert_eq!(fetcher.validation_calls.load(Ordering::Relaxed), 1);

        std::fs::write(dir.join(ver.to_string()), b"replaced after validation").unwrap();
        assert_eq!(cache.get(&fetcher, &ver).await.unwrap(), snapshot_path);
        assert_eq!(std::fs::read(&snapshot_path).unwrap(), b"verified compiler");
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

    #[tokio::test]
    async fn private_preloaded_compiler_is_validated_in_place() {
        struct InPlaceFetcher;

        #[async_trait]
        impl Fetcher for InPlaceFetcher {
            type Version = evm_version::DetailedVersion;

            async fn fetch(&self, _ver: &Self::Version) -> Result<PathBuf, FetchError> {
                panic!("valid preloaded compiler must not be downloaded")
            }

            async fn validate_file(
                &self,
                _ver: &Self::Version,
                _path: &std::path::Path,
            ) -> Result<(), FetchError> {
                Ok(())
            }

            fn all_versions(&self) -> Vec<Self::Version> {
                vec![]
            }
        }

        let ver = new_version(6);
        let dir = tempfile::tempdir().unwrap();
        let version_dir = dir.path().join(ver.to_string());
        std::fs::create_dir(&version_dir).unwrap();
        let preloaded_path = version_dir.join("solc");
        std::fs::write(&preloaded_path, b"trusted compiler").unwrap();
        // The configured compilers directory is reached through a symlink.
        let link_dir = tempfile::tempdir().unwrap();
        let compilers_dir = link_dir.path().join("compilers");
        std::os::unix::fs::symlink(dir.path(), &compilers_dir).unwrap();

        let cache = DownloadCache::default();
        cache.load_from_dir(&compilers_dir).await.unwrap();

        assert_eq!(
            cache.get(&InPlaceFetcher, &ver).await.unwrap(),
            std::fs::canonicalize(&preloaded_path).unwrap(),
            "a compiler only this user can change needs no snapshot, and is used by its resolved \
             path so a symlink along the configured path cannot be retargeted"
        );
    }

    #[test]
    fn non_canonical_version_directories_are_kept_and_canonical_ones_win() {
        let canonical = evm_version::DetailedVersion::from_str("v0.8.4+commit.dea1b9ec").unwrap();
        let only_non_canonical =
            evm_version::DetailedVersion::from_str("v0.8.5+commit.a4f2e591").unwrap();

        let versions: HashMap<evm_version::DetailedVersion, PathBuf> = super::filter_versions(
            [
                "0.8.4+commit.dea1b9ec",
                "v0.8.4+commit.dea1b9ec",
                "0.8.5+commit.a4f2e591",
            ]
            .into_iter()
            .map(PathBuf::from),
        );

        assert_eq!(
            versions.get(&canonical),
            Some(&PathBuf::from("v0.8.4+commit.dea1b9ec"))
        );
        assert_eq!(
            versions.get(&only_non_canonical),
            Some(&PathBuf::from("0.8.5+commit.a4f2e591"))
        );
    }
}
