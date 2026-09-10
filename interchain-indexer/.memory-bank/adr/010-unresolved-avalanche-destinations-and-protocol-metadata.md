# ADR-010: Unresolved Avalanche Destinations And Reusable Protocol Metadata

**Date:** 2026-09-10

**Status:** Accepted

## Context

Avalanche send events may name a destination blockchain ID that the configured
Data API network cannot resolve. Treating every such outcome as a processing
failure leaves historical blocks in endless replay. The observed production
cases are outbound; no unresolved-source examples have been identified in
the investigation.

`crosschain_messages.dst_chain_id` is already nullable, while source identity
is mandatory throughout storage and serving. The Read API already has an
`extra` map for additional parsed data. An earlier proposal to permit NULL
sources would expand this task into a cross-service data-model migration.

## Decision

### 1. Preserve unresolved destinations in canonical messages

Choose the approach with one new nullable JSONB column named
`crosschain_messages.protocol_metadata`, rather than dedicated direction
columns or a separate unresolved-message table.

For an otherwise accepted outbound send whose destination lookup returns an
expected resource-not-found or a valid response without an EVM chain ID:

- retain the known source, send transaction, payload and native message ID;
- store `dst_chain_id = NULL` and the original destination blockchain ID,
  lookup network and resolution reason in `protocol_metadata`;
- render the relevant metadata into `InterchainMessage.extra` in the Read API;
- handle the event successfully through the existing buffer/persistence path
  so that this data condition does not keep the block in `indexer_failures`;
- preserve execution-status semantics and existing bridge filtering;
- defer direction stats and canonical ICTT transfers until destination
  identity is known, retaining the existing buffered event data.

Transport failures, rate limiting, server errors and unrecognised/malformed
responses are not automatically equivalent to a missing destination.

The signal that a destination is genuinely missing is the response **body**,
not the HTTP status: a 404 for a valid-but-unknown blockchain ID and a 404
for a mistyped API path are otherwise indistinguishable (same status, same
`Content-Type`, same top-level `"error":"Not Found"`) — only the `message`
text differs. Only an exact, trimmed, case-insensitive match on
`"Blockchain not found"` is treated as a confirmed missing destination;
everything else, including a `"Cannot GET ..."` 404, stays a processing
error. See the fixtures captured against the live Data API (2026-09-10) in
`interchain-indexer-logic/src/avalanche_data_api.rs`'s
`classify_error_response` and its tests, and the
[gotcha](../gotchas.md#avalanche-data-api-has-two-indistinguishable-404-shapes)
for the two-404-shapes trap this classification exists to avoid.

### 2. Preserve current unresolved-source behavior

`src_chain_id` stays NOT NULL. Source-resolution failure remains a processing
error: affected block coverage is recorded in `indexer_failures` and retried
through the existing machinery. Do not add a successful skip, canonical
NULL-source row, quarantine path, give-up policy or special source recovery.

The accepted architectural assumption is that an unresolved source reflects
an Avalanche Data API problem, for example delayed indexing of a new chain.
This is the chosen operational model, not a newly observed production fact.
Later replay may succeed when the API catches up; persistent failures remain
visible in `indexer_failures` for a separate investigation. This task must not
attempt to solve that scenario, particularly without concrete examples.

Shared resolver changes, including any caching, must preserve this source
failure/retry behavior rather than applying the outbound acceptance policy
to every lookup indiscriminately.

### 3. Keep metadata sparse

For messages with successfully resolved endpoints, `protocol_metadata` must
remain SQL NULL. Do not write `{}`, JSON `null`, a `resolved` marker or duplicate
native identity/network data on the healthy path. Do not backfill metadata
for already resolved messages.

If a previously unresolved destination becomes known through normal later
processing, clear its no-longer-needed unresolved-destination metadata. In
this task that leaves the column SQL NULL. A stale unresolved observation
must neither erase a known destination nor reintroduce its obsolete metadata.
These are consistency requirements for the selected sparse representation;
they do not introduce a background enrichment worker.

### 4. Make the representation reusable

The column and surrounding structures must be generic protocol metadata,
designed for reuse by other indexers. Shared storage/serialization and the
Read API rendering boundary must not be defined as an Avalanche-only model.
Protocol-specific payloads and their extra-field rendering remain typed and
explicit, with an extensible protocol identifier/namespace or equivalent
mechanism. Do not build a framework of speculative indexers or invent their
payloads in this task.

Exact Rust types, JSON discriminator/layout, merge expressions and public
extra-key names belong in the implementation plan. Extensibility must preserve
the NULL healthy-path behavior selected here; later metadata use cases require
their own explicit semantics rather than eagerly populating every message.

## Alternatives Considered

- Separate columns for native destination ID, network and reason: rejected in
  favor of the reusable metadata column.
- Dropping messages or storing them only in the internal pending tier:
  rejected because canonical Read API visibility is required.
- Separate unresolved-message table: unnecessary additional storage and API
  lifecycle for the observed outbound case.
- Nullable source identity: investigated but excluded from this task. It
  changes entity readers, stats, shared filters and dependent services.
- Metadata on successfully resolved messages: rejected to avoid unnecessary
  growth of `crosschain_messages`.
- Synthetic chain IDs or cross-network mappings: do not fabricate a resolved
  endpoint to satisfy a foreign key.

## Consequences

- Unresolved outbound messages remain inspectable without permanent replay
  solely because their destination is not resolvable.
- Unresolved inbound sources intentionally retain current error visibility
  and retry behavior; this change does not guarantee their catch-up completion.
- Metadata growth is limited to exceptional messages in the selected scenario;
  serialization, merge and API rendering must preserve this sparse lifecycle.
- No nullable-source migration, source API optionality change or associated
  stats/filter refactor is part of this task.
- Automatic destination enrichment remains deferred. The existing gap between
  successful processing and durable buffer flush described in ADR-005 remains.

## References

- [Unresolved Avalanche blockchain IDs](../research/avalanche-unresolved-blockchain-ids.md)
- [Blockchain ID resolution](../research/avalanche-blockchain-id-resolution.md)
- [ADR-005: failure ledger](005-failed-range-ledger-and-checkpoint-independence.md)
- `interchain-indexer-logic/src/indexer/avalanche/blockchain_id_resolver.rs`
- `interchain-indexer-logic/src/indexer/avalanche/mod.rs`
- `interchain-indexer-logic/src/message_buffer/persistence.rs`
- `interchain-indexer-server/src/services/interchain_service.rs`
- `interchain-indexer-proto/proto/v1/interchain_indexer.proto`
