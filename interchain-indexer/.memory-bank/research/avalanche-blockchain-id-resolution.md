# Avalanche Blockchain ID Resolution

## Scope

This note explains how the Avalanche indexer resolves Avalanche native
`blockchain_id` values into numeric EVM `chain_id` values, where that
resolution sits in the indexing flow, and how cache, config, database, and the
Avalanche Data API are intended to interact.

It covers only the Avalanche-native indexer path. It does not cover message
consolidation rules, serving-layer API behavior, or non-Avalanche indexers.

## Short Answer

`BlockchainIdResolver` is an Avalanche-specific helper used by the Avalanche
indexer before bridge filtering. Teleporter events identify peer chains by
32-byte Avalanche `blockchain_id`, but the rest of the service filters,
buffers, and persists by numeric EVM `chain_id`.

Current implementation resolves in this order:

1. in-memory Moka cache
2. `avalanche_icm_blockchain_ids` in Postgres
3. Avalanche Data API

Intended behavior established during research is broader than current code:

1. in-memory cache
2. static configured `native_id` mappings from `chains.json`
3. persisted DB mapping table
4. Avalanche Data API

Also, `process_unknown_chains` is intended to control whether bridge-unknown
chains discovered by the resolver are persisted into `chains` and
`avalanche_icm_blockchain_ids`. Current code is not fully aligned with that
rule.

## Why This Matters

Resolution happens before bridge filtering, so it directly affects whether a
Teleporter event is processed or skipped.

Persistence behavior also matters operationally. If unknown-chain resolution is
persisted too aggressively, bridge-local discovery leaks into global chain
metadata. If resolution skips static config and DB cache prewarm, the service
makes unnecessary external API calls.

## Source-of-Truth Files

- `interchain-indexer-logic/src/indexer/avalanche/blockchain_id_resolver.rs`
- `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
- `interchain-indexer-logic/src/avalanche_data_api.rs`
- `interchain-indexer-logic/src/database.rs`
- `interchain-indexer-logic/src/indexer/avalanche/settings.rs`
- `interchain-indexer-server/src/config.rs`
- `interchain-indexer-server/src/settings.rs`
- `config/avalanche/chains.json`
- `config/avalanche/bridges.json`

## Key Types / Tables / Contracts

- `BlockchainIdResolver`
- `AvalancheDataApiClient`
- `AvalancheDataApiClientSettings`
- `AvalancheIndexerSettings`
- `BridgeConfig.process_unknown_chains`
- `BridgeConfig.home_chain_id`
- `chains`
- `avalanche_icm_blockchain_ids`

## Step-by-Step Flow

1. `AvalancheIndexer::run()` constructs one `BlockchainIdResolver` for the
   running indexer instance.
2. Each Teleporter log batch is processed per transaction.
3. Event handlers call `resolve(...)` before applying
   `should_process_message(...)`.
4. The resolved numeric `chain_id` is then used by bridge filtering and
   downstream message assembly.

Current resolver call sites:

- `SendCrossChainMessage`
  - resolves `destinationBlockchainID`
- `ReceiveCrossChainMessage`
  - resolves `sourceBlockchainID`
- `MessageExecuted`
  - resolves `sourceBlockchainID`
- `MessageExecutionFailed`
  - resolves `sourceBlockchainID`

This placement is required because bridge filtering operates on numeric
`chain_id`, not Avalanche native blockchain IDs.

## Current Implementation

### Construction and Configuration

`BlockchainIdResolver` is created inside `AvalancheIndexer::run()` from
`AvalancheIndexerSettings.data_api_client_settings`.

Resolver-specific configuration currently includes only:

- Avalanche Data API `network`
- optional Data API `api_key`

The resolver has no standalone config section. Some important behavior is
hardcoded today:

- Moka cache capacity is fixed at `10_000`
- reqwest timeouts and retry policy are fixed in `AvalancheDataApiClient`

### Resolution Order

Current `resolve(blockchain_id, process_unknown_chains)` behavior:

1. Validate `blockchain_id` is exactly 32 bytes.
2. Check the in-memory Moka cache.
3. Check `avalanche_icm_blockchain_ids`.
4. Call the Avalanche Data API.
5. Read `evm_chain_id` from the response.
6. Optionally persist discovered data.
7. Return the resolved `chain_id`.

The in-memory cache uses `try_get_with`, so concurrent lookups for the same
blockchain ID are coalesced within one process.

### Avalanche Data API Interaction

The resolver uses the Avalanche Data API only on cache and DB miss.

Current endpoint:

- `GET https://data-api.avax.network/v1/networks/{network}/blockchains/{blockchain_id_cb58}`

Where:

- `{network}` is configured as `mainnet`, `fuji`, or `testnet`
- `{blockchain_id_cb58}` is the 32-byte Avalanche blockchain ID encoded as CB58

Current request shape:

- method: `GET`
- header: `Accept: application/json`
- optional header: `x-glacier-api-key: <api_key>`

Current response fields defined by the client:

- `blockchainId`
- `blockchainName`
- `evmChainId`

The resolver uses:

- `evmChainId` as the returned numeric chain ID
- `blockchainName` as the preferred chain name when it decides to persist a
  discovered chain into `chains`

Current client behavior:

