// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::env_merge;
use alloy::{
    network::Ethereum,
    primitives::{Address, ChainId},
    providers::DynProvider,
};
use anyhow::{Context, Result, anyhow, bail, ensure};
use interchain_indexer_entity::{
    bridge_contracts, bridges, chains, sea_orm_active_enums::BridgeType,
};
use interchain_indexer_logic::{
    CredentialHeader, NodeConfig, PoolConfig, Secret, build_layered_http_provider, redact_urls,
};
use sea_orm::{ActiveValue, entity::ActiveEnum};
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::HashMap, path::Path, str::FromStr, time::Duration};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexerType {
    IcmIctt,
    #[serde(rename = "amb")]
    #[allow(clippy::upper_case_acronyms)]
    AMB,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BridgeConfig {
    pub bridge_id: i32,
    pub name: String,
    #[serde(rename = "type", deserialize_with = "deserialize_bridge_type")]
    pub bridge_type: BridgeType,
    #[serde(default)]
    pub indexer_type: IndexerType,
    pub enabled: bool,
    pub api_url: Option<String>,
    pub ui_url: Option<String>,
    pub docs_url: Option<String>,
    /// When true, process messages involving at least one unknown chain
    /// (i.e. a chain not in `contracts`). When false (default), both endpoints
    /// must be configured chains.
    #[serde(default)]
    pub process_unknown_chains: bool,
    /// Optional chain id that narrows processing to messages where one endpoint
    /// equals this chain. Must be one of the chains configured in `contracts`.
    /// Validated at startup.
    #[serde(default)]
    pub home_chain_id: Option<ChainId>,
    /// When true (default), an incoming ICTT transfer from a chain that is
    /// not configured for this bridge is reconstructed from the ICM payload.
    /// Avalanche-only; ignored by other indexer types.
    #[serde(default = "default_reconstruct_incoming_ictt_transfers")]
    pub reconstruct_incoming_ictt_transfers: bool,
    pub contracts: Vec<BridgeContractConfig>,
}

fn default_reconstruct_incoming_ictt_transfers() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BridgeContractConfig {
    pub chain_id: i64,
    #[serde(deserialize_with = "deserialize_address")]
    pub address: Vec<u8>,
    pub version: i16,
    /// Must be at least `1`. `0` is rejected by `load_bridges_impl`: a
    /// completed catch-up persists `genesis_block.saturating_sub(1)`
    /// (`log_stream.rs`), which saturates to `0` at `started_at_block == 0`
    /// and makes the completed interval indistinguishable from one block of
    /// remaining work (`CatchupProgress::compute` in
    /// `interchain-indexer-logic/src/indexer/progress.rs`) — the empty
    /// interval below block zero simply cannot be represented in `u64`.
    pub started_at_block: u64,
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "deserialize_abi")]
    pub abi: Option<String>,
}

/// Deserialize bridge type from JSON string using SeaORM ActiveEnum
fn deserialize_bridge_type<'de, D>(deserializer: D) -> Result<BridgeType, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    BridgeType::try_from_value(&s).map_err(serde::de::Error::custom)
}

/// Deserialize an ABI from either a JSON string (file form) or inline JSON
/// (env-override form), normalizing both to the string representation.
fn deserialize_abi<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => Some(s),
        other => Some(other.to_string()),
    })
}

/// Deserialize an Ethereum address from a hex string to Vec<u8>
fn deserialize_address<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    // Parse address from hex string (with or without 0x prefix)
    let addr = Address::from_str(&s) //Address::from_str(&s).unwrap();
        .map_err(|e| serde::de::Error::custom(format!("Invalid address format: {}", e)))?;
    Ok(addr.as_slice().to_vec())
}

/// Convert BridgeConfig to bridges::ActiveModel for database operations
impl From<BridgeConfig> for bridges::ActiveModel {
    fn from(config: BridgeConfig) -> Self {
        bridges::ActiveModel {
            id: ActiveValue::Set(config.bridge_id),
            name: ActiveValue::Set(config.name),
            r#type: ActiveValue::Set(Some(config.bridge_type)),
            enabled: ActiveValue::Set(config.enabled),
            api_url: ActiveValue::Set(config.api_url),
            ui_url: ActiveValue::Set(config.ui_url),
            docs_url: ActiveValue::Set(config.docs_url),
            ..Default::default()
        }
    }
}

/// Convert bridges::Model to BridgeConfig
/// Note: This conversion loses the `indexer` field and `contracts` as they are not stored in the bridges table
impl From<bridges::Model> for BridgeConfig {
    fn from(model: bridges::Model) -> Self {
        BridgeConfig {
            bridge_id: model.id,
            name: model.name,
            bridge_type: model.r#type.expect("bridge must have a type"),
            indexer_type: Default::default(), // Not stored in database
            enabled: model.enabled,
            api_url: model.api_url,
            ui_url: model.ui_url,
            docs_url: model.docs_url,
            process_unknown_chains: false,
            home_chain_id: None,
            reconstruct_incoming_ictt_transfers: true,
            contracts: vec![], // Contracts are in a separate table
        }
    }
}

/// Convert BridgeContractConfig to bridge_contracts::ActiveModel for database operations
/// Note: `bridge_id` must be set separately as it's not part of BridgeContractConfig
impl BridgeContractConfig {
    pub fn to_active_model(&self, bridge_id: i32) -> bridge_contracts::ActiveModel {
        let abi_value = match &self.abi {
            None => None,
            Some(abi_str) => match serde_json::from_str::<serde_json::Value>(abi_str) {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(
                        err = %e,
                        abi_preview = %abi_str.chars().take(500).collect::<String>(),
                        "Invalid ABI JSON in bridge contract config, treating as None"
                    );
                    None
                }
            },
        };

        bridge_contracts::ActiveModel {
            bridge_id: ActiveValue::Set(bridge_id),
            chain_id: ActiveValue::Set(self.chain_id),
            address: ActiveValue::Set(self.address.clone()),
            version: ActiveValue::Set(self.version),
            kind: ActiveValue::Set(self.kind.clone()),
            started_at_block: ActiveValue::Set(Some(
                i64::try_from(self.started_at_block).expect("started_at_block must fit into i64"),
            )),
            abi: ActiveValue::Set(abi_value),
            ..Default::default()
        }
    }
}

/// Convert bridge_contracts::Model to BridgeContractConfig
/// Note: This conversion loses the `id` and `bridge_id` fields
impl From<bridge_contracts::Model> for BridgeContractConfig {
    fn from(model: bridge_contracts::Model) -> Self {
        let started_at_block = model.validated_started_at_block();
        let abi_string = model.abi.and_then(|json| serde_json::to_string(&json).ok());

        BridgeContractConfig {
            chain_id: model.chain_id,
            address: model.address,
            version: model.version,
            started_at_block,
            kind: model.kind,
            abi: abi_string,
        }
    }
}

/// Convert ChainConfig to chains::ActiveModel for database operations
impl From<ChainConfig> for chains::ActiveModel {
    fn from(config: ChainConfig) -> Self {
        // Build custom_routes JSON from ExplorerConfig fields
        let custom_routes = {
            let mut routes = serde_json::Map::new();
            if let Some(tx_route) = &config.explorer.custom_tx_route {
                routes.insert(
                    "tx".to_string(),
                    serde_json::Value::String(tx_route.clone()),
                );
            }
            if let Some(address_route) = &config.explorer.custom_address_route {
                routes.insert(
                    "address".to_string(),
                    serde_json::Value::String(address_route.clone()),
                );
            }
            if let Some(token_route) = &config.explorer.custom_token_route {
                routes.insert(
                    "token".to_string(),
                    serde_json::Value::String(token_route.clone()),
                );
            }
            if routes.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(routes))
            }
        };

        chains::ActiveModel {
            id: ActiveValue::Set(config.chain_id),
            name: ActiveValue::Set(config.name),
            icon: ActiveValue::Set(if config.icon.is_empty() {
                None
            } else {
                Some(config.icon)
            }),
            explorer: ActiveValue::Set(if config.explorer.url.is_empty() {
                None
            } else {
                Some(config.explorer.url)
            }),
            custom_routes: ActiveValue::Set(custom_routes),
            ..Default::default()
        }
    }
}

