use std::{collections::HashMap, sync::Arc, time::Instant};

use parking_lot::RwLock;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::Mutex;

use crate::token_info::{service::TokenKey, settings::BlockscoutTokenInfoClientSettings};

/// TokenInfo model based on the Swagger specification for contracts-info API.
/// Represents the response from `/api/v1/chains/{chainId}/token-infos/{tokenAddress}` endpoint.
/// Unknown fields from the API response are ignored
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BlockscoutTokenInfo {
    pub token_address: String,
    pub chain_id: String,
    pub project_name: Option<String>,
    pub project_website: String,
    pub project_email: String,
    pub icon_url: String,
    pub project_description: String,
    pub project_sector: Option<String>,
    pub docs: Option<String>,
    pub github: Option<String>,
    pub telegram: Option<String>,
    pub linkedin: Option<String>,
    pub discord: Option<String>,
    pub slack: Option<String>,
    pub twitter: Option<String>,
    pub open_sea: Option<String>,
    pub facebook: Option<String>,
    pub medium: Option<String>,
    pub reddit: Option<String>,
    pub support: Option<String>,
    pub coin_market_cap_ticker: Option<String>,
    pub coin_gecko_ticker: Option<String>,
    pub defi_llama_ticker: Option<String>,
    pub token_name: Option<String>,
    pub token_symbol: Option<String>,
}

impl BlockscoutTokenInfo {
    /// Returns the icon URL if it's not empty, otherwise None
    pub fn icon_url(&self) -> Option<String> {
        if !self.is_valid() || self.icon_url.is_empty() {
            None
        } else {
            Some(self.icon_url.clone())
        }
    }

    /// Returns true if this is a valid token info response (token_address is not empty)
    pub fn is_valid(&self) -> bool {
        !self.token_address.is_empty()
    }
}

/// Error types for the BlockscoutTokenInfo client
#[derive(Debug, thiserror::Error)]
pub enum BlockscoutTokenInfoError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("Token not found")]
    TokenNotFound,
    #[error("Bad response code: {0}")]
    InvalidResponse(u16),
    #[error("Invalid address length: expected 20, got {0}")]
    InvalidAddressLength(usize),
    #[error("Invalid chain ID: {0}")]
    InvalidChainId(i64),
}

/// Cached result for icon fetching.
#[derive(Clone)]
struct CachedIconResult {
    icon_url: Option<String>,
    cached_at: Instant,
}

/// Client for the Blockscout contracts-info API.
/// Only uses the `/api/v1/chains/{chainId}/token-infos/{tokenAddress}` endpoint.
/// Prevents duplicate simultaneous requests for the same token using per-key locks and caching.
#[derive(Clone)]
pub struct BlockscoutTokenInfoClient {
    client: Client,
    settings: BlockscoutTokenInfoClientSettings,
    // Per-key in-flight locks to prevent duplicate API calls
    // (when multiple threads request the same token info concurrently).
    per_key_locks: Arc<RwLock<HashMap<TokenKey, Arc<Mutex<()>>>>>,
    // Cache for icon results to avoid repeated requests
    icon_cache: Arc<RwLock<HashMap<TokenKey, CachedIconResult>>>,
}

