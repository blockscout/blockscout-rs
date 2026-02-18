use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{InterchainDatabase, TokenInfoServiceSettings};
use alloy::{network::Ethereum, providers::DynProvider};
use chrono::{DateTime, Utc};
use interchain_indexer_entity::tokens::{self, Model as TokenInfoModel};
use parking_lot::RwLock;
use sea_orm::ActiveValue::Set;
use tokio::sync::Mutex;

use crate::token_info::{blockscout_tokeninfo::BlockscoutTokenInfoClient, fetchers::*};

pub type TokenKey = (i64, Vec<u8>);

pub struct TokenInfoService {
    db: Arc<InterchainDatabase>,
    providers: HashMap<i64, DynProvider<Ethereum>>,
    settings: TokenInfoServiceSettings,

    // The list of the fetchers to retrieve the token info on-chain
    fetchers: Vec<Box<dyn TokenInfoFetcher>>,

    // Optional client for fetching token icons from Blockscout contracts-info API
    token_info_client: BlockscoutTokenInfoClient,

    // Local token info cache: map of (chain_id, address) to token info DB model
    // Wrapped into Arc<RwLock> to allow concurrent access and cloning for background tasks.
    token_info_cache: Arc<RwLock<HashMap<TokenKey, TokenInfoModel>>>,

    // Per-key in-flight locks to prevent duplicate RPC/DB calls
    // (when multiple threads request the same token info concurrently).
    per_key_locks: RwLock<HashMap<TokenKey, Arc<Mutex<()>>>>,

    // Cache of failed token info requests to avoid repeated attempts
    error_cache: RwLock<HashMap<TokenKey, DateTime<Utc>>>,

    // Keys currently being fetched in the background to prevent duplicate spawns.
    // We can't reuse per_key_locks for this: MutexGuard borrows from the Mutex,
    // so we cannot move the guard into a spawned task (the borrow checker forbids it).
    in_flight_fetches: Arc<RwLock<HashSet<TokenKey>>>,
}