/// Convert chains::Model to ChainConfig
/// Note: This conversion loses the `rpcs` field as it's not stored in the chains table.
impl From<chains::Model> for ChainConfig {
    fn from(model: chains::Model) -> Self {
        // Extract custom routes from JSON
        let (custom_tx_route, custom_address_route, custom_token_route) =
            if let Some(routes) = &model.custom_routes {
                (
                    routes.get("tx").and_then(|v| v.as_str()).map(String::from),
                    routes
                        .get("address")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    routes
                        .get("token")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                )
            } else {
                (None, None, None)
            };

        ChainConfig {
            chain_id: model.id,
            name: model.name,
            icon: model.icon.unwrap_or_default(),
            explorer: ExplorerConfig {
                url: model.explorer.unwrap_or_default(),
                custom_tx_route,
                custom_address_route,
                custom_token_route,
            },
            pool_config: PoolConfig::default(),
            rpcs: vec![], // RPCs are not stored in database
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ExplorerConfig {
    #[serde(default)]
    pub url: String,
    pub custom_tx_route: Option<String>,
    pub custom_address_route: Option<String>,
    pub custom_token_route: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChainConfig {
    pub chain_id: i64,
    pub name: String,
    pub icon: String,
    #[serde(default)]
    pub explorer: ExplorerConfig,
    #[serde(default)]
    pub pool_config: PoolConfig,
    pub rpcs: Vec<HashMap<String, RpcProviderConfig>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RpcProviderConfig {
    pub url: String,
    #[serde(default = "default_rpc_enabled")]
    pub enabled: bool,
    /// Position of this provider inside its chain's failover pool, ascending:
    /// `0` is tried first and becomes the startup primary
    /// (`PoolState::primary_index`). Providers without an `order` rank after
    /// every provider that has one, so leaving it unset means "no preference".
    ///
    /// `u32` on purpose: a negative order has no meaning — unset already covers
    /// "put me last" — and `deny_unknown_fields`-style strictness makes a
    /// negative value fail startup instead of being reinterpreted.
    ///
    /// This is the only way to express intent: JSON object key order does not
    /// survive loading (see [`ranked_rpc_providers`]).
    #[serde(default)]
    pub order: Option<u32>,
    #[serde(default = "default_max_rps")]
    max_rps: u32,
    #[serde(default = "default_error_threshold")]
    error_threshold: u32,
    #[serde(default = "default_cooldown_threshold")]
    cooldown_threshold: u32,
    #[serde(default = "default_cooldown_secs")]
    cooldown_secs: u64,
    #[serde(default = "default_multicall_batching_us")]
    multicall_batching_us: u64,
    #[serde(default)]
    pub api_key: Option<ApiKeyConfig>,
}

fn default_rpc_enabled() -> bool {
    true
}

fn default_max_rps() -> u32 {
    10
}

fn default_error_threshold() -> u32 {
    3
}

fn default_cooldown_threshold() -> u32 {
    1
}

fn default_cooldown_secs() -> u64 {
    60
}

fn default_multicall_batching_us() -> u64 {
    60
}

/// Where a provider's API key belongs in the outbound request.
///
/// Typed rather than a `String` so a typo fails at startup, like every other
/// config mistake in this service.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyLocation {
    /// Sent as a request header. Preferred: the value can be marked sensitive,
    /// so it stays out of every rendering, including third-party ones.
    Header,
    /// Appended as a query parameter. The key becomes part of the URL.
    Query,
    /// Substituted into a `:<param_name>` placeholder in `url`.
    Path,
}

impl ApiKeyLocation {
    /// The spelling used in `chains.json`.
    ///
    /// Config errors must quote the JSON spelling, not the Rust variant name: an
    /// operator told `location "Query"` has to guess that the file wants
    /// `"query"`, which is exactly the guess a config error should remove.
    fn as_str(&self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Query => "query",
            Self::Path => "path",
        }
    }
}

/// Declares the *shape* of a provider credential. Deliberately holds no value:
/// the secret comes from the environment, so it cannot be committed to a config
/// file even by accident.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApiKeyConfig {
    pub location: ApiKeyLocation,
    /// Header name, query parameter name, or path placeholder name.
    pub param_name: String,
    /// Optional value prefix, e.g. `Bearer` for `Authorization: Bearer <key>`.
    /// `header` only — rejected for `query` and `path`.
    #[serde(default)]
    pub prefix: Option<String>,
    /// Environment variable holding the secret. When unset, the variable name is
    /// derived — see `derived_api_key_env_var`.
    #[serde(default)]
    pub value_env: Option<String>,
}

/// Env var prefix carrying RPC provider API key secrets.
const RPC_API_KEY_ENV_PREFIX: &str = "INTERCHAIN_INDEXER_RPC_API_KEY";

/// Name of the environment variable holding `provider_name`'s key on `chain_id`.
///
/// Mirrors the `<CHAIN_ID>__<PROVIDER>` addressing already used by
/// `INTERCHAIN_INDEXER_CHAINS__<CHAIN_ID>__RPCS__<PROVIDER>__*`, so an operator
/// who can address a provider to tune `max_rps` can address it for its key.
/// Provider names are uppercased and any character outside `[A-Z0-9_]` becomes
/// `_`, since provider names are free-form labels but env var names are not.
///
/// Single underscore before `RPC_API_KEY` is deliberate and load-bearing: the
/// main `Settings` env source uses
/// `config::Environment::with_prefix("INTERCHAIN_INDEXER").separator("__")`,
/// whose effective prefix is `interchain_indexer__` (double underscore), so a
/// single-underscore sibling prefix is invisible to it. This is the same
/// argument that makes the existing `INTERCHAIN_INDEXER_CHAINS__` /
/// `INTERCHAIN_INDEXER_BRIDGES__` prefixes safe.
fn derived_api_key_env_var(chain_id: i64, provider_name: &str) -> String {
    let normalized: String = provider_name
        .to_ascii_uppercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{RPC_API_KEY_ENV_PREFIX}__{chain_id}__{normalized}")
}

/// Resolve a provider's API key secret from the environment.
///
/// Exactly one variable is consulted — `value_env` when set, otherwise the
/// derived name. Never a fallback chain: a typo must fail loudly instead of
/// silently selecting some other variable.
fn resolve_api_key(
    chain_id: i64,
    provider_name: &str,
    api_key: &ApiKeyConfig,
    vars: &HashMap<String, String>,
) -> Result<Secret<String>> {
    // Validate the declaration itself before touching the environment: a
    // config mistake must be reported as a config mistake, not sent to the
    // operator as "the variable is unset" when the variable was never the
    // problem.
    ensure!(
        api_key.prefix.is_none() || matches!(api_key.location, ApiKeyLocation::Header),
        "chain {chain_id} provider \"{provider_name}\" declares api_key.prefix with \
         location \"{}\", but prefix is only supported for location \"header\"",
        api_key.location.as_str(),
    );

    let env_var = match &api_key.value_env {
        Some(explicit) => explicit.clone(),
        None => derived_api_key_env_var(chain_id, provider_name),
    };

    let value = vars
        .get(&env_var.to_ascii_uppercase())
        .map(String::as_str)
        .unwrap_or_default()
        .trim();
    ensure!(
        !value.is_empty(),
        "chain {chain_id} provider \"{provider_name}\" declares an api_key but \
         environment variable {env_var} is unset or empty"
    );

    Ok(Secret::new(value.to_string()))
}

/// Env var prefix for overriding/extending the chains config (see README).
const CHAINS_ENV_PREFIX: &str = "INTERCHAIN_INDEXER_CHAINS";
/// Env var prefix for overriding/extending the bridges config (see README).
const BRIDGES_ENV_PREFIX: &str = "INTERCHAIN_INDEXER_BRIDGES";

/// Load and deserialize chains from a JSON file, with
/// `INTERCHAIN_INDEXER_CHAINS*` env overrides deep-merged on top.
pub fn load_chains_from_file<P: AsRef<Path>>(path: P) -> Result<Vec<ChainConfig>> {
    load_chains_impl(path, std::env::vars())
}

fn load_chains_impl<P: AsRef<Path>>(
    path: P,
    vars: impl Iterator<Item = (String, String)>,
) -> Result<Vec<ChainConfig>> {
    let mut value = read_config_array(path.as_ref(), "chains")?;

    let applied = env_merge::apply_env_overrides(
        &mut value,
        CHAINS_ENV_PREFIX,
        vars,
        &env_merge::CHAINS_RULES,
    )?;
    log_applied_overrides(&applied, "chains");

    serde_json::from_value(value).with_context(|| {
        format!(
            "Failed to parse chains config JSON (after env overrides): {:?}",
            path.as_ref()
        )
    })
}

/// Load and deserialize bridges from a JSON file, with
/// `INTERCHAIN_INDEXER_BRIDGES*` env overrides deep-merged on top.
pub fn load_bridges_from_file<P: AsRef<Path>>(path: P) -> Result<Vec<BridgeConfig>> {
    load_bridges_impl(path, std::env::vars())
}

fn load_bridges_impl<P: AsRef<Path>>(
    path: P,
    vars: impl Iterator<Item = (String, String)>,
) -> Result<Vec<BridgeConfig>> {
    let mut value = read_config_array(path.as_ref(), "bridges")?;

    let applied = env_merge::apply_env_overrides(
        &mut value,
        BRIDGES_ENV_PREFIX,
        vars,
        &env_merge::BRIDGES_RULES,
    )?;
    log_applied_overrides(&applied, "bridges");

    let bridges: Vec<BridgeConfig> = serde_json::from_value(value).with_context(|| {
        format!(
            "Failed to parse bridges config JSON (after env overrides): {:?}",
            path.as_ref()
        )
    })?;
    validate_started_at_blocks(&bridges)?;
    validate_bridge_ids(&bridges)?;

    Ok(bridges)
}

/// Rejects `started_at_block == 0`: the empty interval below block zero
/// cannot be represented in `u64`, so a completed catch-up starting at
/// genesis would be permanently unrepresentable (see the doc comment on
/// `BridgeContractConfig::started_at_block`). Config typos fail hard in this
/// repo (`deny_unknown_fields` everywhere); this is the same convention
/// applied to a semantically invalid value rather than a structural one.
fn validate_started_at_blocks(bridges: &[BridgeConfig]) -> Result<()> {
    for bridge in bridges {
        for contract in &bridge.contracts {
            ensure!(
                contract.started_at_block != 0,
                "bridge {} chain {} has started_at_block = 0, which is invalid: the empty \
                 interval below block zero cannot be represented, so configure a \
                 started_at_block of at least 1",
                bridge.bridge_id,
                contract.chain_id,
            );
        }
    }
    Ok(())
}

/// Rejects a negative `bridge_id`: the public API exposes bridge ids as
/// `uint32` (`BridgeInfo.id`, `Bridge.id`, `Pagination.bridge_id`), so a
/// negative id has no representation there. Config typos fail hard in this
/// repo; this is the same convention applied to a semantically invalid value.
fn validate_bridge_ids(bridges: &[BridgeConfig]) -> Result<()> {
    for bridge in bridges {
        ensure!(
            bridge.bridge_id >= 0,
            "bridge {} has a negative bridge_id, which cannot be represented \
             in the public API (bridge ids are uint32); use a non-negative id",
            bridge.bridge_id,
        );
    }
    Ok(())
}

fn log_applied_overrides(applied: &[env_merge::AppliedOverride], kind: &str) {
    for o in applied {
        // No raw config values at info level: RPC URLs and similar fields may
        // embed API keys. Replaced fields are identified by path at info;
        // the old/new values are available at debug for troubleshooting.
        tracing::info!(var = %o.var, path = %o.json_path, kind, "applied config env override");
        for overwrite in &o.overwrites {
            tracing::info!(
                var = %o.var,
                path = %overwrite.path,
                kind,
                "config env override replaced an existing value"
            );
            tracing::debug!(
                var = %o.var,
                path = %overwrite.path,
                old = %overwrite.old,
                new = %overwrite.new,
                kind,
                "config env override replacement values"
            );
        }
    }
}

fn read_config_array(path: &Path, kind: &str) -> Result<serde_json::Value> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {kind} config file: {path:?}"))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {kind} config JSON: {path:?}"))?;
    anyhow::ensure!(
        value.is_array(),
        "{kind} config must be a JSON array: {path:?}"
    );
    Ok(value)
}

