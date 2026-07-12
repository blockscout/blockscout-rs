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
layering (`INTERCHAIN_INDEXER__*`). Chain/bridge topology is read from JSON
files and then patched by a dedicated env-override layer
(`INTERCHAIN_INDEXER_CHAINS*` / `INTERCHAIN_INDEXER_BRIDGES*`, single
underscore after the service name) implemented in
`interchain-indexer-server/src/env_merge.rs`: vars are parsed as JSON fragments
and deep-merged into the file's `serde_json::Value` before typed
deserialization, with arrays addressed element-wise by DB-aligned id keys.
Both channels feed serde deserialization with `deny_unknown_fields` across the
full struct tree. After deserialization, config is seeded into the database via
upserts; semantic validation happens late, at indexer construction time rather
than at load time.

## Why This Matters

- A typo in an env var or in a JSON config file causes a hard startup failure —
  all JSON config structs now carry `deny_unknown_fields`.
- JSON configs can be patched (and extended with whole new entries) via env
  vars — see the README section "Overriding chains.json / bridges.json via
  environment" for the operator surface.
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
| `interchain-indexer-server/src/env_merge.rs` | Generic env-override deep-merge into config JSON (`ArrayRules`, `apply_env_overrides`) |
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
  └─ contracts: Vec<BridgeContractConfig>     (deny_unknown_fields)
       └─ abi: dual-form deserializer (JSON string or inline JSON)

ChainConfig           (deny_unknown_fields)
  ├─ chain_id, name, icon
  ├─ explorer: ExplorerConfig                 (deny_unknown_fields)
  ├─ pool_config: PoolConfig                  (deny_unknown_fields)
  └─ rpcs: Vec<HashMap<String, RpcProviderConfig>>  (deny_unknown_fields)
       └─ api_key: ApiKeyConfig               (deny_unknown_fields)
