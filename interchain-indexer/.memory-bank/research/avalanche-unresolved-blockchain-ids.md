# Unresolved Avalanche Blockchain IDs

Accepted scope: [ADR-010](../adr/010-unresolved-avalanche-destinations-and-protocol-metadata.md)
(2026-09-10, implementation pending). Destination failures gain canonical
metadata; source failures retain current ledger/replay behavior.

## Scope

This note records how unresolved peer `blockchainID` values affect Avalanche
Teleporter indexing, the production evidence observed in September 2026, and
the intended boundary between permanent data conditions and retryable indexing
failures. It covers the `full-mainnet` Avalanche bridge and the shared failed
range replay path; general Data API resolution behavior remains in
`avalanche-blockchain-id-resolution.md`.

## Short Answer

Ten C-Chain blocks contain `SendCrossChainMessage` events whose destination
blockchain IDs cannot be resolved in the configured `mainnet` Data API network.
The Data API returns a permanent `404 Blockchain not found`, but the resolver
turns every non-success response into a processing error. Adaptive replay now
correctly narrows the original broad holes to the ten individual poison blocks,
but singleton replay cannot make a permanent 404 succeed, so
`catchup_complete` remains false.

For an outbound destination, an expected not-found outcome should be recorded
observably without keeping the block in `indexer_failures`. Unresolved source
identity intentionally remains an error/replay case under ADR-010; transport
failures, 429 and 5xx also remain retryable.

## Why This Matters

`full-mainnet` uses `process_unknown_chains: true`, so a destination not already
known to the service is resolved before bridge filtering. Without permanent
error classification, any Teleporter send to a foreign-network, unregistered,
or otherwise unavailable `bytes32` destination can keep historical catch-up
incomplete forever.

Adaptive narrowing limits the reported hole to the exact affected blocks. It
does not remove the underlying livelock.

## Production Evidence

The production snapshot supplied on 2026-09-10 contains ten disjoint singleton
rows for `(bridge_id = 2, chain_id = 43114)`, each with the reason
`failed to fetch blockchain info from Avalanche Data API` and 23 or 24
attempts:

| Destination blockchain ID (CB58) | Hex | Poison blocks | Resolution result |
| --- | --- | --- | --- |
| `yH8D7ThNJkxmtkuv2jgBa4P1Rn3Qpr4pPr7QYNfcdoS6k6HWp` | `0x7fc93d85c6d62c5b2ac0b519c87010ea5294012d1e407030d6acd0021cac10d5` | `48344083`, `48634870` | mainnet: 404; Fuji: C-Chain, EVM chain `43113` |
| `trF28V1BaHHov1fCGWNtiKzKT1CmaNuxe8x8UDKcfegEHiqoJ` | `0x75babf9b4db10c46cd1c4cc28e199cc4acf4c64f78327ff6cda26b8785a7bb5d` | `48736763` | mainnet and Fuji: 404 |
| `FCye47rDVEicKFZmvQnUPGAqrAYJBTuyqvS2kXGEhH5PRr59d` | `0x2041f09f4b533efdd94ef851bcee8d49556f56011178cc4937419bd1f08a0163` | `53843829`, `53844151`, `53845062`, `53845814`, `53846789`, `53847699` | mainnet: 404; Fuji: Armada, EVM chain `47208` |
| `UPZWZV11dfeRLWZYN8Lr1hbpVkbX6rX1dAysLFgBPQe2GWGDC` | `0x3e30a4f0e31d8ec3b8e98957bc7fedf7f6fb560612e2775c74a396200aa3155b` | `78434796` | mainnet and Fuji: 404 |

These are ten failed blocks, not 9,000: the earlier 9,000-block total was the
coarse ledger coverage before adaptive replay isolated the actual poison set.

## Source-of-Truth Files