/// Create layered Alloy providers from ChainConfig definitions.
/// Returns a HashMap mapping chain_id (as i64) to a DynProvider.
/// Only enabled RPC providers are included in each pool, ordered by
/// [`ranked_rpc_providers`] — the first node is the pool's primary.
pub async fn create_provider_pools_from_chains(
    chains: Vec<ChainConfig>,
) -> Result<HashMap<i64, DynProvider<Ethereum>>> {
    // `std::env::vars()`'s iterator type holds a non-`Send` guard internally,
    // so it cannot be part of this `async fn`'s state without breaking the
    // `Send` bound required of its future (this is `.await`ed from
    // `server::run`, whose own future must stay `Send`). Collecting first
    // yields a plain `Send` `Vec` iterator with identical contents.
    create_provider_pools_impl(chains, std::env::vars().collect::<Vec<_>>().into_iter()).await
}

/// Same as [`create_provider_pools_from_chains`], but takes its environment
/// variables as a parameter so tests can inject them without touching the
/// process environment (`std::env::set_var` is not isolated across the
/// multi-threaded test binary).
async fn create_provider_pools_impl(
    chains: Vec<ChainConfig>,
    vars: impl Iterator<Item = (String, String)>,
) -> Result<HashMap<i64, DynProvider<Ethereum>>> {
    let mut pools = HashMap::new();
    // `to_ascii_uppercase` on both sides of this map — here and at the lookup in
    // `resolve_api_key`. Unicode-aware `to_uppercase` on one side only would
    // build a key the other side can never produce.
    let vars: HashMap<String, String> = vars.map(|(k, v)| (k.to_ascii_uppercase(), v)).collect();

    for chain in chains {
        if chain.chain_id < 0 {
            tracing::warn!(
                chain_id = chain.chain_id,
                chain_name = chain.name,
                "Skipping chain with negative ID"
            );
            continue;
        }

        let node_configs = build_chain_node_configs(&chain, &vars)?;

        // Create layered provider for this chain if we have any nodes
        if !node_configs.is_empty() {
            // Node names carry no secrets (unlike the URLs, which may embed API
            // keys), and the resolved order — above all which node starts as
            // primary — is otherwise invisible to operators reading a
            // `Rotating primary RPC node` warning.
            let node_names: Vec<String> = node_configs.iter().map(|cfg| cfg.name.clone()).collect();
            // Non-empty by the branch condition; `first()` keeps startup
            // panic-free regardless.
            let primary = node_names.first().map_or("", String::as_str);
            let pool_order = node_names.join(", ");

            // Check for duplicate chain_id in config
            if pools.contains_key(&chain.chain_id) {
                anyhow::bail!("Duplicate chain_id {} in chains config", chain.chain_id,);
            }

            match build_layered_http_provider(node_configs, chain.pool_config.clone()) {
                Ok(provider) => {
                    tracing::info!(
                        chain_id = chain.chain_id,
                        chain_name = chain.name,
                        // Recorded as strings, not `%`: the joined list contains
                        // ", " and would otherwise be indistinguishable from
                        // separate fields in the log line.
                        primary,
                        nodes = pool_order,
                        "Created layered provider for chain"
                    );
                    pools.insert(chain.chain_id, provider);
                }
                Err(e) => {
                    // A chain whose providers declare an `api_key` must not
                    // degrade to "warn and skip". The failures that reach here
                    // for such a chain are config errors — an invalid header
                    // name, a value that is not a legal header value, or a URL
                    // that stopped parsing after key substitution — and
                    // skipping would leave the service reporting healthy with a
                    // chain that has no providers at all, surfacing much later
                    // as `no provider configured for chain_id`. That is the same
                    // silent-outage shape the hard failure on a missing secret
                    // exists to prevent, so it fails the same way.
                    //
                    // Chains with no `api_key` keep the historical lenient
                    // behavior: one malformed endpoint should not stop a service
                    // that has other chains to index.
                    //
                    // The cause is rendered through `redact_urls` rather than
                    // reasoned about site by site: none of these three errors is
                    // known to carry the URL today, but this message is the one
                    // place a keyed chain's failure becomes operator-visible
                    // text, and "it happens not to leak right now" is exactly
                    // the assumption this change exists to stop relying on.
                    ensure!(
                        !chain_declares_api_key(&chain),
                        "failed to build the RPC provider pool for chain {} (\"{}\"), which \
                         declares an api_key: {}",
                        chain.chain_id,
                        chain.name,
                        redact_urls(&format!("{e:#}")),
                    );

                    tracing::warn!(
                        chain_id = chain.chain_id,
                        chain_name = chain.name,
                        err = %redact_urls(&format!("{e:#}")),
                        "Failed to create layered provider for chain, skipping"
                    );
                }
            }
        } else {
            tracing::warn!(
                chain_id = chain.chain_id,
                chain_name = chain.name,
                "No enabled RPC providers found for chain, skipping provider creation"
            );
        }
    }

    Ok(pools)
}

/// Build one chain's ordered [`NodeConfig`] list, resolving every declared
/// `api_key` from `vars`.
///
/// Split out of [`create_provider_pools_impl`] to be testable: that function
/// returns an opaque `DynProvider`, so the credential wiring — which header a
/// node ends up carrying, and whether a `query`/`path` key made it into the URL
/// — is not observable through its result. Here it is a plain return value.
fn build_chain_node_configs(
    chain: &ChainConfig,
    vars: &HashMap<String, String>,
) -> Result<Vec<NodeConfig>> {
    let mut node_configs = Vec::new();

    // Pool order is load-bearing: element 0 is the startup primary and
    // failover walks the pool from there, so rank before building.
    for (provider_name, rpc_config) in ranked_rpc_providers(chain) {
        let (secret, credential_header) = match rpc_config.api_key.as_ref() {
            None => (None, None),
            Some(api_key) => {
                let secret = resolve_api_key(chain.chain_id, provider_name, api_key, vars)
                    .with_context(|| {
                        format!(
                            "failed to resolve api_key for chain {} provider \"{provider_name}\"",
                            chain.chain_id
                        )
                    })?;
                let credential_header = match api_key.location {
                    ApiKeyLocation::Header => Some(CredentialHeader {
                        name: api_key.param_name.clone(),
                        value: Secret::new(match &api_key.prefix {
                            Some(p) => format!("{p} {}", secret.expose()),
                            None => secret.expose().clone(),
                        }),
                    }),
                    ApiKeyLocation::Query | ApiKeyLocation::Path => None,
                };
                (Some(secret), credential_header)
            }
        };

        let node_config = NodeConfig {
            name: format!("{}[{}]", chain.name, provider_name),
            http_url: Secret::new(build_rpc_url(
                &rpc_config.url,
                rpc_config.api_key.as_ref(),
                secret.as_ref(),
            )?),
            credential_header,
            max_rps: rpc_config.max_rps,
            error_threshold: rpc_config.error_threshold,
            cooldown_threshold: rpc_config.cooldown_threshold,
            cooldown: Duration::from_secs(rpc_config.cooldown_secs),
            multicall_batching_wait: Duration::from_micros(rpc_config.multicall_batching_us),
        };

        node_configs.push(node_config);
    }

    Ok(node_configs)
}

/// Whether any *enabled* provider of `chain` declares an `api_key`.
///
/// Decides whether a pool-construction failure is fatal: see the `Err` arm in
/// [`create_provider_pools_impl`]. Reuses [`ranked_rpc_providers`] so it sees
/// exactly the providers the pool would have been built from — a disabled
/// provider's stale `api_key` must not make an unrelated failure fatal.
fn chain_declares_api_key(chain: &ChainConfig) -> bool {
    ranked_rpc_providers(chain)
        .iter()
        .any(|(_, rpc)| rpc.api_key.is_some())
}

/// Enabled RPC providers of a chain, in the order the failover pool should try
/// them: element 0 becomes the primary, and `PoolState::pick_node` walks the
/// rest round-robin from there.
///
/// Ordering key, in precedence order:
///
/// 1. `order` — ascending; providers without one rank after all that have one
///    (`u32::MAX`).
/// 2. Position of the containing object in the `rpcs` **array** — a JSON array,
///    so this order does survive loading.
/// 3. Provider name, alphabetically.
///
/// Steps 2 and 3 exist because the order of keys *within* one `rpcs` object is
/// not recoverable: config loading routes every file through
/// `serde_json::Value` (whose `Map` is a `BTreeMap` without the `preserve_order`
/// feature) and then into `HashMap`, whose iteration order is randomly seeded
/// per process. Sorting on the name instead of the map's iteration order is
/// what makes the pool — and therefore the startup primary — stable across
/// restarts and identical across replicas.
fn ranked_rpc_providers(chain: &ChainConfig) -> Vec<(&String, &RpcProviderConfig)> {
    let mut ranked: Vec<_> = chain
        .rpcs
        .iter()
        .enumerate()
        .flat_map(|(map_index, rpc_map)| {
            rpc_map
                .iter()
                .filter(|(_, rpc)| rpc.enabled)
                .map(move |(name, rpc)| (map_index, name, rpc))
        })
        .collect();

    ranked.sort_unstable_by(|(a_index, a_name, a_rpc), (b_index, b_name, b_rpc)| {
        (a_rpc.order.unwrap_or(u32::MAX), a_index, a_name).cmp(&(
            b_rpc.order.unwrap_or(u32::MAX),
            b_index,
            b_name,
        ))
    });

    ranked
        .into_iter()
        .map(|(_, name, rpc)| (name, rpc))
        .collect()
}

