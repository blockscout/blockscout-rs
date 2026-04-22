# Configuration Loading and Validation

## Scope

How `Settings`, JSON config files (`chains.json`, `bridges.json`), environment
variable overrides, `deny_unknown_fields`, DB seeding, and cross-field
validation interact from process startup to indexer construction.

Out of scope: runtime indexer behavior after construction, bridge filtering
semantics (see `avalanche-bridge-filtering.md`), stats projection, API serving.

## Short Answer

Configuration flows through two disconnected channels that converge in
`server.rs::run()`. Runtime settings use the `config` crate with env-var
layering (`INTERCHAIN_INDEXER__*`). Chain/bridge topology uses raw JSON file
reads with no env-override capability. Both channels feed serde deserialization,
but only the settings path benefits from `deny_unknown_fields` across the full
struct tree. After deserialization, config is seeded into the database via
upserts; semantic validation happens late, at indexer construction time rather
than at load time.

## Why This Matters

- A typo in an env var causes a hard startup failure (good). A typo in certain
  JSON config structs is silently ignored (bad).
- JSON configs cannot be patched via env vars, making development iteration
  slower than it needs to be. The sibling `stats` service has a reusable pattern
  for this.
- Semantic validation (e.g., `home_chain_id` must be a configured chain) happens
  after DB seeding, so invalid config can be partially committed before the error
  surfaces.
- Understanding the two-channel split is essential before modifying config
  loading or adding new config fields.

## Source-of-Truth Files

| File | Role |
|------|------|
| `interchain-indexer-server/src/settings.rs` | Top-level `Settings` struct, `ConfigSettings` impl |
| `interchain-indexer-server/src/config.rs` | JSON config models, loaders, `From` impls for DB seeding |
| `interchain-indexer-server/src/server.rs` | Startup orchestration (`run()`) |
| `interchain-indexer-server/src/indexers.rs` | Bridge-to-indexer wiring, late validation |
| `interchain-indexer-server/src/bin/interchain-indexer-server.rs` | Process entrypoint |
| `interchain-indexer-server/src/bin/check-envs.rs` | Env var documentation generator |
| `interchain-indexer-logic/src/settings.rs` | `MessageBufferSettings` |
| `interchain-indexer-logic/src/indexer/avalanche/settings.rs` | `AvalancheIndexerSettings` |
| `interchain-indexer-logic/src/chain_info/settings.rs` | `ChainInfoServiceSettings` |
| `interchain-indexer-logic/src/token_info/settings.rs` | `TokenInfoServiceSettings` |
| `interchain-indexer-logic/src/database.rs` | `upsert_chains`, `upsert_bridges`, `upsert_bridge_contracts` |
| `libs/blockscout-service-launcher/src/launcher/settings.rs` | `ConfigSettings` trait with `build()` |
| `config/avalanche/chains.json` | Avalanche chain topology |
| `config/avalanche/bridges.json` | Avalanche bridge topology |

## Key Types / Tables / Contracts

### Settings Struct Tree

```
Settings                              (deny_unknown_fields)
  ├─ chains_config: PathBuf
  ├─ bridges_config: PathBuf
  ├─ token_info: TokenInfoServiceSettings     (deny_unknown_fields)
  ├─ chain_info: ChainInfoServiceSettings     (deny_unknown_fields)
  ├─ buffer_settings: MessageBufferSettings   (deny_unknown_fields)
  ├─ example_indexer: ExampleIndexerSettings   (deny_unknown_fields)
  ├─ avalanche_indexer: AvalancheIndexerSettings (deny_unknown_fields)
  ├─ server: ServerSettings                   (deny_unknown_fields)
  ├─ metrics: MetricsSettings                 (deny_unknown_fields)
  ├─ tracing: TracingSettings
  ├─ jaeger: JaegerSettings
  ├─ database: DatabaseSettings
  ├─ api: ApiSettings                         (deny_unknown_fields)
  └─ stats: StatsSettings                     (deny_unknown_fields)
       ├─ backfill_on_start: bool
       ├─ chains_recalculation_period_secs: u64
       └─ include_zero_chains: bool
```

### JSON Config Structs

```
BridgeConfig          (deny_unknown_fields)
  ├─ bridge_id, name, bridge_type, indexer_type, enabled
  ├─ process_unknown_chains, home_chain_id
  └─ contracts: Vec<BridgeContractConfig>     (NO deny_unknown_fields)

ChainConfig           (NO deny_unknown_fields)
  ├─ chain_id, name, icon
  ├─ explorer: ExplorerConfig                 (NO deny_unknown_fields)
  ├─ pool_config: PoolConfig
  └─ rpcs: Vec<HashMap<String, RpcProviderConfig>>  (NO deny_unknown_fields)
```

