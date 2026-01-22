use std::{collections::HashMap, sync::Arc};

use crate::{InterchainDatabase, TokenInfoServiceSettings};
use alloy::{network::Ethereum, providers::DynProvider};
use chrono::{DateTime, Utc};
use interchain_indexer_entity::tokens::{self, Model as TokenInfoModel};
use parking_lot::RwLock;
use sea_orm::ActiveValue::Set;
use tokio::sync::Mutex;

use crate::token_info::{blockscout_tokeninfo::BlockscoutTokenInfoClient, fetchers::*};

pub type TokenKey = (u64, Vec<u8>);

pub struct TokenInfoService {
    db: Arc<InterchainDatabase>,
    providers: HashMap<u64, DynProvider<Ethereum>>,
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
}

impl TokenInfoService {
    pub fn new(
        db: Arc<InterchainDatabase>,
        providers: HashMap<u64, DynProvider<Ethereum>>,
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
            if let Some(model) = self.db.get_token_info(chain_id, key.1.clone()).await? {
                // Insert into cache (drop guard before any await)
                {
                    let mut cache = self.token_info_cache.write();
                    cache.entry(key.clone()).or_insert_with(|| model.clone());
                }

                // Try to fetch icon if missing and stale, return updated model
                let model = self.fetch_icon_if_needed(model).await;

                return Ok(model);
            }

            // Not in DB: fetch on-chain via RPC provider
            let provider = self
                .providers
                .get(&chain_id)
                .ok_or_else(|| anyhow::anyhow!("Provider not found for chain_id={}", chain_id))?;

            // Fetch token info on-chain and icon in parallel
            let onchain_future =
                self.try_fetch_token_info_onchain(provider, chain_id, address.clone());
            let icon_future = self.try_fetch_token_icon(chain_id, address.clone());

            let (onchain_result, icon_result) = tokio::join!(onchain_future, icon_future);

            match onchain_result {
                Ok(token_info) => {
                    // Use the icon from the external API if available
                    let icon_url = icon_result.ok().flatten();

                    let model = TokenInfoModel {
                        chain_id: chain_id as i64,
                        address,
                        name: Some(token_info.name),
                        symbol: Some(token_info.symbol),
                        token_icon: icon_url,
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
                }
                Err(e) => {
                    let mut error_cache = self.error_cache.write();
                    error_cache.insert(key.clone(), Utc::now());
                    Err(e)
                }
            }
        }
        .await;

        // Release the mutex
        drop(guard);

        // prevent per_key_locks unlimited growth
        self.remove_lock_for_key(&key);

        res
    }

    /// Checks if an existing token (from cache or DB) needs an icon update.
    /// Returns the model with the icon if successfully fetched, otherwise returns the original model.
    async fn fetch_icon_if_needed(&self, mut model: TokenInfoModel) -> TokenInfoModel {
        // Only process tokens without icons
        if model.token_icon.as_ref().is_some_and(|s| !s.is_empty()) {
            return model;
        }

        let chain_id = model.chain_id as u64;
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
                // Request failed, touch updated_at to prevent repeated attempts
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
    async fn update_token_icon(&self, chain_id: u64, address: Vec<u8>, icon_url: Option<String>) {
        if let Err(e) = self
            .db
            .update_token_icon(chain_id, address.clone(), icon_url.clone())
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
        chain_id: u64,
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
                .fetch_token_info(provider, chain_id, address.clone())
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
        chain_id: u64,
        address: Vec<u8>,
    ) -> anyhow::Result<Option<String>> {
        self.token_info_client
            .get_token_icon(chain_id, &address)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch token icon: {}", e))
    }
}