/// Apply a URL-embedded API key to `url`.
///
/// `header` keys are not URL-embedded and return `url` unchanged — they are
/// attached to the transport instead (see `NodeConfig::credential_header`).
///
/// Errors must never interpolate `url` or the key: the caller adds chain and
/// provider context, which is the diagnostic an operator actually needs.
fn build_rpc_url(
    url: &str,
    api_key: Option<&ApiKeyConfig>,
    secret: Option<&Secret<String>>,
) -> Result<String> {
    let Some(api_key) = api_key else {
        return Ok(url.to_string());
    };

    match api_key.location {
        ApiKeyLocation::Header => Ok(url.to_string()),
        ApiKeyLocation::Query => {
            let key = secret
                .context("api_key.location \"query\" requires a resolved secret")?
                .expose();
            let mut parsed = url::Url::parse(url).context("api_key url is not a valid URL")?;
            parsed
                .query_pairs_mut()
                .append_pair(&api_key.param_name, key);
            Ok(parsed.to_string())
        }
        ApiKeyLocation::Path => {
            let key = secret
                .context("api_key.location \"path\" requires a resolved secret")?
                .expose();
            let placeholder = format!(":{}", api_key.param_name);
            // The key is spliced into the path, so it has to be encoded as
            // exactly one segment. An unescaped `/` would silently restructure
            // the path and an unescaped `?`/`#` would truncate it into a query
            // or fragment — either way the request goes to a different
            // endpoint than the operator configured, which surfaces as a
            // confusing 404 rather than an auth failure.
            let encoded_key = encode_path_segment(key)?;
            let Some(substituted) = replace_path_placeholder(url, &placeholder, &encoded_key)
            else {
                bail!(
                    "api_key location \"path\" declared with param_name \"{}\" but the url \
                     contains no \"{placeholder}\" placeholder",
                    api_key.param_name,
                );
            };
            Ok(substituted)
        }
    }
}

/// Percent-encode `value` so it occupies exactly one URL path segment.
///
/// Encoding is delegated to the `url` crate's own path-segment set rather than
/// a hand-maintained `AsciiSet`: characters that would restructure the URL
/// (`/`, `?`, `#`, `%`, controls) are escaped, while the unreserved and
/// sub-delimiter characters providers legitimately use in keys are preserved.
fn encode_path_segment(value: &str) -> Result<String> {
    const BASE: &str = "http://api-key-encoding.invalid/";

    let mut probe = url::Url::parse(BASE).context("failed to build the api_key encoding url")?;
    probe
        .path_segments_mut()
        .map_err(|()| anyhow!("failed to build the api_key encoding url"))?
        .clear()
        .push(value);

    Ok(probe
        .path()
        .strip_prefix('/')
        .unwrap_or(probe.path())
        .to_string())
}