### Database Tables Seeded from Config

- `chains` — upserted from `ChainConfig` via `From<ChainConfig> for chains::ActiveModel`
- `bridges` — upserted from `BridgeConfig` with name-conflict guard
- `bridge_contracts` — upserted from `BridgeContractConfig` via `to_active_model(bridge_id)`

## Step-by-Step Flow

### 1. Process Entry

`main()` in `bin/interchain-indexer-server.rs` calls `Settings::build()`.

### 2. Settings Assembly (`ConfigSettings::build()`)

The `config` crate assembles settings from two layers:

1. **File layer** (optional): if `INTERCHAIN_INDEXER__CONFIG` env var is set,
   loads that file path as TOML/YAML/JSON
2. **Env layer**: `config::Environment::with_prefix("INTERCHAIN_INDEXER").separator("__")`
   — keys like `INTERCHAIN_INDEXER__BUFFER_SETTINGS__HOT_TTL` map to nested
   struct fields

Env vars override file values. Then `try_deserialize()` feeds the merged config
through serde, where `deny_unknown_fields` catches unknown keys at every
annotated struct level.

The default no-op `validate()` is called but never overridden — no cross-field
validation occurs at this stage.

### 3. JSON Config Loading

`load_chains_from_file` and `load_bridges_from_file` in `config.rs` use
`std::fs::read_to_string` + `serde_json::from_str`. This path is completely
separate from the `config` crate — no env-var override is possible for
individual JSON fields. You can only change the file path via
`INTERCHAIN_INDEXER__CHAINS_CONFIG` / `INTERCHAIN_INDEXER__BRIDGES_CONFIG`.

Custom deserializers:
- `deserialize_bridge_type` — maps JSON string to `BridgeType` via SeaORM `ActiveEnum`
- `deserialize_address` — parses hex string (with or without `0x`) to `Vec<u8>`

### 4. Provider Pool Construction

`create_provider_pools_from_chains` iterates chain configs, filters enabled RPC
providers, builds `NodeConfig` structs, and creates layered Alloy HTTP providers.
Detects duplicate `chain_id` entries and bails. Chains with no enabled RPCs or
negative IDs are skipped with warnings.

### 5. Service Construction

`TokenInfoService`, `StatsService`, `ChainInfoService` are created from the
assembled settings and provider pools. Optional stats backfill runs before
bridge loading.

### 6. DB Seeding

Config is converted to SeaORM `ActiveModel`s and upserted:

- **`upsert_chains`**: `ON CONFLICT (id)` updates name, icon, explorer,
  custom_routes, sets `updated_at` to current timestamp
- **`upsert_bridges`**: first queries existing bridges and **fails if a bridge
  ID exists with a different name**; then inserts/updates
- **`upsert_bridge_contracts`**: `ON CONFLICT (bridge_id, chain_id, address,
  version)` updates ABI and `started_at_block`

Fields not persisted (runtime-only):
- Bridge: `indexer_type`, `process_unknown_chains`, `home_chain_id`
- Chain: `rpcs`, `pool_config`

### 7. Indexer Construction and Late Validation

`spawn_configured_indexers` loops through enabled bridges, matches on
`bridge_type`, and constructs indexer instances. Semantic validation happens
here:

- `AvalancheIndexer::new()` validates `home_chain_id` is in the configured
  chains list (`mod.rs:136-151`). Failure is caught per-bridge — the indexer is
  skipped, not the whole process.
- `build_avalanche_chain_configs` validates contract addresses are 20 bytes and
  that providers exist for referenced chains.

### 8. Server Launch

HTTP and gRPC routers are built from API services. Stats recalculation worker is
spawned. `launcher::launch()` runs the servers until shutdown.

## Invariants

1. **Env vars override file settings** — the `config` crate env layer always
   wins over the file layer for `Settings`
2. **JSON configs have no env override** — they are file-only
3. **`deny_unknown_fields` is pervasive on Settings** — every settings struct in
   the tree has it; a typo in any env var mapping to these structs causes hard
   startup failure
4. **`deny_unknown_fields` is partial on JSON structs** — `BridgeConfig` has it;
   `ChainConfig`, `ExplorerConfig`, `BridgeContractConfig`, `RpcProviderConfig`
   do not
5. **DB seeding is unconditional** — every startup overwrites chain/bridge/
   contract rows; config is the source of truth, not the database
6. **Bridge name-conflict guard** — `upsert_bridges` fails if an existing bridge
   ID has a different name (prevents accidental ID reuse)
7. **Round-trip lossy** — `From<Model> for Config` impls fill runtime-only
   fields with defaults; DB-loaded config is incomplete for indexer construction

