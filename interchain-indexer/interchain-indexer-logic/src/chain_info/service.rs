use interchain_indexer_entity::chains;
use parking_lot::RwLock;
use sea_orm::JsonValue;
use std::{collections::HashMap, sync::Arc, time::Instant};

use crate::{ChainInfoServiceSettings, InterchainDatabase};

/// Default name returned when chain info is not found in database for any reason
const UNKNOWN_CHAIN_NAME: &str = "Unknown";

// Do not return default explorer routes even if they are specified in the database
const DEFAULT_EXPLORER_TX_ROUTE: &str = "/tx/{hash}";
const DEFAULT_EXPLORER_ADDRESS_ROUTE: &str = "/address/{hash}";
const DEFAULT_EXPLORER_TOKEN_ROUTE: &str = "/token/{hash}";

/// Creates a placeholder chain model for an unknown chain
fn unknown_chain(chain_id: i64) -> chains::Model {
    chains::Model {
        id: chain_id,
        name: UNKNOWN_CHAIN_NAME.to_string(),
        icon: None,
        explorer: None,
        custom_routes: None,
        created_at: None,
        updated_at: None,
    }
}

/// Checks if the chain has a valid (non-empty) name
fn has_valid_name(model: &chains::Model) -> bool {
    !model.name.is_empty()
}

/// Filters out default routes from custom_routes JSON.
/// Returns None if all routes are default or the input is None/empty.
fn filter_default_routes(custom_routes: Option<JsonValue>) -> Option<JsonValue> {
    let value = custom_routes?;

    let obj = value.as_object()?;

    let mut filtered = serde_json::Map::new();

    for (key, val) in obj {
        let Some(route) = val.as_str() else {
            // Keep non-string values as-is
            filtered.insert(key.clone(), val.clone());
            continue;
        };

        let is_default = match key.as_str() {
            "tx" => route == DEFAULT_EXPLORER_TX_ROUTE,
            "address" => route == DEFAULT_EXPLORER_ADDRESS_ROUTE,
            "token" => route == DEFAULT_EXPLORER_TOKEN_ROUTE,
            _ => false, // Keep unknown route types
        };

        if !is_default {
            filtered.insert(key.clone(), val.clone());
        }
    }

    if filtered.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(filtered))
    }
}

/// Normalizes the chain model: substitutes empty name, filters default routes,
/// and removes trailing slash from explorer URL
fn normalize_chain(mut model: chains::Model) -> chains::Model {
    if model.name.is_empty() {
        model.name = UNKNOWN_CHAIN_NAME.to_string();
    }
    model.custom_routes = filter_default_routes(model.custom_routes);
    model.explorer = model
        .explorer
        .map(|url| url.trim_end_matches('/').to_string());
    model
}

/// Service for retrieving chain information with in-memory caching.
///
/// This service caches chain data to minimize database queries.
/// For chains without a name, it implements rate-limiting to avoid
/// frequent DB lookups and returns a placeholder with "Unknown" name.
#[derive(Clone)]
pub struct ChainInfoService {
    db: Arc<InterchainDatabase>,
    settings: ChainInfoServiceSettings,
    /// Cache of chains with valid names: chain_id -> chains::Model
    cache: Arc<RwLock<HashMap<i64, chains::Model>>>,
    /// Tracks last query time for chains with unknown names to implement cooldown
    unknown_name_last_query: Arc<RwLock<HashMap<i64, Instant>>>,
}