/// Substitute every *complete* `placeholder` occurrence in `url` with `value`.
///
/// A plain `str::replace` also rewrites longer placeholders that merely start
/// with this name — `:api` would corrupt a `:api_key` placeholder into
/// `<key>_key` — so a match only counts when the next character cannot itself
/// be part of a placeholder name. Returns `None` when no complete placeholder
/// is present, which is the caller's startup-failure signal.
fn replace_path_placeholder(url: &str, placeholder: &str, value: &str) -> Option<String> {
    let mut out = String::with_capacity(url.len());
    let mut rest = url;
    let mut replaced = false;

    while let Some(at) = rest.find(placeholder) {
        let (before, from_match) = rest.split_at(at);
        let tail = &from_match[placeholder.len()..];
        let complete = tail
            .chars()
            .next()
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_' && c != '-');

        out.push_str(before);
        out.push_str(if complete { value } else { placeholder });
        replaced |= complete;
        rest = tail;
    }
    out.push_str(rest);

    replaced.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_bridge_without_home_chain_field() {
        let json = r#"
        [
            {
                "bridge_id": 7,
                "name": "No Home Chain",
                "type": "avalanche_native",
                "indexer_type": "icm_ictt",
                "enabled": true,
                "api_url": null,
                "ui_url": null,
                "docs_url": null,
                "contracts": []
            }
        ]
        "#;

        let bridges: Vec<BridgeConfig> = serde_json::from_str(json).unwrap();
        assert_eq!(bridges.len(), 1);
        assert!(!bridges[0].process_unknown_chains);
        assert_eq!(bridges[0].home_chain_id, None);
        assert!(
            bridges[0].reconstruct_incoming_ictt_transfers,
            "reconstruct_incoming_ictt_transfers must default to true so existing \
             config files stay valid and behavior does not change go-forward"
        );
    }

    #[test]
    fn test_deserialize_bridge_with_home_chain_id_field() {
        let json = r#"
        [
            {
                "bridge_id": 7,
                "name": "With Home Chain",
                "type": "avalanche_native",
                "indexer_type": "icm_ictt",
                "enabled": true,
                "api_url": null,
                "ui_url": null,
                "docs_url": null,
                "process_unknown_chains": true,
                "home_chain_id": 43114,
                "contracts": []
            }
        ]
        "#;

        let bridges: Vec<BridgeConfig> = serde_json::from_str(json).unwrap();
        assert_eq!(bridges.len(), 1);
        assert!(bridges[0].process_unknown_chains);
        assert_eq!(bridges[0].home_chain_id, Some(43114));
    }

    #[test]
    fn test_deserialize_bridge_with_reconstruct_incoming_ictt_transfers_false() {
        let json = r#"
        [
            {
                "bridge_id": 7,
                "name": "Reconstruction Disabled",
                "type": "avalanche_native",
                "indexer_type": "icm_ictt",
                "enabled": true,
                "api_url": null,
                "ui_url": null,
                "docs_url": null,
                "reconstruct_incoming_ictt_transfers": false,
                "contracts": []
            }
        ]
        "#;

        let bridges: Vec<BridgeConfig> = serde_json::from_str(json).unwrap();
        assert_eq!(bridges.len(), 1);
        assert!(!bridges[0].reconstruct_incoming_ictt_transfers);
    }

    #[test]
    fn test_model_to_bridge_config() {
        use interchain_indexer_entity::bridges;

        let model = bridges::Model {
            id: 1,
            name: "Test Bridge".to_string(),
            r#type: Some(BridgeType::Lockmint),
            enabled: true,
            api_url: Some("https://api.example.com".to_string()),
            ui_url: Some("https://ui.example.com".to_string()),
            docs_url: Some("https://docs.example.com".to_string()),
            created_at: None,
            updated_at: None,
        };

        let config: BridgeConfig = model.into();

        assert_eq!(config.bridge_id, 1);
        assert_eq!(config.name, "Test Bridge");
        assert_eq!(config.bridge_type, BridgeType::Lockmint);
        assert!(config.enabled);
        assert_eq!(config.api_url, Some("https://api.example.com".to_string()));
        assert_eq!(config.ui_url, Some("https://ui.example.com".to_string()));
        assert_eq!(
            config.docs_url,
            Some("https://docs.example.com".to_string())
        );
        // indexer and contracts are lost in conversion (not stored in DB)
        assert_eq!(config.indexer_type, IndexerType::Unknown);
        assert!(!config.process_unknown_chains);
        assert_eq!(config.home_chain_id, None);
        assert!(config.reconstruct_incoming_ictt_transfers);
        assert_eq!(config.contracts, vec![]);
    }

    #[test]
    fn test_bridge_contract_config_to_active_model() {
        let config = BridgeContractConfig {
            chain_id: 1,
            address: vec![0x12; 20],
            version: 1,
            started_at_block: 12345,
            kind: None,
            abi: None,
        };

        let active_model = config.to_active_model(100);

        assert!(matches!(active_model.bridge_id, ActiveValue::Set(100)));
        assert!(matches!(active_model.chain_id, ActiveValue::Set(1)));
        assert!(matches!(active_model.version, ActiveValue::Set(1)));
        assert!(matches!(
            active_model.started_at_block,
            ActiveValue::Set(Some(12345))
        ));
    }

    #[test]
    fn test_model_to_bridge_contract_config() {
        use interchain_indexer_entity::bridge_contracts;

        let model = bridge_contracts::Model {
            id: 1,
            bridge_id: 100,
            chain_id: 1,
            address: vec![0x12; 20],
            version: 1,
            abi: None,
            started_at_block: Some(12345),
            created_at: None,
            updated_at: None,
            kind: None,
        };

        let config: BridgeContractConfig = model.into();

        assert_eq!(config.chain_id, 1);
        assert_eq!(config.address, vec![0x12; 20]);
        assert_eq!(config.version, 1);
        assert_eq!(config.started_at_block, 12345);
    }

    #[test]
    fn test_model_to_bridge_contract_config_clamps_started_at_block() {
        use interchain_indexer_entity::bridge_contracts;

        let common = bridge_contracts::Model {
            id: 1,
            bridge_id: 100,
            chain_id: 1,
            address: vec![0x12; 20],
            version: 1,
            abi: None,
            created_at: None,
            updated_at: None,
            started_at_block: None,
            kind: None,
        };

        let none_block: BridgeContractConfig = common.clone().into();
        assert_eq!(none_block.started_at_block, 0);

        let negative_block: BridgeContractConfig = bridge_contracts::Model {
            started_at_block: Some(-42),
            ..common
        }
        .into();
        assert_eq!(negative_block.started_at_block, 0);
    }

    #[test]
    fn test_chain_config_to_active_model() {
        let config = ChainConfig {
            chain_id: 1,
            name: "Ethereum".to_string(),
            icon: "https://example.com/icon.png".to_string(),
            explorer: ExplorerConfig {
                url: "https://etherscan.io".to_string(),
                custom_tx_route: Some("/transaction/{hash}".to_string()),
                custom_address_route: None,
                custom_token_route: None,
            },
            pool_config: Default::default(),
            rpcs: vec![],
        };

        let active_model: chains::ActiveModel = config.clone().into();

        assert!(matches!(active_model.id, ActiveValue::Set(1)));
        assert!(matches!(active_model.name, ActiveValue::Set(ref name) if name == "Ethereum"));
        assert!(matches!(
            active_model.icon,
            ActiveValue::Set(Some(ref icon)) if icon == "https://example.com/icon.png"
        ));
        assert!(matches!(
            active_model.explorer,
            ActiveValue::Set(Some(ref url)) if url == "https://etherscan.io"
        ));
        // Check custom_routes contains the tx route
        if let ActiveValue::Set(Some(ref routes)) = active_model.custom_routes {
            assert_eq!(
                routes.get("tx").and_then(|v| v.as_str()),
                Some("/transaction/{hash}")
            );
            assert!(routes.get("address").is_none());
        } else {
            panic!("Expected custom_routes to be set");
        }
    }

    #[test]
    fn test_chain_config_to_active_model_empty_icon() {
        let config = ChainConfig {
            chain_id: 1,
            name: "Ethereum".to_string(),
            icon: String::new(),
            explorer: ExplorerConfig {
                url: String::new(),
                custom_tx_route: None,
                custom_address_route: None,
                custom_token_route: None,
            },
            pool_config: Default::default(),
            rpcs: vec![],
        };

        let active_model: chains::ActiveModel = config.into();

        assert!(matches!(active_model.icon, ActiveValue::Set(None)));
        assert!(matches!(active_model.explorer, ActiveValue::Set(None)));
        assert!(matches!(active_model.custom_routes, ActiveValue::Set(None)));
    }

    #[test]
    fn test_model_to_chain_config() {
        use interchain_indexer_entity::chains;

        let custom_routes = serde_json::json!({
            "tx": "/transaction/{hash}",
            "address": "/addr/{hash}"
        });

        let model = chains::Model {
            id: 1,
            name: "Ethereum".to_string(),
            icon: Some("https://example.com/icon.png".to_string()),
            explorer: Some("https://etherscan.io".to_string()),
            custom_routes: Some(custom_routes),
            created_at: None,
            updated_at: None,
        };

        let config: ChainConfig = model.into();

        assert_eq!(config.chain_id, 1);
        assert_eq!(config.name, "Ethereum");
        assert_eq!(config.icon, "https://example.com/icon.png");
        assert_eq!(config.explorer.url, "https://etherscan.io");
        assert_eq!(
            config.explorer.custom_tx_route,
            Some("/transaction/{hash}".to_string())
        );
        assert_eq!(
            config.explorer.custom_address_route,
            Some("/addr/{hash}".to_string())
        );
        assert_eq!(config.explorer.custom_token_route, None);
        // rpcs are lost in conversion (not stored in DB)
        assert_eq!(config.rpcs, vec![]);
    }

    #[test]
    fn test_deserialize_abi_accepts_string_and_inline_json_forms() {
        let file_form = r#"
        {
            "chain_id": 1,
            "address": "0x4C36d2919e407f0Cc2Ee3c993ccF8ac26d9CE64e",
            "version": 6,
            "started_at_block": 1,
            "kind": null,
            "abi": "[{\"name\":\"RelayedMessage\",\"type\":\"event\"}]"
        }
        "#;
        let env_form = r#"
        {
            "chain_id": 1,
            "address": "0x4C36d2919e407f0Cc2Ee3c993ccF8ac26d9CE64e",
            "version": 6,
            "started_at_block": 1,
            "kind": null,
            "abi": [{"name":"RelayedMessage","type":"event"}]
        }
        "#;

        let from_file: BridgeContractConfig = serde_json::from_str(file_form).unwrap();
        let from_env: BridgeContractConfig = serde_json::from_str(env_form).unwrap();

        assert_eq!(
            from_file.abi,
            Some(r#"[{"name":"RelayedMessage","type":"event"}]"#.to_string())
        );
        assert_eq!(from_file.abi, from_env.abi);

        let null_form = file_form.replace(
            r#""abi": "[{\"name\":\"RelayedMessage\",\"type\":\"event\"}]""#,
            r#""abi": null"#,
        );
        let from_null: BridgeContractConfig = serde_json::from_str(&null_form).unwrap();
        assert_eq!(from_null.abi, None);
    }

    fn ranked_names(chains_json: &str) -> Vec<String> {
        let chains: Vec<ChainConfig> = serde_json::from_str(chains_json)
            .unwrap_or_else(|e| panic!("failed to parse chains fixture: {e}"));
        ranked_rpc_providers(&chains[0])
            .into_iter()
            .map(|(name, _)| name.clone())
            .collect()
    }

    #[test]
    fn test_ranked_rpc_providers_orders_by_explicit_order_ascending() {
        let names = ranked_names(
            r#"[{"chain_id":1,"name":"Ethereum","icon":"","rpcs":[{
                "drpc":       {"url":"https://drpc",       "order": 10},
                "blockscout": {"url":"https://blockscout", "order": 0},
                "gateway":    {"url":"https://gateway",    "order": 1}
            }]}]"#,
        );
        assert_eq!(names, ["blockscout", "gateway", "drpc"]);
    }

    #[test]
    fn test_ranked_rpc_providers_unordered_rank_after_ordered() {
        // `1rpc` would win any name-based tie-break; an explicit order on
        // `gateway` must still place it first.
        let names = ranked_names(
            r#"[{"chain_id":1,"name":"Ethereum","icon":"","rpcs":[{
                "1rpc":    {"url":"https://1rpc"},
                "zeta":    {"url":"https://zeta"},
                "gateway": {"url":"https://gateway", "order": 7}
            }]}]"#,
        );
        assert_eq!(names, ["gateway", "1rpc", "zeta"]);
    }

    #[test]
    fn test_ranked_rpc_providers_without_order_ranks_by_map_then_name() {
        // Position in the `rpcs` array outranks the provider name: `beta` sits
        // in the second object, so it comes after both members of the first.
        let names = ranked_names(
            r#"[{"chain_id":1,"name":"Ethereum","icon":"","rpcs":[
                { "zeta": {"url":"https://zeta"}, "alpha": {"url":"https://alpha"} },
                { "beta": {"url":"https://beta"} }
            ]}]"#,
        );
        assert_eq!(names, ["alpha", "zeta", "beta"]);
    }

    #[test]
    fn test_ranked_rpc_providers_order_is_stable_across_parses() {
        // Each `HashMap` instance gets its own randomly seeded hasher, so an
        // implementation that leaned on map iteration order diverges here even
        // within a single process — let alone across restarts.
        let json = r#"[{"chain_id":1,"name":"Ethereum","icon":"","rpcs":[{
            "blockscout": {"url":"https://blockscout"},
            "gateway":    {"url":"https://gateway"},
            "drpc":       {"url":"https://drpc"},
            "1rpc":       {"url":"https://1rpc"}
        }]}]"#;

        let expected = ["1rpc", "blockscout", "drpc", "gateway"];
        for _ in 0..64 {
            assert_eq!(ranked_names(json), expected);
        }
    }

    #[test]
    fn test_ranked_rpc_providers_skips_disabled_providers() {
        let names = ranked_names(
            r#"[{"chain_id":1,"name":"Ethereum","icon":"","rpcs":[{
                "blockscout": {"url":"https://blockscout", "order": 0, "enabled": false},
                "gateway":    {"url":"https://gateway",    "order": 1}
            }]}]"#,
        );
        assert_eq!(names, ["gateway"], "a disabled provider must not be ranked");
    }

    #[test]
    fn test_rpc_provider_order_defaults_to_none() {
        let chains: Vec<ChainConfig> = serde_json::from_str(CHAINS_FILE).unwrap();
        assert_eq!(chains[0].rpcs[0]["drpc"].order, None);
    }

    #[test]
    fn test_rpc_provider_order_rejects_negative_value() {
        // Unset already means "rank me last", so a negative order has no
        // meaning and must fail loudly rather than be reinterpreted.
        let err = serde_json::from_str::<Vec<ChainConfig>>(
            r#"[{"chain_id":1,"name":"Ethereum","icon":"","rpcs":[{
                "drpc": {"url":"https://drpc", "order": -1}
            }]}]"#,
        )
        .expect_err("a negative order must be rejected");
        assert!(
            err.to_string().contains("invalid value"),
            "unexpected error: {err}"
        );
    }

    // --- api_key: derived env var name ---

    #[test]
    fn test_derived_api_key_env_var_plain_name() {
        assert_eq!(
            derived_api_key_env_var(1, "drpc"),
            "INTERCHAIN_INDEXER_RPC_API_KEY__1__DRPC"
        );
    }

    #[test]
    fn test_derived_api_key_env_var_mixed_case() {
        assert_eq!(
            derived_api_key_env_var(1, "DrPc"),
            "INTERCHAIN_INDEXER_RPC_API_KEY__1__DRPC"
        );
    }

    #[test]
    fn test_derived_api_key_env_var_replaces_non_alphanumeric_characters() {
        assert_eq!(
            derived_api_key_env_var(1, "my-node.io"),
            "INTERCHAIN_INDEXER_RPC_API_KEY__1__MY_NODE_IO"
        );
    }

    #[test]
    fn test_derived_api_key_env_var_leading_digit() {
        // `1rpc` exists in config/full-mainnet/chains.json.
        assert_eq!(
            derived_api_key_env_var(1, "1rpc"),
            "INTERCHAIN_INDEXER_RPC_API_KEY__1__1RPC"
        );
    }

    // --- api_key: build_rpc_url ---

    const FAKE_KEY: &str = "test-key-not-a-secret";

    fn api_key(location: ApiKeyLocation, param_name: &str, prefix: Option<&str>) -> ApiKeyConfig {
        ApiKeyConfig {
            location,
            param_name: param_name.to_string(),
            prefix: prefix.map(str::to_string),
            value_env: None,
        }
    }

    #[test]
    fn test_build_rpc_url_no_api_key_returns_url_unchanged() {
        let url = build_rpc_url("https://drpc.example/rpc", None, None).unwrap();
        assert_eq!(url, "https://drpc.example/rpc");
    }

    #[test]
    fn test_build_rpc_url_header_location_returns_url_unchanged() {
        let cfg = api_key(ApiKeyLocation::Header, "Authorization", None);
        let secret = Secret::new(FAKE_KEY.to_string());

        // This is the property that makes `header` the safe location: the URL
        // never carries the key, so nothing here ever needs redaction.
        let url = build_rpc_url("https://drpc.example/rpc", Some(&cfg), Some(&secret)).unwrap();
        assert_eq!(url, "https://drpc.example/rpc");
    }

    #[test]
    fn test_build_rpc_url_query_location_appends_param() {
        let cfg = api_key(ApiKeyLocation::Query, "apikey", None);
        let secret = Secret::new(FAKE_KEY.to_string());

        let url = build_rpc_url("https://drpc.example/rpc", Some(&cfg), Some(&secret)).unwrap();
        assert!(
            url.contains(&format!("?apikey={FAKE_KEY}")),
            "unexpected url: {url}"
        );
    }

    #[test]
    fn test_build_rpc_url_path_location_replaces_placeholder() {
        let cfg = api_key(ApiKeyLocation::Path, "api_key", None);
        let secret = Secret::new(FAKE_KEY.to_string());

        let url = build_rpc_url(
            "https://drpc.example/:api_key/rpc",
            Some(&cfg),
            Some(&secret),
        )
        .unwrap();
        assert_eq!(url, format!("https://drpc.example/{FAKE_KEY}/rpc"));
    }

    #[test]
    fn test_build_rpc_url_path_location_encodes_reserved_characters() {
        let cfg = api_key(ApiKeyLocation::Path, "api_key", None);
        // A key carrying the three characters that would restructure the URL:
        // `/` adds a path segment, `?` starts a query, `#` starts a fragment.
        let secret = Secret::new("ab/cd?ef#gh".to_string());

        let url = build_rpc_url(
            "https://drpc.example/:api_key/rpc",
            Some(&cfg),
            Some(&secret),
        )
        .unwrap();

        assert_eq!(url, "https://drpc.example/ab%2Fcd%3Fef%23gh/rpc");

        // The key must stay one segment, and the path after it must survive.
        let parsed = url::Url::parse(&url).unwrap();
        let segments: Vec<_> = parsed.path_segments().unwrap().collect();
        assert_eq!(segments, vec!["ab%2Fcd%3Fef%23gh", "rpc"]);
        assert_eq!(parsed.query(), None);
        assert_eq!(parsed.fragment(), None);
    }

    #[test]
    fn test_build_rpc_url_path_location_preserves_provider_allowed_characters() {
        let cfg = api_key(ApiKeyLocation::Path, "api_key", None);
        // Unreserved plus sub-delimiters: real provider keys use these and they
        // are safe inside a segment, so encoding must leave them alone.
        let secret = Secret::new("aZ09-_.~!$&'()*+,;=:@".to_string());

        let url = build_rpc_url(
            "https://drpc.example/:api_key/rpc",
            Some(&cfg),
            Some(&secret),
        )
        .unwrap();
        assert_eq!(url, "https://drpc.example/aZ09-_.~!$&'()*+,;=:@/rpc");
    }

    #[test]
    fn test_build_rpc_url_path_location_percent_in_key_is_escaped() {
        let cfg = api_key(ApiKeyLocation::Path, "api_key", None);
        // A literal `%` must not be left to read as the start of an escape.
        let secret = Secret::new("ab%2Fcd".to_string());

        let url = build_rpc_url(
            "https://drpc.example/:api_key/rpc",
            Some(&cfg),
            Some(&secret),
        )
        .unwrap();
        assert_eq!(url, "https://drpc.example/ab%252Fcd/rpc");
    }

    #[test]
    fn test_build_rpc_url_path_location_shorter_name_does_not_match_longer_placeholder() {
        let cfg = api_key(ApiKeyLocation::Path, "api", None);
        let secret = Secret::new(FAKE_KEY.to_string());

        // `:api` is a prefix of `:api_key`; a naive `str::replace` would emit
        // `<key>_key` here and silently send a malformed key.
        let err = build_rpc_url(
            "https://drpc.example/:api_key/rpc",
            Some(&cfg),
            Some(&secret),
        )
        .expect_err("\":api\" must not match the \":api_key\" placeholder");
        assert!(err.to_string().contains(":api"), "unexpected error: {err}");
    }

    #[test]
    fn test_build_rpc_url_path_location_replaces_only_the_complete_placeholder() {
        let cfg = api_key(ApiKeyLocation::Path, "api", None);
        let secret = Secret::new(FAKE_KEY.to_string());

        // Both spellings are present: only the standalone `:api` is a match.
        let url = build_rpc_url(
            "https://drpc.example/:api_key/:api/rpc",
            Some(&cfg),
            Some(&secret),
        )
        .unwrap();
        assert_eq!(url, format!("https://drpc.example/:api_key/{FAKE_KEY}/rpc"));
    }

    #[test]
    fn test_build_rpc_url_path_location_replaces_every_occurrence() {
        let cfg = api_key(ApiKeyLocation::Path, "api_key", None);
        let secret = Secret::new(FAKE_KEY.to_string());

        let url = build_rpc_url(
            "https://drpc.example/:api_key/rpc/:api_key",
            Some(&cfg),
            Some(&secret),
        )
        .unwrap();
        assert_eq!(
            url,
            format!("https://drpc.example/{FAKE_KEY}/rpc/{FAKE_KEY}")
        );
    }

    #[test]
    fn test_build_rpc_url_path_location_matches_placeholder_at_url_end() {
        let cfg = api_key(ApiKeyLocation::Path, "api_key", None);
        let secret = Secret::new(FAKE_KEY.to_string());

        // End-of-string is a valid terminator, not a truncated placeholder.
        let url =
            build_rpc_url("https://drpc.example/:api_key", Some(&cfg), Some(&secret)).unwrap();
        assert_eq!(url, format!("https://drpc.example/{FAKE_KEY}"));
    }

    #[test]
    fn test_build_rpc_url_path_location_missing_placeholder_errors() {
        let cfg = api_key(ApiKeyLocation::Path, "api_key", None);
        let secret = Secret::new(FAKE_KEY.to_string());

        let err = build_rpc_url("https://drpc.example/rpc", Some(&cfg), Some(&secret))
            .expect_err("a path key with no placeholder in the url must fail");
        assert!(
            err.to_string().contains(":api_key"),
            "unexpected error: {err}"
        );
    }

    // --- api_key: resolve_api_key ---

    #[test]
    fn test_resolve_api_key_prefix_on_query_errors() {
        let cfg = api_key(ApiKeyLocation::Query, "apikey", Some("Bearer"));
        let vars = HashMap::new();

        let err = resolve_api_key(1, "drpc", &cfg, &vars)
            .expect_err("prefix is header-only and must be rejected for query");
        assert!(
            err.to_string().contains("prefix"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_resolve_api_key_prefix_on_path_errors() {
        let cfg = api_key(ApiKeyLocation::Path, "api_key", Some("Bearer"));
        let vars = HashMap::new();

        let err = resolve_api_key(1, "drpc", &cfg, &vars)
            .expect_err("prefix is header-only and must be rejected for path");
        assert!(
            err.to_string().contains("prefix"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_resolve_api_key_prefix_error_takes_precedence_over_missing_variable() {
        // No variable is set at all: if the env lookup ran first, this would
        // report "unset or empty" and send the operator to fix a deployment
        // when the actual bug is the `prefix` on a `query` key in the JSON.
        let cfg = api_key(ApiKeyLocation::Query, "apikey", Some("Bearer"));
        let vars = HashMap::new();

        let err = resolve_api_key(1, "drpc", &cfg, &vars).expect_err("must fail");
        assert!(
            err.to_string().contains("prefix"),
            "expected the prefix/location error, got: {err}"
        );
    }

    #[test]
    fn test_resolve_api_key_missing_variable_errors_with_details() {
        let cfg = api_key(ApiKeyLocation::Header, "Authorization", None);
        let vars = HashMap::new();

        let err = resolve_api_key(1, "drpc", &cfg, &vars).expect_err("must fail");
        let message = err.to_string();
        assert!(message.contains('1'), "missing chain id: {message}");
        assert!(message.contains("drpc"), "missing provider name: {message}");
        assert!(
            message.contains("INTERCHAIN_INDEXER_RPC_API_KEY__1__DRPC"),
            "missing variable name: {message}"
        );
    }

    #[test]
    fn test_resolve_api_key_empty_variable_errors() {
        let cfg = api_key(ApiKeyLocation::Header, "Authorization", None);
        let vars = fixture_vars(&[("INTERCHAIN_INDEXER_RPC_API_KEY__1__DRPC", "")])
            .collect::<HashMap<_, _>>();

        resolve_api_key(1, "drpc", &cfg, &vars).expect_err("an empty variable must be rejected");
    }

    #[test]
    fn test_resolve_api_key_whitespace_only_variable_errors() {
        let cfg = api_key(ApiKeyLocation::Header, "Authorization", None);
        let vars = fixture_vars(&[("INTERCHAIN_INDEXER_RPC_API_KEY__1__DRPC", "   ")])
            .collect::<HashMap<_, _>>();

        resolve_api_key(1, "drpc", &cfg, &vars)
            .expect_err("a whitespace-only variable must be rejected");
    }

    #[test]
    fn test_resolve_api_key_value_env_overrides_derived_name() {
        let mut cfg = api_key(ApiKeyLocation::Header, "Authorization", None);
        cfg.value_env = Some("MY_CUSTOM_VAR".to_string());
        let vars = fixture_vars(&[
            ("INTERCHAIN_INDEXER_RPC_API_KEY__1__DRPC", "derived-value"),
            ("MY_CUSTOM_VAR", "explicit-value"),
        ])
        .collect::<HashMap<_, _>>();

        let secret = resolve_api_key(1, "drpc", &cfg, &vars).unwrap();
        assert_eq!(secret.expose(), "explicit-value");
    }

    // --- api_key: node config wiring ---
    //
    // These go through `build_chain_node_configs`, the seam that decides what a
    // node actually carries. `create_provider_pools_impl` returns an opaque
    // `DynProvider`, so asserting on the credential and the final URL is only
    // possible here.

    /// One chain, one `drpc` provider. `extra` is spliced into the provider
    /// object so a test can attach an `"api_key"` entry.
    fn chain_fixture(url: &str, extra: &str) -> ChainConfig {
        let json = format!(
            r#"[{{"chain_id":1,"name":"Ethereum","icon":"","rpcs":[{{
                "drpc": {{"url":"{url}"{extra}}}
            }}]}}]"#
        );
        serde_json::from_str::<Vec<ChainConfig>>(&json)
            .expect("fixture chain must parse")
            .pop()
            .expect("fixture has one chain")
    }

    /// Already uppercased, matching what `create_provider_pools_impl` hands
    /// down. The normalization itself is covered separately, through that
    /// function.
    fn uppercased_vars(vars: &[(&str, &str)]) -> HashMap<String, String> {
        vars.iter()
            .map(|(k, v)| (k.to_ascii_uppercase(), v.to_string()))
            .collect()
    }

    const DERIVED_VAR: &str = "INTERCHAIN_INDEXER_RPC_API_KEY__1__DRPC";

    #[test]
    fn test_build_chain_node_configs_header_prefix_produces_prefixed_value() {
        let chain = chain_fixture(
            "https://drpc.example/rpc",
            r#","api_key":{"location":"header","param_name":"Authorization","prefix":"Bearer"}"#,
        );
        let vars = uppercased_vars(&[(DERIVED_VAR, FAKE_KEY)]);

        let nodes = build_chain_node_configs(&chain, &vars).unwrap();

        let credential = nodes[0]
            .credential_header
            .as_ref()
            .expect("a header api_key must produce a credential");
        assert_eq!(credential.name, "Authorization");
        assert_eq!(credential.value.expose(), &format!("Bearer {FAKE_KEY}"));
        // A header key must leave the URL alone — that is what makes `header`
        // the location whose secret never reaches URL-rendering code.
        assert_eq!(nodes[0].http_url.expose(), "https://drpc.example/rpc");
    }

    #[test]
    fn test_build_chain_node_configs_header_without_prefix_uses_the_bare_key() {
        let chain = chain_fixture(
            "https://drpc.example/rpc",
            r#","api_key":{"location":"header","param_name":"X-Api-Key"}"#,
        );
        let vars = uppercased_vars(&[(DERIVED_VAR, FAKE_KEY)]);

        let nodes = build_chain_node_configs(&chain, &vars).unwrap();

        let credential = nodes[0].credential_header.as_ref().unwrap();
        assert_eq!(credential.value.expose(), FAKE_KEY);
    }

    #[test]
    fn test_build_chain_node_configs_query_embeds_key_in_url_and_sets_no_header() {
        let chain = chain_fixture(
            "https://drpc.example/rpc",
            r#","api_key":{"location":"query","param_name":"apikey"}"#,
        );
        let vars = uppercased_vars(&[(DERIVED_VAR, FAKE_KEY)]);

        let nodes = build_chain_node_configs(&chain, &vars).unwrap();

        assert!(nodes[0].credential_header.is_none());
        assert_eq!(
            nodes[0].http_url.expose(),
            &format!("https://drpc.example/rpc?apikey={FAKE_KEY}")
        );
    }

    #[test]
    fn test_build_chain_node_configs_path_substitutes_and_sets_no_header() {
        let chain = chain_fixture(
            "https://drpc.example/v1/:api_key/rpc",
            r#","api_key":{"location":"path","param_name":"api_key"}"#,
        );
        let vars = uppercased_vars(&[(DERIVED_VAR, FAKE_KEY)]);

        let nodes = build_chain_node_configs(&chain, &vars).unwrap();

        assert!(nodes[0].credential_header.is_none());
        assert_eq!(
            nodes[0].http_url.expose(),
            &format!("https://drpc.example/v1/{FAKE_KEY}/rpc")
        );
    }

    #[test]
    fn test_build_chain_node_configs_without_api_key_changes_nothing() {
        let chain = chain_fixture("https://drpc.example/rpc", "");

        let nodes = build_chain_node_configs(&chain, &HashMap::new()).unwrap();

        assert!(nodes[0].credential_header.is_none());
        assert_eq!(nodes[0].http_url.expose(), "https://drpc.example/rpc");
    }

    #[test]
    fn test_build_chain_node_configs_missing_secret_fails_with_context() {
        let chain = chain_fixture(
            "https://drpc.example/rpc",
            r#","api_key":{"location":"header","param_name":"X-Api-Key"}"#,
        );

        // `.err().expect(...)` rather than `expect_err`: the latter needs
        // `Debug` on the success type, and `NodeConfig` deliberately has none.
        let err = build_chain_node_configs(&chain, &HashMap::new())
            .err()
            .expect("a declared api_key with no secret must fail");

        let rendered = format!("{err:#}");
        assert!(rendered.contains("drpc"), "unexpected error: {rendered}");
        assert!(
            rendered.contains(DERIVED_VAR),
            "unexpected error: {rendered}"
        );
    }

    #[test]
    fn test_chain_declares_api_key_ignores_disabled_providers() {
        // A disabled provider's leftover api_key must not make an unrelated
        // pool failure fatal: it is not in the pool at all.
        let chain = chain_fixture(
            "https://drpc.example/rpc",
            r#","enabled":false,"api_key":{"location":"header","param_name":"X-Api-Key"}"#,
        );

        assert!(!chain_declares_api_key(&chain));
    }

    #[tokio::test]
    async fn test_create_provider_pools_impl_normalizes_env_var_case() {
        // The operator's variable arrives lowercase; the derived name is
        // uppercase. The pool must still build, which is what pins the two
        // sides of the lookup to the same normalization.
        let chain = chain_fixture(
            "https://drpc.example/rpc",
            r#","api_key":{"location":"header","param_name":"X-Api-Key"}"#,
        );
        let lowercased = DERIVED_VAR.to_lowercase();
        // Collected up front: `fixture_vars` borrows its input, and the
        // iterator would otherwise outlive the temporary array across the
        // `.await` — the same reason `create_provider_pools_from_chains`
        // collects `std::env::vars()` before handing it down.
        let vars: Vec<(String, String)> = fixture_vars(&[(&lowercased, FAKE_KEY)]).collect();

        let pools = create_provider_pools_impl(vec![chain], vars.into_iter())
            .await
            .unwrap();

        assert!(pools.contains_key(&1));
    }

    #[tokio::test]
    async fn test_create_provider_pools_impl_keyed_chain_fails_on_invalid_header_name() {
        // A space is not legal in a header name. For a chain that declares an
        // api_key this must abort startup rather than warn and skip: skipping
        // would leave the service healthy with a chain that has no providers,
        // which only surfaces later as `no provider configured for chain_id`.
        let chain = chain_fixture(
            "https://drpc.example/rpc",
            r#","api_key":{"location":"header","param_name":"X Api Key"}"#,
        );
        let vars: Vec<(String, String)> = fixture_vars(&[(DERIVED_VAR, FAKE_KEY)]).collect();

        let err = create_provider_pools_impl(vec![chain], vars.into_iter())
            .await
            .expect_err("an invalid header name on a keyed chain must fail startup");

        let rendered = format!("{err:#}");
        assert!(
            rendered.contains("declares an api_key"),
            "unexpected error: {rendered}"
        );
        assert!(
            !rendered.contains(FAKE_KEY),
            "the failure must not render the key: {rendered}"
        );
    }

    // --- api_key: deserialization ---

    #[test]
    fn test_api_key_config_rejects_unknown_field() {
        let err = serde_json::from_str::<ApiKeyConfig>(
            r#"{"location":"header","param_name":"Authorization","bogus":true}"#,
        )
        .expect_err("unknown field must be rejected");
        assert!(err.to_string().contains("bogus"), "unexpected error: {err}");
    }

    #[test]
    fn test_api_key_config_rejects_invalid_location() {
        serde_json::from_str::<ApiKeyConfig>(
            r#"{"location":"cookie","param_name":"Authorization"}"#,
        )
        .expect_err("an unknown location must be rejected");
    }

    #[test]
    fn test_api_key_config_rejects_old_name_field() {
        // The pre-feature `ApiKeyConfig` used `name`; it was renamed to
        // `param_name` and must not silently accept the old spelling.
        serde_json::from_str::<ApiKeyConfig>(r#"{"location":"header","name":"Authorization"}"#)
            .expect_err("the old `name` field must be rejected");
    }

    fn write_temp_json(content: &str) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    fn fixture_vars(vars: &[(&str, &str)]) -> impl Iterator<Item = (String, String)> {
        vars.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>()
            .into_iter()
    }

    const CHAINS_FILE: &str = r#"
    [
        {
            "chain_id": 1,
            "name": "Ethereum",
            "icon": "https://icon.example/eth.svg",
            "rpcs": [ { "drpc": { "url": "https://eth.drpc.org" } } ]
        }
    ]
    "#;

    const BRIDGES_FILE: &str = r#"
    [
        {
            "bridge_id": 1,
            "name": "AMB",
            "type": "amb",
            "indexer_type": "amb",
            "enabled": true,
            "api_url": "https://api.example",
            "ui_url": null,
            "docs_url": null,
            "contracts": [
                {
                    "chain_id": 100,
                    "address": "0xf6A78083ca3e2a662D6dd1703c939c8aCE2e268d",
                    "version": 6,
                    "started_at_block": 10
                }
            ]
        }
    ]
    "#;

    #[test]
    fn test_load_chains_impl_without_override_vars_matches_file() {
        let file = write_temp_json(CHAINS_FILE);
        let chains = load_chains_impl(file.path(), fixture_vars(&[])).unwrap();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].chain_id, 1);
        assert_eq!(chains[0].name, "Ethereum");
    }

    #[test]
    fn test_load_chains_impl_new_chain_field_by_field_parses_typed() {
        let file = write_temp_json(CHAINS_FILE);
        let chains = load_chains_impl(
            file.path(),
            fixture_vars(&[
                ("INTERCHAIN_INDEXER_CHAINS__137__NAME", "Polygon"),
                (
                    "INTERCHAIN_INDEXER_CHAINS__137__ICON",
                    "https://icon.example/poly.svg",
                ),
                (
                    "INTERCHAIN_INDEXER_CHAINS__137__RPCS__MYNODE__URL",
                    "https://my.node",
                ),
            ]),
        )
        .unwrap();

        assert_eq!(chains.len(), 2);
        let polygon = &chains[1];
        assert_eq!(polygon.chain_id, 137);
        assert_eq!(polygon.name, "Polygon");
        assert_eq!(polygon.rpcs[0]["mynode"].url, "https://my.node");
    }

    #[test]
    fn test_load_bridges_impl_null_api_url_parses_as_none() {
        let file = write_temp_json(BRIDGES_FILE);
        let bridges = load_bridges_impl(
            file.path(),
            fixture_vars(&[("INTERCHAIN_INDEXER_BRIDGES__1__API_URL", "null")]),
        )
        .unwrap();

        assert_eq!(bridges[0].api_url, None);
    }

    #[test]
    fn test_load_bridges_impl_rejects_started_at_block_zero() {
        let file = write_temp_json(BRIDGES_FILE);
        let err = load_bridges_impl(
            file.path(),
            fixture_vars(&[(
                "INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0xF6A78083CA3E2A662D6DD1703C939C8ACE2E268D__6__STARTED_AT_BLOCK",
                "0",
            )]),
        )
        .unwrap_err();

        let message = format!("{err:#}");
        assert!(
            message.contains("started_at_block = 0"),
            "unexpected: {message}"
        );
    }

    #[test]
    fn test_load_bridges_impl_rejects_negative_bridge_id() {
        // A negative bridge id has no representation in the public API, where
        // bridge ids are `uint32`. The env-override key is itself the bridge
        // id, so the invalid value can only come from the file.
        const NEGATIVE_BRIDGE_ID_FILE: &str = r#"
        [
            {
                "bridge_id": -1,
                "name": "AMB",
                "type": "amb",
                "indexer_type": "amb",
                "enabled": true,
                "api_url": null,
                "ui_url": null,
                "docs_url": null,
                "contracts": [
                    {
                        "chain_id": 100,
                        "address": "0xf6A78083ca3e2a662D6dd1703c939c8aCE2e268d",
                        "version": 6,
                        "started_at_block": 10
                    }
                ]
            }
        ]
        "#;

        let file = write_temp_json(NEGATIVE_BRIDGE_ID_FILE);
        let err = load_bridges_impl(file.path(), fixture_vars(&[])).unwrap_err();

        let message = format!("{err:#}");
        assert!(
            message.contains("negative bridge_id"),
            "unexpected: {message}"
        );
    }

    #[test]
    fn test_load_bridges_impl_new_bridge_fragment_parses_typed() {
        let file = write_temp_json(BRIDGES_FILE);
        let bridges = load_bridges_impl(
            file.path(),
            fixture_vars(&[(
                "INTERCHAIN_INDEXER_BRIDGES__2",
                r#"{
                    "name": "Avalanche ICTT",
                    "type": "avalanche_native",
                    "indexer_type": "icm_ictt",
                    "enabled": false,
                    "api_url": null,
                    "ui_url": null,
                    "docs_url": null,
                    "contracts": [
                        {
                            "chain_id": 43114,
                            "address": "0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf",
                            "version": 1,
                            "started_at_block": 42526120
                        }
                    ]
                }"#,
            )]),
        )
        .unwrap();

        assert_eq!(bridges.len(), 2);
        let new_bridge = &bridges[1];
        assert_eq!(new_bridge.bridge_id, 2);
        assert_eq!(new_bridge.indexer_type, IndexerType::IcmIctt);
        assert!(!new_bridge.enabled);
        assert_eq!(new_bridge.contracts.len(), 1);
        assert_eq!(new_bridge.contracts[0].chain_id, 43114);
    }

    #[test]
    fn test_load_chains_impl_env_built_chain_missing_name_errors() {
        let file = write_temp_json(CHAINS_FILE);
        let err = load_chains_impl(
            file.path(),
            fixture_vars(&[(
                "INTERCHAIN_INDEXER_CHAINS__137__ICON",
                "https://icon.example/poly.svg",
            )]),
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("name"), "unexpected: {err:#}");
    }

    #[test]
    fn test_load_chains_impl_unknown_field_in_env_path_errors() {
        let file = write_temp_json(CHAINS_FILE);
        let err = load_chains_impl(
            file.path(),
            fixture_vars(&[("INTERCHAIN_INDEXER_CHAINS__1__NAME_TYPO", "X")]),
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("name_typo"),
            "unexpected: {err:#}"
        );
    }

    #[test]
    fn test_load_bridges_impl_inline_json_abi_from_env_parses() {
        let file = write_temp_json(BRIDGES_FILE);
        let bridges = load_bridges_impl(
            file.path(),
            fixture_vars(&[(
                "INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0xF6A78083CA3E2A662D6DD1703C939C8ACE2E268D__6__ABI",
                r#"[{"name":"RelayedMessage","type":"event"}]"#,
            )]),
        )
        .unwrap();

        assert_eq!(
            bridges[0].contracts[0].abi,
            Some(r#"[{"name":"RelayedMessage","type":"event"}]"#.to_string())
        );
    }

    /// Collect all `.json` files under a directory, recursively.
    fn collect_json_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect_json_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "json") {
                out.push(path);
            }
        }
    }

    #[test]
    fn test_all_repo_config_files_parse_through_strict_structs() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut files = Vec::new();
        collect_json_files(&repo_root.join("config"), &mut files);
        collect_json_files(&repo_root.join("docker/config"), &mut files);
        assert!(!files.is_empty(), "no config JSON files found");

        for path in files {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let content = std::fs::read_to_string(&path).unwrap();
            if name.starts_with("chains") {
                serde_json::from_str::<Vec<ChainConfig>>(&content)
                    .unwrap_or_else(|e| panic!("failed to parse {path:?} as chains config: {e}"));
            } else if name.starts_with("bridges") {
                let bridges: Vec<BridgeConfig> = serde_json::from_str(&content)
                    .unwrap_or_else(|e| panic!("failed to parse {path:?} as bridges config: {e}"));
                // Structural parsing is not enough: `started_at_block = 0`
                // and a negative `bridge_id` are both syntactically valid and
                // semantically rejected at load time, so a committed config
                // carrying either must fail here too. Keep this in step with
                // `load_bridges_impl`'s validation.
                validate_started_at_blocks(&bridges)
                    .unwrap_or_else(|e| panic!("invalid bridges config {path:?}: {e}"));
                validate_bridge_ids(&bridges)
                    .unwrap_or_else(|e| panic!("invalid bridges config {path:?}: {e}"));
            } else {
                panic!("unexpected config file {path:?}: neither chains* nor bridges*");
            }
        }
    }

    /// No committed config file may declare an `api_key`. A declared credential
    /// makes its secret variable mandatory — startup fails without it — so
    /// putting one in a shared file blocks every deployment that does not hold
    /// that key. The credential's shape belongs in the environment next to its
    /// value (see `config/full-mainnet/ENVs.md`), which is what the next test
    /// exercises.
    #[test]
    fn test_no_repo_config_file_declares_an_api_key() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut files = Vec::new();
        collect_json_files(&repo_root.join("config"), &mut files);
        collect_json_files(&repo_root.join("docker/config"), &mut files);

        for path in files {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if !name.starts_with("chains") {
                continue;
            }
            let content = std::fs::read_to_string(&path).unwrap();
            let chains: Vec<ChainConfig> = serde_json::from_str(&content).unwrap();
            for chain in chains {
                for provider in chain.rpcs.iter().flatten() {
                    let (provider_name, rpc_config) = provider;
                    assert!(
                        rpc_config.api_key.is_none(),
                        "{path:?}: chain {} provider \"{provider_name}\" declares an api_key, \
                         which makes {} mandatory for everyone using this file — declare it \
                         through the environment instead",
                        chain.chain_id,
                        derived_api_key_env_var(chain.chain_id, provider_name),
                    );
                }
            }
        }
    }

    /// The counterpart: a provider that carries no `api_key` in the file can be
    /// credentialed entirely from the environment — the `api_key` object is
    /// created by the env merge on demand, and its value comes from the derived
    /// secret variable. This is the arrangement `config/avalanche` documents for
    /// Glacier, pinned against the real file so a config edit cannot break it
    /// silently.
    #[test]
    fn test_env_declared_api_key_credentials_a_provider_from_a_committed_config() {
        let chains_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../config/avalanche/chains.json");

        let bare = load_chains_impl(&chains_path, fixture_vars(&[])).unwrap();
        let glacier = |chains: &[ChainConfig]| {
            chains
                .iter()
                .find(|c| c.chain_id == 8021)
                .expect("chain 8021 must be present")
                .clone()
        };
        assert!(
            glacier(&bare).rpcs[0]["glacier"].api_key.is_none(),
            "the file itself must not declare the credential"
        );

        let keyed = load_chains_impl(
            &chains_path,
            fixture_vars(&[
                (
                    "INTERCHAIN_INDEXER_CHAINS__8021__RPCS__GLACIER__API_KEY__LOCATION",
                    "header",
                ),
                (
                    "INTERCHAIN_INDEXER_CHAINS__8021__RPCS__GLACIER__API_KEY__PARAM_NAME",
                    "x-glacier-api-key",
                ),
            ]),
        )
        .unwrap();
        let chain = glacier(&keyed);
        let api_key = chain.rpcs[0]["glacier"]
            .api_key
            .as_ref()
            .expect("the env override must create the api_key object");
        assert_eq!(api_key.location, ApiKeyLocation::Header);
        assert_eq!(api_key.param_name, "x-glacier-api-key");

        let vars = uppercased_vars(&[("INTERCHAIN_INDEXER_RPC_API_KEY__8021__GLACIER", FAKE_KEY)]);
        let nodes = build_chain_node_configs(&chain, &vars).unwrap();
        let credential = nodes[0]
            .credential_header
            .as_ref()
            .expect("a header api_key must produce a credential");
        assert_eq!(credential.name, "x-glacier-api-key");
        assert_eq!(credential.value.expose(), FAKE_KEY);
        assert_eq!(
            nodes[0].http_url.expose(),
            "https://glacier-api.avax.network/v1/ext/bc/8021/rpc"
        );
    }

    /// The server test harness boots `run()` against
    /// `tests/fixtures/chains-offline.json` instead of the deployment config,
    /// so that no test starts a live indexer against mainnet. That substitution
    /// is only sound while the fixture declares the same chains — a chain added
    /// to `config/omnibridge/chains.json` and not to the fixture would silently
    /// stop being exercised, and a test asserting "this pair is absent from
    /// config" would start passing for the wrong reason.
    #[test]
    fn test_offline_chains_fixture_matches_the_omnibridge_chains_config() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |path: &Path| -> Vec<ChainConfig> {
            let content = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
            serde_json::from_str(&content)
                .unwrap_or_else(|e| panic!("failed to parse {path:?} as chains config: {e}"))
        };

        let deployed = read(&manifest_dir.join("../config/omnibridge/chains.json"));
        let fixture = read(&manifest_dir.join("tests/fixtures/chains-offline.json"));

        let ids = |chains: &[ChainConfig]| {
            let mut ids: Vec<i64> = chains.iter().map(|chain| chain.chain_id).collect();
            ids.sort_unstable();
            ids
        };
        assert_eq!(
            ids(&fixture),
            ids(&deployed),
            "tests/fixtures/chains-offline.json must declare the same chains as \
             config/omnibridge/chains.json"
        );

        // The point of the fixture is that nothing it names is reachable.
        for chain in &fixture {
            for rpc_map in &chain.rpcs {
                for (name, rpc) in rpc_map {
                    assert!(
                        rpc.url.starts_with("http://127.0.0.1:"),
                        "fixture RPC {name} for chain {} must be a dead loopback endpoint, got {}",
                        chain.chain_id,
                        rpc.url,
                    );
                }
            }
        }
    }
}