impl TokenInfoService {
    pub fn new(
        db: Arc<InterchainDatabase>,
        providers: HashMap<i64, DynProvider<Ethereum>>,
        settings: TokenInfoServiceSettings,
    ) -> Self {
        let token_info_client =
            BlockscoutTokenInfoClient::new(settings.blockscout_token_info.clone());

        Self {
            db,
            providers,
            settings,
            fetchers: vec![
                Box::new(Erc20TokenInfoFetcher),
                Box::new(Erc20TokenHomeInfoFetcher::new()),
            ],
            token_info_client,
            token_info_cache: Arc::new(RwLock::new(HashMap::new())),
            per_key_locks: RwLock::new(HashMap::new()),
            error_cache: RwLock::new(HashMap::new()),
            in_flight_fetches: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub async fn get_token_info(
        self: Arc<Self>,
        chain_id: i64,
        address: Vec<u8>,
    ) -> anyhow::Result<TokenInfoModel> {
        if chain_id < 0 {
            return Err(anyhow::anyhow!("chain_id is negative: {}", chain_id));
        }

        let key = (chain_id, address.clone());

        // Fast path: try to read from cache
        if let Some(model) = self.read_cache(&key) {
            return Ok(self.fetch_icon_if_needed(model).await);
        }

        // Acquire the lock for this key and wait it
        // It's needed to prevent duplicate RPC/DB calls
        // in case of concurrent requests for the same token info
        let key_lock = self.get_lock_for_key(&key);
        let guard = key_lock.lock().await;

        let res = async {
            // Check the cache again: it may be filled by other workers
            // by the time we waiting for the lock
            if let Some(model) = self.read_cache(&key) {
                return Ok(self.fetch_icon_if_needed(model).await);
            }

            // Check if the token info request was failed recently
            if let Some(failed_at) = self.error_cache.read().get(&key)
                && (Utc::now() - failed_at).num_seconds()
                    < self.settings.onchain_retry_interval.as_secs() as i64
            {
                return Err(anyhow::anyhow!(
                    "Token info request failed recently, will retry later"
                ));
            }

            // Not in cache: load from DB
            if let Some(model) = self
                .db
                .get_token_info(chain_id as u64, key.1.clone())
                .await?
            {
                // Insert into cache (drop guard before any await)
                {
                    let mut cache = self.token_info_cache.write();
                    cache.entry(key.clone()).or_insert_with(|| model.clone());
                }

                // Try to fetch icon if missing and stale, return updated model
                let model = self.fetch_icon_if_needed(model).await;

                return Ok(model);
            }

            // Not in DB: spawn background fetch and return bare model immediately.
            // Use in_flight_fetches to prevent duplicate spawns (we can't reuse per_key_locks:
            // MutexGuard borrows from the Mutex, so we cannot move it into a spawned task).
            let _provider = self
                .providers
                .get(&chain_id)
                .ok_or_else(|| anyhow::anyhow!("Provider not found for chain_id={}", chain_id))?;

            let should_spawn = {
                let mut in_flight = self.in_flight_fetches.write();
                in_flight.insert(key.clone())
            };

            if should_spawn {
                let service = self.clone();
                let chain_id = key.0;
                let address = key.1.clone();
                let key_clone = key.clone();
                tokio::spawn(async move {
                    service
                        .fetch_token_info_from_chain_and_persist(chain_id, address, key_clone)
                        .await;
                });
            }

            // Return bare model immediately without waiting
            Ok(TokenInfoModel {
                chain_id,
                address: key.1.clone(),
                name: None,
                symbol: None,
                token_icon: None,
                decimals: None,
                created_at: None,
                updated_at: None,
            })
        }
        .await;

        // Release the mutex and the local Arc so strong_count can reach 1
        drop(guard);
        drop(key_lock);

        // prevent per_key_locks unlimited growth
        self.remove_lock_for_key(&key);

        res
    }

    fn remove_from_in_flight(&self, key: &TokenKey) {
        let mut in_flight = self.in_flight_fetches.write();
        in_flight.remove(key);
    }

    /// Fetches token info via RPC and token icon via external API, then persists to DB and cache.
    async fn fetch_token_info_from_chain_and_persist(
        self: Arc<Self>,
        chain_id: i64,
        address: Vec<u8>,
        key: TokenKey,
    ) {
        let provider = match self.providers.get(&chain_id) {
            Some(p) => p,
            None => {
                self.remove_from_in_flight(&key);
                return;
            }
        };

        let onchain_future = self.try_fetch_token_info_onchain(provider, chain_id, address.clone());
        let icon_future = self.try_fetch_token_icon(chain_id, address.clone());

        let (onchain_result, icon_result) = tokio::join!(onchain_future, icon_future);

        match onchain_result {
            Ok(token_info) => {
                let icon_url = icon_result.ok().flatten();

                let model = TokenInfoModel {
                    chain_id,
                    address,
                    name: Some(token_info.name),
                    symbol: Some(token_info.symbol),
                    token_icon: icon_url,
                    decimals: Some(token_info.decimals as i16),
                    created_at: None,
                    updated_at: None,
                };

                let active_model = tokens::ActiveModel {
                    chain_id: Set(chain_id),
                    address: Set(model.address.clone()),
                    symbol: Set(model.symbol.clone()),
                    name: Set(model.name.clone()),
                    token_icon: Set(model.token_icon.clone()),
                    decimals: Set(model.decimals),
                    ..Default::default()
                };

                if let Err(e) = self.db.upsert_token_info(active_model).await {
                    tracing::warn!(
                        chain_id = chain_id,
                        address = hex::encode(&model.address),
                        error = ?e,
                        "Failed to upsert token info in background"
                    );
                } else {
                    let mut cache = self.token_info_cache.write();
                    cache.entry(key.clone()).or_insert_with(|| model.clone());
                }

                let mut error_cache = self.error_cache.write();
                error_cache.remove(&key);
            }
            Err(e) => {
                tracing::warn!(
                    chain_id = chain_id,
                    address = hex::encode(&address),
                    error = ?e,
                    "Background token info fetch failed"
                );
                let mut error_cache = self.error_cache.write();
                error_cache.insert(key.clone(), Utc::now());
            }
        }
        self.remove_from_in_flight(&key);
    }

    /// Checks if an existing token (from cache or DB) needs an icon update.
    /// Returns the model with the icon if successfully fetched, otherwise returns the original model.
    async fn fetch_icon_if_needed(&self, mut model: TokenInfoModel) -> TokenInfoModel {
        // Only process tokens without icons
        if model.token_icon.as_ref().is_some_and(|s| !s.is_empty()) {
            return model;
        }

        let chain_id = model.chain_id;
        let address = model.address.clone();

        match self.try_fetch_token_icon(chain_id, address.clone()).await {
            Ok(Some(icon_url)) => {
                // Update the model with the icon
                model.token_icon = Some(icon_url.clone());

                // Update database and cache
                self.update_token_icon(chain_id, address, Some(icon_url))
                    .await;
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(
                    chain_id = chain_id,
                    address = hex::encode(&address),
                    error = ?e,
                    "Failed to fetch token icon"
                );
            }
        }

        model
    }

    fn read_cache(&self, key: &TokenKey) -> Option<TokenInfoModel> {
        let cache = self.token_info_cache.read();
        cache.get(key).cloned()
    }

    fn get_lock_for_key(&self, key: &TokenKey) -> Arc<Mutex<()>> {
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

    fn remove_lock_for_key(&self, key: &TokenKey) {
        let mut locks = self.per_key_locks.write();
        if let Some(stored_lock) = locks.get(key) {
            // nobody holds the lock
            if Arc::strong_count(stored_lock) == 1 {
                locks.remove(key);
            }
        }
    }

    /// Updates the token icon in the database and cache.
    async fn update_token_icon(&self, chain_id: i64, address: Vec<u8>, icon_url: Option<String>) {
        if let Err(e) = self
            .db
            .update_token_icon(chain_id as u64, address.clone(), icon_url.clone())
            .await
        {
            tracing::warn!(
                chain_id = chain_id,
                address = hex::encode(&address),
                error = ?e,
                "Failed to update token icon in database"
            );
            return;
        }

        // Update the cache
        let key = (chain_id, address);
        let mut cache_guard = self.token_info_cache.write();
        if let Some(cached_model) = cache_guard.get_mut(&key) {
            cached_model.token_icon = icon_url;
            cached_model.updated_at = Some(Utc::now().naive_utc());
        }
    }

    async fn try_fetch_token_info_onchain(
        &self,
        provider: &DynProvider<Ethereum>,
        chain_id: i64,
        address: Vec<u8>,
    ) -> anyhow::Result<OnchainTokenInfo> {
        if address.len() != 20 {
            return Err(anyhow::anyhow!(
                "Invalid address length: expected 20, got {}",
                address.len()
            ));
        }
        let mut last_error = None;
        for fetcher in self.fetchers.iter() {
            match fetcher
                .fetch_token_info(provider, chain_id as u64, address.clone())
                .await
            {
                Ok(info) => return Ok(info),
                Err(e) => last_error = Some(e),
            };
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!("Failed to fetch token info: no available fetchers")
        }))
    }

    /// Attempts to fetch a token icon from the Blockscout contracts-info API.
    async fn try_fetch_token_icon(
        &self,
        chain_id: i64,
        address: Vec<u8>,
    ) -> anyhow::Result<Option<String>> {
        self.token_info_client
            .get_token_icon(chain_id, &address)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch token icon: {}", e))
    }
}
