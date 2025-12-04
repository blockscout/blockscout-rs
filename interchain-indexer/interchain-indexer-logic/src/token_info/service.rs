use std::{collections::HashMap, sync::Arc};

use alloy::{network::Ethereum, providers::DynProvider};
use chrono::{DateTime, Utc};
use interchain_indexer_entity::tokens::{self, Model as TokenInfoModel};
use parking_lot::RwLock;
use sea_orm::ActiveValue::Set;
use tokio::sync::Mutex;

use crate::InterchainDatabase;

use crate::token_info::fetchers::*;

pub struct TokenInfoService {
    db: Arc<InterchainDatabase>,
    providers: HashMap<u64, DynProvider<Ethereum>>,

    // The list of the fetchers to retrieve the token info on-chain
    fetchers: Vec<Box<dyn TokenInfoFetcher>>,

    // Local token info cache: map of (chain_id, address) to token info DB model
    // Wrapped into async RwLock to allow concurrent access.
    token_info_cache: RwLock<HashMap<(u64, Vec<u8>), TokenInfoModel>>,

    // Per-key in-flight locks to prevent duplicate RPC/DB calls
    // (when multiple threads request the same token info concurrently).
    per_key_locks: RwLock<HashMap<(u64, Vec<u8>), Arc<Mutex<()>>>>,

    // Cache of failed token info requests to avoid repeated attempts
    error_cache: RwLock<HashMap<(u64, Vec<u8>), DateTime<Utc>>>,
}

impl TokenInfoService {
    pub fn new(
        db: Arc<InterchainDatabase>,
        providers: HashMap<u64, DynProvider<Ethereum>>,
    ) -> Self {
        Self {
            db,
            providers,
            fetchers: vec![
                Box::new(Erc20TokenInfoFetcher),
                Box::new(Erc20TokenHomeInfoFetcher::new()),
            ],
            token_info_cache: RwLock::new(HashMap::new()),
            per_key_locks: RwLock::new(HashMap::new()),
            error_cache: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_token_info(
        &self,
        chain_id: u64,
        address: Vec<u8>,
    ) -> anyhow::Result<TokenInfoModel> {
        let key = (chain_id, address.clone());

        // Fast path: try to read from cache
        if let Some(model) = self.read_cache(&key) {
            return Ok(model);
        }

        // Acquire the lock for this key and wait it
        // It's needed to prevent duplicate RPC/DB calls
        // in case of concurrent requests for the same token info
        let key_lock = self.get_lock_for_key(&key);
        let guard = key_lock.lock().await;

        // Check the cache again: it may be filled by other workers
        // by the time we waiting for the lock
        if let Some(model) = self.read_cache(&key) {
            return Ok(model);
        }

        // Check if the token info request was failed recently
        if let Some(failed_at) = self.error_cache.read().get(&key) {
            static ERROR_CACHE_TTL_SECONDS: i64 = 10;
            if (Utc::now() - failed_at).num_seconds() < ERROR_CACHE_TTL_SECONDS {
                return Err(anyhow::anyhow!("Token info request failed recently, will retry later"));
            }
        }

        // Not in cache: load from DB
        if let Some(model) = self.db.get_token_info(chain_id, key.1.clone()).await? {
            // Insert into cache
            let mut cache = self.token_info_cache.write();
            cache.entry(key.clone()).or_insert_with(|| model.clone());

            return Ok(model);
        }

        // Not in DB: fetch on-chain via RPC provider
        let provider = self
            .providers
            .get(&chain_id)
            .ok_or_else(|| anyhow::anyhow!("Provider not found for chain_id={}", chain_id))?;

        let res = match self.try_fetch_token_info(provider, chain_id, address.clone()).await {
            Ok(token_info) => {
                let model = TokenInfoModel {
                    chain_id: chain_id as i64,
                    address: address,
                    name: Some(token_info.name),
                    symbol: Some(token_info.symbol),
                    token_icon: None,
                    decimals: Some(token_info.decimals as i16),
                    created_at: None,
                    updated_at: None,
                };
        
                let active_model = tokens::ActiveModel {
                    chain_id: Set(model.chain_id),
                    address: Set(model.address.clone()),
                    symbol: Set(model.symbol.clone()),
                    name: Set(model.name.clone()),
                    token_icon: Set(model.token_icon.clone()),
                    decimals: Set(model.decimals),
                    ..Default::default()
                };
        
                self.db.upsert_token_info(active_model).await?;
        
                {
                    let mut cache = self.token_info_cache.write();
                    cache.entry(key.clone()).or_insert_with(|| model.clone());
                }

                Ok(model)
            },
            Err(e) => {
                let mut error_cache = self.error_cache.write();
                error_cache.insert(key.clone(), Utc::now());
                Err(e.into())
            }
        };

        // Release the mutex
        drop(guard);

        // prevent per_key_locks unlimited growth
        self.remove_lock_for_key(&key);

        res
    }

    fn read_cache(&self, key: &(u64, Vec<u8>)) -> Option<TokenInfoModel> {
        let cache = self.token_info_cache.read();
        cache.get(key).cloned()
    }

    fn get_lock_for_key(&self, key: &(u64, Vec<u8>)) -> Arc<Mutex<()>> {
        // If the lock already exist for the key, return it
        if let Some(lock) = self.per_key_locks.read().get(key) {
            return lock.clone();
        }

        // Otherwise - create a new lock and return it
        let mut locks = self.per_key_locks.write();
        locks
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn remove_lock_for_key(&self, key: &(u64, Vec<u8>)) {
        let mut locks = self.per_key_locks.write();
        if let Some(stored_lock) = locks.get(&key) {
            // nobody holds the lock
            if Arc::strong_count(stored_lock) == 1 {
                locks.remove(&key);
            }
        }
    }

    async fn try_fetch_token_info(&self, provider: &DynProvider<Ethereum>, chain_id: u64, address: Vec<u8>) -> anyhow::Result<OnchainTokenInfo> {
        if address.len() != 20 {
            return Err(anyhow::anyhow!("Invalid address length: expected 20, got {}", address.len()));
        }
        
        let mut last_error = None;
        for fetcher in self.fetchers.iter() {
            match fetcher.fetch_token_info(provider, chain_id, address.clone()).await {
                Ok(info) => return Ok(info),
                Err(e) => last_error = Some(e),
            };
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Failed to fetch token info: no available fetchers")))
    }
}