- `config/full-mainnet/bridges.json`
- `interchain-indexer-logic/src/avalanche_data_api.rs`
- `interchain-indexer-logic/src/indexer/avalanche/blockchain_id_resolver.rs`
- `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
- `interchain-indexer-logic/src/indexer/range_driver.rs`
- `interchain-indexer-logic/src/indexer/retry_scheduler.rs`
- `interchain-indexer-logic/src/indexer/failure_ledger/`
- `interchain-indexer-logic/src/indexer/progress.rs`
- `.memory-bank/research/avalanche-blockchain-id-resolution.md`
- `.memory-bank/research/indexing-gaps-retries-and-checkpoint-safety.md`

## Key Types / Tables / Contracts

- `AvalancheDataApiClient::get_blockchain_by_id`
- `BlockchainIdResolver::resolve`
- `RangeDriver`
- `RetryScheduler`
- `FailureLedger`
- `indexer_failures`
- `crosschain_messages`
- `BridgeConfig.process_unknown_chains`

## Step-by-Step Flow

1. C-Chain yields a valid Teleporter `SendCrossChainMessage` log.
2. The handler asks `BlockchainIdResolver` to map
   `destinationBlockchainID` to an EVM chain ID before filtering the message.
3. Cache and `avalanche_icm_blockchain_ids` miss, so the resolver calls the
   Data API for the configured `mainnet` network.
4. The Data API returns 404. `error_for_status` converts it to a generic error,
   and the resolver's `err.to_string()` boundary discards the nested HTTP
   details before the error reaches the ledger.
5. `RangeDriver` records the failed coverage and later replays it.
6. After the configured number of wholly unsuccessful sweeps, `RetryScheduler`
   repeatedly halves request width. Successful subranges are removed from the
   ledger; the failing coverage converges to singleton blocks.
7. Each singleton still contains the same event and receives the same 404, so
   it remains open indefinitely. `catchup_complete` requires both 100% scan
   progress and zero failed blocks.

## Accepted Handling (Implemented)

- For an otherwise accepted outbound send, treat an expected network-scoped
  not-found or valid response without evmChainId as an unresolved destination
  to preserve, not a permanent failed block. Keep technical errors retryable.
- Persist the send as an ordinary crosschain_messages row with its known source,
  dst_chain_id=NULL and raw destination ID/network/reason in the new nullable
  protocol_metadata JSONB column. Render these details into Read API extra.
- protocol_metadata and its surrounding structures must be reusable by other
  indexers: common extensible container/boundary, typed protocol-specific
  payload and explicit public renderer. Avalanche is the initial use case.
- For successfully resolved endpoints, protocol_metadata stays SQL NULL.
  Do not store healthy-path identity duplicates, resolved markers, empty
  objects or JSON null. If later processing resolves an earlier unknown
  destination, clear its obsolete metadata; stale replay must not restore it.
- Keep unresolved-source behavior exactly as today: processing error,
  indexer_failures and later replay. src_chain_id remains NOT NULL. The accepted
  architectural assumption is an Avalanche Data API issue such as indexing
  lag for a new blockchain. Replay may eventually succeed; persistent failures
  remain visible for a separate investigation. No examples are currently known.
- Shared resolver/cache changes must preserve that source failure/retry path;
  do not apply outbound acceptance indiscriminately to source lookups.
- Preserve bridge policy and message execution status. Do not create synthetic
  chains or cross-network mappings. Defer direction stats/ICTT rows until a
  numeric destination exists and keep ordinary pending state.
- Add structured destination diagnostics, while source failures remain visible
  through the existing ledger. Background destination enrichment is deferred.
- No new unresolved table, nullable-source migration, source API optionality
  or related stats/filter refactor is included in this task.

## Invariants

- `indexer_failures` means blockchain coverage that could not be processed for
  a retryable technical reason, including unresolved source identity under
  ADR-010. Expected unresolved outbound destinations are represented in the
  canonical row instead of remaining in the ledger.
- `message_status = failed` remains a destination execution outcome, not an
  identity-resolution status.
- `process_unknown_chains = true` permits discovery of resolvable chains; it
  does not require every arbitrary `bytes32` value to resolve.
- Narrowing a failure to one block improves accuracy but cannot make a
  deterministic permanent failure recoverable.

## Failure Modes / Observability

`indexer_failures.reason` now carries the full error chain
(`anyhow!("{err:#}")` in `blockchain_id_resolver.rs`, rather than
`err.to_string()`, which discarded it) and `DataApiError` is typed, so a
permanent 404 (`Avalanche Data API returned status 404 Not Found: ...`) is
distinguishable from a timeout or 5xx directly in the ledger. A confirmed
missing destination (`message == "Blockchain not found"`) no longer reaches
`indexer_failures` at all on the outbound path — it is classified as
`Resolution::Unresolved` and persisted as data (ADR-010); only unrecognized
Data API errors remain in the ledger.

Deleting the singleton rows manually would report false completion and skip
the intended handling of those messages. Deploy permanent-error classification
first, then let normal replay resolve the rows.

## Edge Cases / Gotchas

- A foreign-network ID may be valid elsewhere while correctly returning 404 in
  the configured network.
- A 404 may reflect a permanently invalid ID, a decommissioned chain, or Data
  API registration lag. The accepted non-blocking handling applies to outbound
  destinations; unresolved sources retain error/replay handling.
- Incoming events with an unresolved source do not fit the current canonical
  schema as easily as outbound events: `crosschain_messages.src_chain_id` is
  non-null. ADR-010 explicitly keeps this invariant and the current failure
  ledger/replay behavior; a different source representation is outside scope.

## Change Triggers

Update this note when Data API errors become typed, unresolved directions gain
persistence/API representation, `process_unknown_chains` semantics change, or
retry narrowing/completion rules change.

## Remaining Implementation Details

The user selected the canonical metadata/Read API approach and explicitly
excluded source changes. Remaining details are the reusable Rust/JSON shape,
extra-key names, sparse merge/clear rules and destination cache policy.
Do not reopen source nullability or healthy-path metadata as implementation
choices; they are settled by ADR-010.

## Nullable Source Impact Audit (2026-09-10)

This is supporting research for an excluded alternative. ADR-010 keeps source
NOT NULL; none of the source changes below belongs to the accepted task.

Source identity is a stronger current storage assumption than destination
identity. The initial migration already made source NOT NULL and destination
nullable (commit `c57d6a2c`), even while allowing `src_tx_hash` to be absent.
No separate rationale for the source constraint was found in the inspected
history/ADRs. Current code consistently distinguishes an unobserved source
transaction from an unresolved source chain.

Relaxing the constraint is structurally possible: source is not part of the
message primary key, pagination key, or child-message foreign keys. It is not
sufficient by itself. Confirmed dependencies include:

- The generated canonical message model and API serializer require an `i64`
  source. A nullable schema plus an unchanged reader fails when reading a NULL
  row; the serializer must also support absent source `ChainInfo`.
- Avalanche annotations, source fallback, payload classification and ICTT
  reconstruction require numeric identities today. AMB knows its source from
  a header or configured counterpart and should retain strict domain types.
- Live message stats and startup backfill filter out only NULL destinations.
  A NULL source must be deferred explicitly before the mandatory direction
  tables. Projection errors roll back the shared maintenance transaction,
  including messages, pending state and cursors.
- `STATS_CHAINS_MESSAGE_USER_COUNTS_SQL` selects senders without checking
  source chain. A synthetic PostgreSQL probe using the exact query produced
  NULL-chain groups; `BridgeChainUserCountRow.chain_id: i64` cannot represent
  them. The known-destination recipient contribution can remain countable.
- The absent-bridge arm of `ChainBridgeFilter::messages_condition()` in
  `interchain-indexer-filters/src/lib.rs` only guards destination non-null.
  It needs a symmetric source guard if source becomes nullable. This crate
  also serves the sibling `stats` service, whose git-pinned dependency set
  must be updated explicitly.
- Existing source upsert COALESCE already supports missing-to-known enrichment
  without erasing a known source on a later missing observation. New identity
  metadata must merge consistently with that scalar value.
- Completed execution with unresolved identity needs an explicit buffered-state
  retention policy. Protocol completion alone does not establish that all data
  needed for future transfer reconstruction may be discarded.

Validation used synthetic temporary PostgreSQL tables within a rolled-back
transaction; no production data or schema were changed. It confirmed the
NULL-source stats constraint failure and rollback of a healthy write in the
same transaction block, as well as the filter discrepancy and source COALESCE
behavior. It was not an end-to-end implementation test.

Consequently nullable source requires a coordinated data-model change and
reader/rollout compatibility work. It is unnecessary for fixing only the ten
outbound blocks described above. No nullable-source migration has been applied.