impl BlockscoutTokenInfoClient {
    pub fn new(settings: BlockscoutTokenInfoClientSettings) -> Self {
        if settings.url.is_none() {
            tracing::warn!(
                "Blockscout token info URL not configured. Token icons will not available."
            );
        }

        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .expect("Failed to build BlockscoutTokenInfoClient"),
            settings,
            per_key_locks: Arc::new(RwLock::new(HashMap::new())),
            icon_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns cached icon if it exists and is still valid (within retry_interval).
    /// Returns Some(Some(url)) if icon found, Some(None) if cached as not found,
    /// None if not in cache or cache expired.
    fn get_cached_icon(&self, key: &TokenKey) -> Option<Option<String>> {
        let cache = self.icon_cache.read();
        let cached = cache.get(key)?;

        // Check if cached None isn't expired
        if cached.icon_url.is_none() && cached.cached_at.elapsed() > self.settings.retry_interval {
            return None; // old record -> ignore it
        }

        Some(cached.icon_url.clone())
    }

    fn cache_icon_result(&self, key: TokenKey, icon_url: Option<String>) {
        let result = CachedIconResult {
            icon_url,
            cached_at: Instant::now(),
        };
        self.icon_cache.write().insert(key, result);
    }

    fn get_lock_for_key(&self, key: &TokenKey) -> Arc<Mutex<()>> {
        // If the lock already exists for the key, return it
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

    /// Fetches full available token info for a specific token.
    async fn get_token_info(
        &self,
        base_url: &str,
        chain_id: i64,
        token_address: &str,
    ) -> Result<BlockscoutTokenInfo, BlockscoutTokenInfoError> {
        let normalized_address = token_address.to_lowercase();
        let url = format!(
            "{}/api/v1/chains/{}/token-infos/{}",
            base_url.trim_end_matches('/'),
            chain_id,
            normalized_address
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(BlockscoutTokenInfoError::InvalidResponse(
                response.status().as_u16(),
            ));
        }

        let token_info: BlockscoutTokenInfo = response.json().await?;

        // Check if the response is valid (token_address is not empty indicates a known token)
        if !token_info.is_valid() {
            return Err(BlockscoutTokenInfoError::TokenNotFound);
        }

        Ok(token_info)
    }

    /// Fetches only the icon URL for a specific token.
    /// This is a convenience method that extracts just the icon from the full token info.
    /// Uses per-key locks to prevent duplicate simultaneous requests.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID where the token is deployed
    /// * `token_address` - The token contract address bytes
    ///
    /// # Returns
    /// * `Ok(Some(url))` - The icon URL if found
    /// * `Ok(None)` - If the token exists but has no icon
    /// * `Err(...)` - If the request fails or token is not found
    pub async fn get_token_icon(
        &self,
        chain_id: i64,
        token_address: &Vec<u8>,
    ) -> Result<Option<String>, BlockscoutTokenInfoError> {
        if chain_id < 0 {
            return Err(BlockscoutTokenInfoError::InvalidChainId(chain_id));
        }

        if token_address.len() != 20 {
            return Err(BlockscoutTokenInfoError::InvalidAddressLength(
                token_address.len(),
            ));
        }

        let Some(base_url) = self.settings.url.as_ref() else {
            // if service isn't configured - just return None
            return Ok(None);
        };

        if self.settings.ignore_chains.contains(&chain_id) {
            // if chain is in the ignore list - just return None
            return Ok(None);
        }

        let key = (chain_id, token_address.clone());

        // Fast path: check cache first
        if let Some(cached) = self.get_cached_icon(&key) {
            return Ok(cached);
        }

        // Acquire the lock for this key to prevent duplicate requests.
        // Use a block so key_lock and _guard are dropped before remove_lock_for_key,
        // allowing strong_count to reach 1 so the map entry can be removed.
        let icon_url = {
            let key_lock = self.get_lock_for_key(&key);
            let _guard = key_lock.lock().await;

            // Check cache again after acquiring lock (another thread may have fetched it)
            if let Some(cached) = self.get_cached_icon(&key) {
                drop(_guard);
                drop(key_lock);
                self.remove_lock_for_key(&key);
                return Ok(cached);
            }

            let address_hex = format!("0x{}", hex::encode(token_address));
            let result = self.get_token_info(base_url, chain_id, &address_hex).await;

            match &result {
                Ok(info) => info.icon_url(),
                Err(e) => {
                    tracing::warn!(
                        chain_id = chain_id,
                        token_address = address_hex,
                        error = e.to_string(),
                        "Failed to fetch token info"
                    );
                    None
                }
            }
        };

        self.cache_icon_result(key.clone(), icon_url.clone());

        // Clean up the lock (key_lock is dropped so strong_count == 1)
        self.remove_lock_for_key(&key);

        Ok(icon_url)
    }
}
