use ethers_solc::Solc;
use semver::Version;
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub struct CompilerCache {
    cache: std::sync::Mutex<HashMap<Version, Arc<tokio::sync::RwLock<Option<Solc>>>>>,
}

impl CompilerCache {
    async fn try_get(&self, ver: &Version) -> Option<Solc> {
        let entry = {
            let cache = self.cache.lock().unwrap();
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

    async fn fetch(&self, ver: &Version) -> anyhow::Result<Solc> {
        let lock = {
            let mut cache = self.cache.lock().unwrap();
            Arc::clone(cache.entry(ver.clone()).or_default())
        };
        let mut entry = lock.write().await;
        match entry.as_ref() {
            Some(solc) => Ok(solc.clone()),
            None => {
                log::info!("installing solc version {}", ver);
                let solc = Solc::install(ver).await?;
                *entry = Some(solc.clone());
                Ok(solc)
            }
        }
    }

    pub async fn get(&self, ver: &Version) -> anyhow::Result<Solc> {
        match self.try_get(ver).await {
            Some(solc) => Ok(solc),
            None => self.fetch(ver).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fetch(cache: &CompilerCache, ver: &Version) {
        let now = std::time::Instant::now();
        cache.get(&ver).await.unwrap();
        println!("{} {:?}", ver, now.elapsed());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
    async fn it_works() {
        env_logger::init();

        let cache = Arc::<CompilerCache>::default();

        let versions = vec![
            Version::new(0, 5, 16),
            Version::new(0, 8, 13),
            Version::new(0, 6, 4),
        ];

        for i in 0..versions.len() {
            let mut tasks = vec![];
            for j in 0..i + 1 {
                let c = Arc::clone(&cache);
                let v = versions[j].clone();
                tasks.push(tokio::spawn(async move {
                    fetch(&c, &v).await;
                }));
            }
            let r: Result<Vec<_>, _> = futures::future::join_all(tasks).await.into_iter().collect();
            r.unwrap();
        }
    }
}
