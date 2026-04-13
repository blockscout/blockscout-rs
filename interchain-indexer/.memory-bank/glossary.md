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

## Teleporter / ICM

Avalanche native interchain messaging protocol. In this repo, Teleporter / ICM
events are the main message-level signal for the Avalanche indexer.

## ICTT

Avalanche Inter-Chain Token Transfer protocol. ICTT events extend message flows
with token transfer semantics and affect finality and stats behavior.

## Source-Indexed Data

Data attached to a message when the source-side chain was indexed directly.
Presence of source-side fields such as `src_tx_hash` is often the stable signal
for “source-indexed” semantics.

## Destination-Indexed Data

Data attached to a message when the destination-side chain is the indexed side
or when only destination-side observations are available.
