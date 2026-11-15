# ADR-018: OP Interop Indexer Reads Confirmed Blocks Only And Keeps A Leg Ledger

**Date:** 2026-11-14

**Authors:** @EvgenKor

**Status:** Accepted

## Context

OP Stack native interop (the Lagoon hardfork) lets a message sent through
`L2ToL2CrossDomainMessenger` on one chain be relayed on another chain of the
same dependency set. The protocol facts the indexer relies on (event grammar,
the two hashes, the absence of a failure event) are in
`research/op-interop-protocol-and-indexing-fit.md`.

Two properties of the existing pipeline matter here:

- `LogStream` follows `latest` and has no rollback. Upserts into
  `crosschain_messages` are merge-only, stats projection is additive, and
  checkpoint cursors are monotonic.
- An unsafe block that holds an `ExecutingMessage` whose initiating message
  turns out to be invalid is replaced by the sequencer, at the same height with
  a different block hash. Nothing in this pipeline can retract what was written
  from the replaced block.

Every interop log is also useful as evidence, independent of the message row
built from it: where exactly was it observed, in which block.

## Decision

### 1. Read only up to a confirmed block tag

The OP interop indexer reads only up to a block tag: `finalized` by default,
`safe` by setting (`confirmation_block_tag`). `latest` is not representable in
the settings type. The tag bounds the realtime head of the `LogStream`
(`HeadBlockTag`, resolved with `eth_getBlockByNumber(<tag>, false)`) and seeds
the first-start cursors of a pair that has no checkpoint. An unavailable head,
a `null` head, or a head behind the cursor means wait; there is **no fallback
to `latest`**. All other indexers keep the `Latest` default with the same RPC
call as before.

### 2. Fetch logs only

No receipts and no block headers. Block hash and block timestamp come from the
`eth_getLogs` rows. A log row without a block hash, timestamp, index or
transaction hash is a processing failure attributed to its block, so it lands
in `indexer_failures` and is retried; it is never skipped silently.

### 3. Grammar pinned in code, versioned by `(address, block)`

Contract entries in `bridges.json` carry a `kind` (`cross_l2_inbox`,
`l2_to_l2_messenger`) and a grammar `version`, not an ABI. The event
signatures and their `topic0` literals live in `indexer/op_interop/version.rs`;
the literals are transcribed from live logs, not derived from the `sol!`
types they check. Version windows are resolved by `(address, block)` exactly as
ADR-006 describes. A wrong kind, address or version is a startup error.

### 4. Identity is the messenger `messageHash`

`crosschain_messages.native_id` is the messenger `messageHash`, computed on the
source side and carried as the third topic of `RelayedMessage`.
`ExecutingMessage.msgHash` is the hash of the source *log*, an attribute and a
cross-check; it is never a key. A destination-first entry waits in the buffer
for its source: no canonical row is written for a destination-only entry.

### 5. The `op_interop_legs` ledger

One row per observed interop log, keyed
`(bridge_id, chain_id, block_hash, log_index)`, with the block hash stored from
the start so two observations of one height can coexist later. There is no
foreign key to `crosschain_messages`: `message_id` links by value, and a leg
row must be able to outlive a deleted message. Rows are upserted on that key;
duplicate keys inside one flush are de-duplicated before the statement,
because they would otherwise be a statement error that rolls back the
maintenance transaction.

### 6. Amendment to ADR-010 decision 3

ADR-010 keeps `protocol_metadata` sparse for resolved messages. The
`op_interop` namespace is the exception: it is written for **every** OP
message, is never cleared, and is merged **one level deep** at upsert
(`crosschain_messages_on_conflict`), because it is one object with a
sub-object per leg and a source-only flush can follow a complete one (a retry
or rescan re-reads the source block after the message was finalized and
evicted). Every other namespace still merges with a shallow `||`.

## Alternatives Considered

### Alternative 1: Read `latest` and handle reorgs

Would match the other indexers and have no latency, but needs retraction of
messages, stats and cursors that the pipeline was built without.

### Alternative 2: A per-bridge block tag

A tag on `bridges.json` instead of an indexer setting. Rejected: the tag is an
operational choice about RPC topology (`safe` is cross-chain validated only
behind op-supernode), not a property of the dependency set.

### Alternative 3: JSON ABI in the config

Rejected: the grammar is small, fixed per revision, and a mistyped ABI would
index nothing silently. Pinned literals plus a startup check catch it.

### Alternative 4: A destination-only completed row

Writing a `completed` row from a relay alone would lose the source side and
need a later merge into a row without `src_chain_id` or `init_timestamp`
(both NOT NULL).

### Alternative 5: Render `extra` from the ledger at read time

Keeps the message row small but turns every list endpoint into a join, and the
ledger deliberately has no link to the message table.

## Consequences

### Positive

- Nothing written can be retracted by a reorg of an unconfirmed block.
- No receipt or header RPC load: one `eth_getLogs` per range per chain.
- Per-leg evidence, with block hashes, is kept even for messages that never
  complete.

### Negative

- Indexed data lags the chain by 20 to 30 minutes at `finalized`.
- `sender_address` is the `sendMessage` caller, a contract for most traffic,
  not the transaction origin: no transaction is fetched.
- A one-sided replay (retry or rescan of only one chain's block) of an
  already finalized message leaves a row in `pending_messages` that never
  finalizes. Harmless to the canonical row; accepted.
- Nodes of one RPC pool can disagree about the confirmed head.

### Neutral

- Expiry and refund events, ETH transfers and pull reads are out of this
  slice.

## References

- `research/op-interop-protocol-and-indexing-fit.md`
- ADR-006 (version by block), ADR-010 (protocol metadata), ADR-014/015
- `interchain-indexer-logic/src/indexer/op_interop/`
