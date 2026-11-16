# ADR-019: OP Interop ETH Transfers Are Derived From The Message Payload

**Date:** 2026-11-16

**Authors:** @EvgenKor

**Status:** Accepted

## Context

A native ETH transfer through `SuperchainETHBridge` (`0x4200…0024`) travels as
an ordinary `L2ToL2CrossDomainMessenger` message whose sender and target are
both the bridge and whose payload is `relayETH(from, to, amount)`. The
messenger row therefore names the bridge, not the user; the user addresses and
the amount are only in the payload, and in the bridge's `SendETH` / `RelayETH`
events, which carry neither a nonce nor a message hash and can only be linked
to a message by their position in the transaction.

Building the transfer from those events would make the result depend on a
positional pairing; a missed or mis-paired event would silently change or drop
a transfer. `relayETH` is callable only by the messenger during a relay of a
message the bridge itself sent, so anyone can send a message targeting the
bridge, but only a genuine `sendETH` message can ever be relayed.

## Decision

- The transfer is derived from the `SentMessage` payload alone
  (`op_interop/eth_transfer.rs::classify`), under the rule sender = target =
  bridge and a payload of exactly `relayETH(address, address, uint256)`
  (100 bytes, the known selector, clean address words). Anything else is no
  transfer. A bridge-to-bridge payload of another shape is an observation and
  a counter, never an error.
- `SendETH` and `RelayETH` are diagnostics only. They are paired by position
  inside the transaction, cross-checked against the payload, and surface as
  observations, `DestinationLeg.relay_eth` and the
  `interchain_indexer_op_interop_eth_transfer_anomalies_total` counter. They
  never create, suppress or change a transfer row.
- The row is written from the first flush (`initiated`) and is identical after
  the relay. Stats count it exactly once, after the message is `completed`, by
  the unchanged projection (an OP message is countable only when completed).
- The token is the zero-address native sentinel
  (`xdai::NATIVE_SENTINEL`, re-exported) on both chains with `Mirror`
  linkage, so every chain of the set shares one native asset. An `ETH` /
  18-decimals native token row is seeded for every chain at indexer start.
- `superchain_eth_bridge` becomes a required third contract kind per chain.
- No `op_interop_legs` rows and no `extra.op_interop` metadata for the bridge
  events; no migration, proto or settings change.

## Alternatives Considered

### Alternative 1: Build the transfer from `SendETH` / `RelayETH`

**Pros:**
- The events are the contract's own statement of the transfer.

**Cons:**
- No nonce or hash: positional pairing decides the result.
- A transfer would appear only once the event is paired, and a pairing error
  would change stored data.

### Alternative 2: Read the transaction input or a trace

**Pros:**
- Gives the user who called `sendETH`.

**Cons:**
- Needs transactions or traces, against the logs-only design of ADR-018.

## Consequences

### Positive

- The transfer needs only the log that already defines the message; it exists
  while the message is `initiated` and cannot be affected by event pairing.
- The stats projection needs no change.

### Negative

- A bridge upgrade that keeps the selector and length but changes the meaning
  of the payload would yield wrong rows; the checks only raise an alarm.
- There is no custom-gas-token guard: a member chain whose native token is
  not ETH would be labelled ETH and merged irreversibly into the shared asset.
- The seed overwrites name, symbol and decimals of an existing native row on
  every start.
- Messages stored before this change get no transfer until their source block
  is ingested again (see `gotchas.md`).
- The shapes rest on simulated, not mined, transactions.

### Neutral

- `RefundETH`, `MessageExpired` and `UndeliveredMessageExported` are not read.
- `relay_eth_missing` / `relay_eth_mismatch` are emitted from `consolidate`
  and are counted per consolidation attempt.

## References

- [ADR-018](./018-op-interop-confirmed-reading-and-leg-ledger.md)
- `research/op-interop-protocol-and-indexing-fit.md`