impl ChainInfoService {
    pub fn new(db: Arc<InterchainDatabase>, settings: ChainInfoServiceSettings) -> Self {
        Self {
            db,
            settings,
            cache: Arc::new(RwLock::new(HashMap::new())),
            unknown_name_last_query: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Gets chain info by chain_id.
    ///
    /// Returns cached data if available. If not cached:
    /// - Queries the database (with rate-limiting for chains with empty names)
    /// - Returns "Unknown" as the chain name if name is empty or chain not found
    pub async fn get_chain_info(&self, chain_id: i64) -> chains::Model {
        if chain_id < 0 {
            return unknown_chain(chain_id);
        }

        // Fast path: check cache first (only chains with valid names are cached)
        if let Some(info) = self.cache.read().get(&chain_id) {
            return info.clone();
        }

        // Check if we should skip DB query due to cooldown for chains with unknown names
        if self.is_in_cooldown(chain_id) {
            return unknown_chain(chain_id);
        }

        // Query database
        match self.db.get_chain_by_id(chain_id as u64).await {
            Ok(Some(model)) => {
                let has_name = has_valid_name(&model);
                let normalized = normalize_chain(model);
                if has_name {
                    // Chain has a valid name - cache it and remove from cooldown tracker
                    self.cache.write().insert(chain_id, normalized.clone());
                    self.unknown_name_last_query.write().remove(&chain_id);
                    normalized
                } else {
                    // Chain exists but has empty name - apply cooldown
                    tracing::debug!(chain_id, "Chain has empty name in database");
                    self.unknown_name_last_query
                        .write()
                        .insert(chain_id, Instant::now());
                    normalized
                }
            }
            Ok(None) => {
                tracing::warn!(chain_id, "Chain not found in database");
                unknown_chain(chain_id)
            }
            Err(e) => {
                tracing::error!(err = ?e, chain_id, "Failed to fetch chain from database");
                unknown_chain(chain_id)
            }
        }
    }

    /// Returns all chains from the database, ordered by `chains.id` ascending.
    ///
    /// Each row is normalized the same way as [`Self::get_chain_info`]: empty names become
    /// `"Unknown"`, default explorer routes are removed, and trailing slashes are stripped
    /// from explorer URLs.
    pub async fn get_all_chains_info(&self) -> anyhow::Result<Vec<chains::Model>> {
        let chains = self.db.get_all_chains().await?;
        let mut out = Vec::with_capacity(chains.len());
        for chain in chains {
            let normalized = normalize_chain(chain);
            out.push(normalized);
        }
        Ok(out)
    }

    /// Preloads all chains from database into cache.
    /// Only chains with valid names are cached.
    /// Useful for warming up the cache at service startup.
    pub async fn preload_all(&self) -> anyhow::Result<()> {
        let chains = self.db.get_all_chains().await?;

        let mut cache = self.cache.write();
        for chain in chains {
            let has_name = has_valid_name(&chain);
            let normalized = normalize_chain(chain);
            if has_name {
                cache.insert(normalized.id as i64, normalized);
            }
        }

        tracing::info!(count = cache.len(), "Preloaded chains into cache");
        Ok(())
    }

    /// Invalidates the entire cache, forcing fresh DB queries on next access.
    pub fn invalidate_cache(&self) {
        self.cache.write().clear();
        self.unknown_name_last_query.write().clear();
    }

    /// Invalidates a specific chain from cache.
    pub fn invalidate_chain(&self, chain_id: i64) {
        self.cache.write().remove(&chain_id);
        self.unknown_name_last_query.write().remove(&chain_id);
    }

    /// Checks if we're in cooldown period for a chain with unknown name.
    fn is_in_cooldown(&self, chain_id: i64) -> bool {
        if let Some(last_query) = self.unknown_name_last_query.read().get(&chain_id) {
            last_query.elapsed() < self.settings.cooldown_interval
        } else {
            false
        }
    }
}

#[cfg(test)]
impl ChainInfoService {
    pub(crate) fn cache_contains_chain(&self, chain_id: i64) -> bool {
        self.cache.read().contains_key(&chain_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InterchainDatabase, test_utils::init_db};
    use interchain_indexer_entity::chains;
    use sea_orm::ActiveValue::Set;
    use serde_json::json;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_all_chains_info_returns_all_rows_ordered_by_chain_id() {
        let db = init_db("chain_info_get_all_ordered").await;
        let interchain_db = InterchainDatabase::new(db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(300),
                    name: Set("Third".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(10),
                    name: Set("First".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(200),
                    name: Set("Second".to_string()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        let svc =
            ChainInfoService::new(Arc::new(interchain_db), ChainInfoServiceSettings::default());
        let rows = svc.get_all_chains_info().await.unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].id, 10);
        assert_eq!(rows[1].id, 200);
        assert_eq!(rows[2].id, 300);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_all_chains_info_normalizes_and_updates_cache() {
        let db = init_db("chain_info_get_all_norm").await;
        let interchain_db = InterchainDatabase::new(db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("".to_string()),
                    explorer: Set(Some("https://one.example/".to_string())),
                    custom_routes: Set(Some(json!({
                        "tx": "/tx/{hash}",
                        "address": "/custom/addr",
                        "token": "/token/{hash}"
                    }))),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("Named".to_string()),
                    explorer: Set(Some("https://two.example".to_string())),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        let svc =
            ChainInfoService::new(Arc::new(interchain_db), ChainInfoServiceSettings::default());
        svc.invalidate_cache();

        let rows = svc.get_all_chains_info().await.unwrap();
        assert_eq!(rows.len(), 2);

        assert_eq!(rows[0].name, super::UNKNOWN_CHAIN_NAME);
        assert_eq!(rows[0].explorer.as_deref(), Some("https://one.example"));
        let routes = rows[0].custom_routes.as_ref().unwrap().as_object().unwrap();
        assert!(!routes.contains_key("tx"));
        assert!(!routes.contains_key("token"));
        assert_eq!(
            routes.get("address").and_then(|v| v.as_str()),
            Some("/custom/addr")
        );

        assert_eq!(rows[1].name, "Named");
        assert_eq!(rows[1].explorer.as_deref(), Some("https://two.example"));

        assert!(!svc.cache_contains_chain(1));
        assert!(svc.cache_contains_chain(2));
    }
}