```

A repo-level test (`test_all_repo_config_files_parse_through_strict_structs`)
deserializes every `config/**/*.json` and `docker/config/*.json` through the
strict structs.

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

`load_chains_from_file` and `load_bridges_from_file` in `config.rs` read the
file into a `serde_json::Value`, apply the env-override layer
(`env_merge::apply_env_overrides` with the `INTERCHAIN_INDEXER_CHAINS` /
`INTERCHAIN_INDEXER_BRIDGES` prefixes and the chains/bridges `ArrayRules`),
log every applied override at info level (var name + JSON path, no values —
RPC URLs may embed API keys), and only then run the typed serde parse. The
file path itself is still set via `INTERCHAIN_INDEXER__CHAINS_CONFIG` /
`INTERCHAIN_INDEXER__BRIDGES_CONFIG`. This path remains separate from the
`config` crate: the env collection is hand-rolled to preserve key casing.
Testable impls (`load_chains_impl` / `load_bridges_impl`) take an injectable
vars iterator so tests never mutate process env.

Custom deserializers:
- `deserialize_bridge_type` — maps JSON string to `BridgeType` via SeaORM `ActiveEnum`
- `deserialize_address` — parses hex string (with or without `0x`) to `Vec<u8>`
- `deserialize_abi` — accepts a JSON string (file form) or inline JSON
  (env-fragment form), normalizing to `Option<String>`

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
2. **JSON configs have their own env-override layer** — `INTERCHAIN_INDEXER_CHAINS*`
   / `INTERCHAIN_INDEXER_BRIDGES*` (single underscore — cannot collide with the
   `INTERCHAIN_INDEXER__*` settings prefix, whose prefix separator is `__`);
   env always wins over the file, merge keys equal DB uniqueness keys
3. **`deny_unknown_fields` is pervasive on Settings** — every settings struct in
   the tree has it; a typo in any env var mapping to these structs causes hard
   startup failure
4. **`deny_unknown_fields` is complete on JSON structs** — `BridgeConfig`,
   `BridgeContractConfig`, `ChainConfig`, `ExplorerConfig`, `RpcProviderConfig`,
   `ApiKeyConfig`, `PoolConfig` all have it; stray keys in files or env paths
   fail startup
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
| Unknown field in any chains/bridges JSON struct (file or env path) | `load_*_from_file()` | Fatal — process won't start | Startup panic with serde error |
| Invalid env-override path/value (ambiguous duplicate path, multi-match key, null entry, malformed root patch) | `env_merge::apply_env_overrides()` | Fatal | anyhow error naming the env var |
| Env-built entry missing a required field | `load_*_from_file()` typed parse | Fatal | serde error (references the merged JSON; per-override info logs printed beforehand are the breadcrumb) |
| Missing JSON config file | `load_*_from_file()` | Fatal | anyhow context with file path |
| Invalid address format in JSON | `deserialize_address` | Fatal (whole file fails) | serde error |
| Duplicate `chain_id` in chains config | `create_provider_pools_from_chains` | Fatal | anyhow bail |
| Bridge ID reused with different name | `upsert_bridges` | Fatal | Error log + anyhow |
| `home_chain_id` not in configured chains | `AvalancheIndexer::new()` | Per-bridge skip | Error log, indexer not started |
| No enabled RPCs for a chain | `create_provider_pools_from_chains` | Warning, chain skipped | Warn log |
| API key config present | `build_rpc_url` | Fatal (unimplemented) | anyhow bail |

## Edge Cases / Gotchas

1. **`deny_unknown_fields` is now complete** (closed 2026-07): all chains/
   bridges JSON structs reject unknown keys, so a typo like `"chain_Id"` in
   `chains.json` — or a typo'd field segment in an override env var — fails
   startup. Flip side: external deployments with stray keys in their JSONs now
   fail at startup too (behavior change; the repo's own files are guarded by
   the all-files parse test).

2. **DB seeding happens before validation**: if `home_chain_id` is invalid, the
   bridge is already upserted into the database before `AvalancheIndexer::new()`
   rejects it. The bridge row persists with `enabled = true` even though no
   indexer is running for it.

3. **No centralized cross-field validation**: the `ConfigSettings::validate()`
   hook is never overridden. Semantic checks are scattered across constructors.

4. **JSON configs are patched via env vars** (adopted 2026-07): the
   `env_merge` layer supports field overrides, whole-entry fragments, root
   bulk patches, and building brand-new entries from scratch. Semantics worth
   remembering: `null` replaces a value but never removes the key; nested
   whole-array values replace wholesale; a literal string that is valid JSON
   needs JSON-string quoting (see `gotchas.md`).

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

**Outcome for interchain-indexer (2026-07):** the operator surface (dedicated
per-domain prefixes, env wins over file) was adopted, but the mechanism was
not: instead of parallel JSON/env struct pairs and per-domain merge functions,
`env_merge.rs` deep-merges untyped `serde_json::Value` fragments guided by a
small id-key rule table (`ArrayRules`), and the existing strict structs do all
validation. Compound array keys (`contracts` by `(chain_id, address, version)`)
and named-map arrays (`rpcs`) are handled by the rule table.

## Change Triggers

Update this note when:
- `deny_unknown_fields` is added or removed from any config struct
- JSON loading switches from raw file I/O to `config` crate `File` source
- A `ConfigSettings::validate()` override is added
- New config files are introduced or existing ones restructured
- DB seeding logic changes (upsert conflict keys, name-conflict guard)
- The stats-style env-patching pattern is adopted

## Open Questions

1. ~~Should `ChainConfig`, `ExplorerConfig`, `BridgeContractConfig`, and
   `RpcProviderConfig` gain `deny_unknown_fields`?~~ Done (2026-07), plus
   `ApiKeyConfig`.
2. Should semantic validation (like `home_chain_id` check) move earlier — either
   into `ConfigSettings::validate()` or immediately after JSON loading — so it
   runs before DB seeding?
3. ~~Is the stats-style env-patching pattern worth adopting?~~ Resolved
   (2026-07) with the `env_merge` value-level deep-merge instead — see the
   outcome note above. All fields are patchable, including whole new entries.