- connect timeout: 5 seconds
- total request timeout: 15 seconds
- retry middleware with exponential backoff
- maximum retries: 5

### Persistence Logic

Current slow-path persistence is controlled by this effective rule:

- if `process_unknown_chains == true`, persist discovered chain/mapping
- if `process_unknown_chains == false`, persist only when the resolved
  `chain_id` already exists in `chains`

When persistence is attempted:

1. `ensure_chain_exists(chain_id, Some(chain_name), None)` runs first to
   satisfy the FK target
2. `upsert_avalanche_icm_blockchain_id(blockchain_id, chain_id)` writes the
   mapping

Both writes are best-effort. Failure is warn-logged and does not block
returning a resolved `chain_id` after successful Data API lookup.

## Intended Behavior Confirmed During Research

The following expectations were explicitly confirmed during research and should
be treated as the intended model even though current code does not fully match:

### `process_unknown_chains` Controls Persistence for Unknown Chains

Example: the bridge indexes chains `A` and `B`, but encounters a message from
`B` to `C`.

- if `process_unknown_chains = true`
  - resolver should resolve chain `C`
  - resolver may persist `C` into `chains`
  - resolver may persist the `blockchain_id -> chain_id` mapping
- if `process_unknown_chains = false`
  - resolver should still return the resolved EVM `chain_id` for filtering
  - resolver should not persist that bridge-unknown chain or mapping

This makes `process_unknown_chains` the persistence gate for bridge-unknown
chains.

### Static `native_id` Config Should Be Consulted Before the Data API

`config/avalanche/chains.json` already contains `native_id` values for known
Avalanche chains. The resolver should consult those static mappings before
making an external Avalanche Data API call.

Current code does not do this. More specifically, the current config model does
not even deserialize `native_id` into `ChainConfig`, so these mappings are not
available to runtime code today.

### In-Memory Cache Should Be Prewarmed From Postgres

The database layer provides `load_native_id_map()` and its comment describes
pre-populating an in-memory resolver cache from persisted mappings.

This should happen on startup or resolver construction so already-known native
ID mappings do not require one database lookup per process lifetime before they
become hot.

Current runtime does not call `load_native_id_map()`.

## Current Mismatches

### Missing Static Config Lookup

The resolver does not currently consult configured `native_id` mappings from
`chains.json` before calling the Avalanche Data API.

This means the service may call the external API for chains that are already
fully known in static config.

The gap starts earlier than the resolver itself: `native_id` values exist in
the JSON files, but `interchain-indexer-server/src/config.rs` defines
`ChainConfig` without a `native_id` field, so `load_chains_from_file(...)`
drops these values during deserialization.

### Missing DB Cache Prewarm

`load_native_id_map()` exists in `InterchainDatabase`, but the runtime never
uses it to seed the resolver cache.

This means persisted mappings only become hot after the first lookup in each
process lifetime.

### Strict-Mode Persistence Is Too Permissive

Current strict-mode behavior still persists when the resolved `chain_id`
already exists in `chains`, even if `process_unknown_chains = false`.

That is looser than the intended semantics established during research. Under
the intended rule, strict mode should not persist bridge-unknown chain
discoveries at all.

## Invariants

- This resolver is Avalanche-specific, not a generic chain identity abstraction.
- Event handlers need numeric `chain_id` values before bridge filtering can run.
- `avalanche_icm_blockchain_ids` is the persisted acceleration layer for
  repeated native-ID resolution.
- The table currently enforces `UNIQUE(chain_id)`, so the current model assumes
  one native Avalanche blockchain ID per EVM `chain_id`.

## Failure Modes / Observability

- non-32-byte input causes immediate resolver failure
- Avalanche Data API request failures bubble up from event handling
- missing `evm_chain_id` in the API response is treated as an error
- persistence failures are warn-logged but do not block successful resolution

Useful places to inspect:

- resolver warnings in `blockchain_id_resolver.rs`
- Data API client behavior in `avalanche_data_api.rs`
- bridge filtering trace logs in Avalanche event handlers

## Edge Cases / Gotchas

- Resolution can happen even for messages that are later filtered out by bridge
  policy, because resolution occurs before `should_process_message(...)`.
- Chain-level `native_id` values currently exist only in static JSON config.
  They are not represented in the runtime `ChainConfig` model or the `chains`
  table schema, so they cannot currently participate in runtime resolution.
- Without config-first lookup and DB cache prewarm, repeated observations of the
  same native blockchain ID can cause avoidable Data API traffic.
- Current implementation mixes bridge-local unknown-chain policy with global
  persisted metadata in a way that does not fully match intended semantics.

## Change Triggers

Update this note when any of the following changes:

- schema or constraints for `chains` or `avalanche_icm_blockchain_ids`
- startup config model for Avalanche chains or bridges
- resolver lookup order
- strict-vs-unknown-chain persistence behavior
- Avalanche Data API contract or dependency model
- any attempt to generalize this logic beyond Avalanche

## Open Questions

- Should resolver failure on Data API lookup be treated as a hard indexing error
  or downgraded to a soft skip for the affected message?
- Is `UNIQUE(chain_id)` in `avalanche_icm_blockchain_ids` the right long-term
  invariant, or only a current simplifying assumption?