## Failure Modes / Observability

| Failure | When | Severity | Observable |
|---------|------|----------|------------|
| Unknown field in Settings env var | `Settings::build()` | Fatal — process won't start | Startup panic with serde error |
| Unknown field in `BridgeConfig` JSON | `load_bridges_from_file()` | Fatal — process won't start | Startup panic with serde error |
| Unknown field in `ChainConfig` JSON | `load_chains_from_file()` | **Silent** — ignored | No signal |
| Missing JSON config file | `load_*_from_file()` | Fatal | anyhow context with file path |
| Invalid address format in JSON | `deserialize_address` | Fatal (whole file fails) | serde error |
| Duplicate `chain_id` in chains config | `create_provider_pools_from_chains` | Fatal | anyhow bail |
| Bridge ID reused with different name | `upsert_bridges` | Fatal | Error log + anyhow |
| `home_chain_id` not in configured chains | `AvalancheIndexer::new()` | Per-bridge skip | Error log, indexer not started |
| No enabled RPCs for a chain | `create_provider_pools_from_chains` | Warning, chain skipped | Warn log |
| API key config present | `build_rpc_url` | Fatal (unimplemented) | anyhow bail |

## Edge Cases / Gotchas

1. **`deny_unknown_fields` coverage gap**: `ChainConfig` and its children lack
   `deny_unknown_fields`. A typo like `"chain_Id"` instead of `"chain_id"` in
   `chains.json` will silently produce a deserialization error or default,
   depending on whether the field has a `#[serde(default)]`. Since `chain_id`
   has no default, this would actually fail — but optional fields like
   `custom_tx_route` in `ExplorerConfig` would be silently lost.

2. **DB seeding happens before validation**: if `home_chain_id` is invalid, the
   bridge is already upserted into the database before `AvalancheIndexer::new()`
   rejects it. The bridge row persists with `enabled = true` even though no
   indexer is running for it.

3. **No centralized cross-field validation**: the `ConfigSettings::validate()`
   hook is never overridden. Semantic checks are scattered across constructors.

4. **JSON configs can't be patched via env vars**: unlike the `stats` service
   which uses `config` crate `File` source + env overlay + custom merge
   functions, interchain-indexer uses raw file I/O. During development, any
   topology change requires editing JSON files directly.

5. **Two separate provider pools**: `create_provider_pools_from_chains` is
   called twice in `run()` — once for `TokenInfoService` and once for indexers.
   This means RPC connections are duplicated, not shared.

## Stats Service Pattern (Cross-Reference)

The `stats` service at `../stats/stats-server/src/config/read/mod.rs` solves the
JSON-env patching problem with `read_json_override_from_env_config`:

1. Loads JSON via `config::File` source (not raw `fs::read_to_string`)
2. Loads env vars with a dedicated prefix (e.g., `STATS_CHARTS__`)
3. Deserializes both into separate typed structs (JSON struct + env-override struct)
4. Applies a custom merge function where env values take precedence

Each config domain gets its own prefix (`STATS_CHARTS__`, `STATS_LAYOUT__`,
`STATS_UPDATE_GROUPS__`), separate from the main `STATS__` settings prefix.

**Applicability to interchain-indexer:**
- Leaf fields like `enabled`, `process_unknown_chains`, `home_chain_id`,
  `started_at_block`, explorer URLs, and RPC tuning knobs map well to env
  overrides
- Deeply nested array structures (`contracts`, `rpcs`) are harder — stats uses
  named keys with order fields for arrays, but bridge contracts have compound
  keys
- Adoption requires writing parallel JSON/env struct pairs and merge functions
  per config domain — meaningful but bounded effort

## Change Triggers

Update this note when:
- `deny_unknown_fields` is added or removed from any config struct
- JSON loading switches from raw file I/O to `config` crate `File` source
- A `ConfigSettings::validate()` override is added
- New config files are introduced or existing ones restructured
- DB seeding logic changes (upsert conflict keys, name-conflict guard)
- The stats-style env-patching pattern is adopted

## Open Questions

1. Should `ChainConfig`, `ExplorerConfig`, `BridgeContractConfig`, and
   `RpcProviderConfig` gain `deny_unknown_fields` for parity with `BridgeConfig`
   and `Settings`?
2. Should semantic validation (like `home_chain_id` check) move earlier — either
   into `ConfigSettings::validate()` or immediately after JSON loading — so it
   runs before DB seeding?
3. Is the stats-style `read_json_override_from_env_config` pattern worth
   adopting for `chains.json` / `bridges.json` to improve development ergonomics?
   If so, which fields should be patchable first?
