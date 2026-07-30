# Glossary

## Bridge

A configured cross-chain mechanism that the service indexes as a single logical
unit. In this repo, a bridge carries metadata such as `bridge_id`, type,
enabled flag, filtering settings, and per-chain contracts.

## Bridge Contract

A contract address on a specific chain that belongs to a configured bridge.
Bridge contracts tell the indexer which on-chain logs to stream for that
bridge.

## Configured Chain

A chain that exists in `chains.json` and has usable RPC configuration. In
practice, bridge filtering logic treats configured chains as the known endpoints
 the indexer can reason about directly.

## Unknown Chain

A chain endpoint referenced by observed events but not fully configured for the
current bridge. Unknown-chain handling is controlled by
`process_unknown_chains`.

## Home Chain

An optional bridge setting that narrows indexing to messages where at least one
endpoint equals a specific chain. It is represented by `home_chain_id`.

## Cross-Chain Message

The canonical stored representation of an interchain message flow. In this repo
it is assembled from one or more protocol events and persisted into
`crosschain_messages` when the system has enough information.

## Cross-Chain Transfer

A transfer attached to a cross-chain message, typically representing token
movement associated with ICTT flows. Finalized transfers are stored in
`crosschain_transfers`.

## Pending Message

Intermediate persisted state for messages that are not yet final or were
offloaded from the hot in-memory buffer. Pending rows allow progress without
requiring every message to finalize in memory.

## Message Buffer

The tiered state assembly layer that keeps active message state in memory and
persists colder state to the database. It is the central mechanism for
assembling cross-chain state from multiple events over time.

## Consolidation

The step where the current buffered message state is evaluated and converted
into a `ConsolidatedMessage` candidate. Consolidation may return:

- nothing yet
- a partial consolidated state
- a final consolidated state

## Finality

The repo-specific condition under which a message is ready for canonical final
storage. For Avalanche flows, finality is more complex than “an execution event
exists”; it depends on execution success and, for ICTT, transfer completion.

## Checkpoint

Persisted cursor state that lets an indexer resume log streaming safely after
restart. Checkpoints are updated from message-buffer maintenance rather than
directly from raw log observation.

## Projection

A derived write from canonical tables into aggregate tables. In this repo,
stats are projections from `crosschain_messages` and `crosschain_transfers`,
not primary ingestion tables.

## Observability Horizon

The stats layer's eligibility rule (ADR-004): a message or transfer counts
once the evidence it is still missing can no longer arrive, because the chain
that would produce it is not indexed by that bridge. Answered by exactly one
method, `IndexedChains::may_observe(bridge_id, chain_id)`, used identically by
projection and by the read-side unindexed-chain filter. Distinct from
protocol finality — a message can be observability-horizon-countable while
its protocol `status` is still `Initiated`.

## `IndexedChains`

The per-bridge set of chains a bridge actually indexes (a configured
contract there), built once at startup from the in-memory `bridges.json`
config — never from the `bridge_contracts` table, which is stale exactly when
backfill needs an accurate set. `AllIndexed` (no config, permissive default)
or `PerBridge(HashMap<bridge_id, HashSet<chain_id>>)`. A bridge absent from
the map is permissive (existing history stays countable); a bridge present
with an empty chain set is restrictive (a misconfiguration surfaced by a
startup warning). See `interchain-indexer-logic/src/stats/indexed_chains.rs`.

## Countable / Deferred (Stats)

The two outcomes of the observability-horizon eligibility check. *Countable*
means a message/transfer's missing evidence either arrived or can never
arrive, so it is safe to count now (increments `stats_processed` and the
matching aggregate). *Deferred* means the evidence could still arrive later
(its chain is indexed by the bridge), so the row waits, uncounted, for a
future flush. Deferral is re-evaluated, not remembered — a deferred row is
simply re-checked the next time its canonical key is flushed.

## Union-Find Asset Merge

The strategy for resolving bridged-token identity in `stats_assets` /
`stats_asset_tokens` (ADR-004): a transfer is an edge between two token
vertices, an asset is a connected component, and linking two tokens already
in different components merges the components (repointing tokens, edges,
transfers, then deleting the losing row) rather than refusing. The only
unresolvable conflict is two different tokens of one chain landing in the
same asset. See `merge_assets` / `ensure_asset_for_transfer` in
`stats/projection.rs` and `gotchas.md`, "Stats Asset Mapping Conflicts Merge;
Only Same-Chain Collisions Skip."

## Teleporter / ICM

Avalanche native interchain messaging protocol. In this repo, Teleporter / ICM
events are the main message-level signal for the Avalanche indexer.

## ICTT

Avalanche Inter-Chain Token Transfer protocol. ICTT events extend message flows
with token transfer semantics and affect finality and stats behavior.

## ICM Payload / `TransferrerMessage`

The ICTT envelope carried inside `TeleporterMessage.message`, decoded by
`indexer/avalanche/ictt_payload.rs`. Its `messageType` classifies the hop
into `REGISTER_REMOTE`, `SINGLE_HOP_SEND`, `SINGLE_HOP_CALL`,
`MULTI_HOP_SEND`, or `MULTI_HOP_CALL`. `SINGLE_HOP_*` means a destination
credit event is expected before the transfer is complete; `MULTI_HOP_*` /
`REGISTER_REMOTE` mean none will ever arrive for this message id (a multi-hop
first leg re-sends under a *new* id at the home chain instead of crediting a
recipient) — this classification is what lets `consolidation.rs` finalize
such a message without waiting forever for a destination event that will
never come. Decoding enforces a canonicity round-trip (re-encode and
byte-compare) to reject non-canonical ABI encodings or trailing bytes.

## Incoming ICTT Reconstruction

The second transfer-building path in `consolidation.rs`
(`try_reconstruct_transfer` / `build_reconstructed_transfer`), which builds a
`crosschain_transfers` row purely from the ICM payload and a receiver-side
ICTT effect when the source chain is unconfigured (no `send` event will ever
arrive). Fires only for `SINGLE_HOP_SEND` / `SINGLE_HOP_CALL`; never races a
real `send` event; gated per bridge by `bridges.json`'s
`reconstruct_incoming_ictt_transfers` (default `true`).

## Unindexed-Chain Read Filter

The opt-in read-side surface built on `IndexedChains::may_observe`:
`include_unindexed_chains` (request field, default `false`) widens list and
stats endpoints to include rows a bridge could not have fully observed;
`has_unindexed_chain` (response field on messages/transfers) flags such rows
either way; `indexed_chain_ids` (response field on `Bridge`) reports a
bridge's actual configured chain set. See `stats-subsystem.md`.

## Source-Indexed Data

Data attached to a message when the source-side chain was indexed directly.
Presence of source-side fields such as `src_tx_hash` is often the stable signal
for “source-indexed” semantics.

## Destination-Indexed Data

Data attached to a message when the destination-side chain is the indexed side
or when only destination-side observations are available.
