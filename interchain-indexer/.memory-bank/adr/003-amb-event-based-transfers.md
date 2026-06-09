# ADR-003: AMB Transfers Reconstructed From Events; Nullable Transfer Sides

**Date:** 2026-06-09

**Authors:** @EvgenKor

## Context

AMB/Omnibridge transfer rows were reconstructed partly from the AMB **application
calldata** (the mediator function call carried inside the message), decoded by a
dedicated `payload_processor` subsystem. The `crosschain_transfers` columns
`token_src_address`, `token_dst_address`, `src_amount`, `dst_amount` were
`NOT NULL`, so when a side could not be reconstructed the indexer mirrored the
known side into the unknown one.

Two problems surfaced:

1. **The calldata token is directionally ambiguous.** In Omnibridge the
   `_token` argument is always the token on its *native* chain. For
   `handleBridgedTokens*` / `deployAndHandleBridgedTokens*` that is the **source**
   token; for `handleNativeTokens*` it is the **destination** token. Labeling it
   unconditionally as the source token was wrong for the native-tokens direction.
   (Verified against the on-chain `HomeOmnibridge` source — see the research note.)
2. **The calldata is fragile to indirection.** An AMB message can target a
   wrapper contract that internally calls the mediator; then the executor does
   not match the configured mediator and/or the calldata is not a mediator
   function, so decoding produces nothing or a bogus result.

The mirroring forced by `NOT NULL` then produced `crosschain_transfers` rows
where `token_src_address == token_dst_address` (and equal amounts), which also
corrupted stats projection (volume/asset edges counted a placeholder token).

## Decision

Reconstruct AMB transfers **purely from on-chain bridge events** and make the
four transfer columns **nullable**:

- Source side (`token_src_address`, `src_amount`, `sender_address`) comes only
  from the source-chain `TokensBridgingInitiated` event (`source_transfer`).
- Destination side (`token_dst_address`, `dst_amount`, `recipient_address`)
  comes only from the destination-chain `TokensBridged` event
  (`destination_transfer`).
- A side whose event has not been observed is left **NULL** — never mirrored.
- A transfer row exists once *either* event is seen; a pure (non-token) AMB
  message produces no transfer row.

Implementation:
- Migration `m20260508_082944_add_amb_indexer` drops `NOT NULL` on the four
  columns; the down migration backfills NULLs with a zero-address / zero-amount
  sentinel (not by mirroring) before re-adding `NOT NULL`.
- Removed the calldata-decode subsystem entirely: `payload_processor.rs`, the
  `DecodedPayload` type and `decoded_payload` buffer field, the function-decoding
  in `abi.rs` (`function_for_selector`/`functions_by_selector`), and the mediator
  `functions` grammar in `version.rs`. Event parsing is unchanged.
- Readers updated for `Option`: `JoinedTransfer`, proto mapping, buffer
  enrichment, the Avalanche writer, and stats projection (skips NULL endpoints;
  amount falls back to the known side).

Scope boundary: go-forward only. Pre-existing mirrored rows persist until
reindexed.

## Alternatives Considered

### Keep calldata decoding as a fallback

**Cons:** retains the directional-ambiguity bug and the wrapper-contract
fragility; calldata amount (gross, pre-fee) is also less accurate than the event
amounts. The events already supersede it on every happy path.

### Keep `NOT NULL` and mirror the unknown side

**Cons:** this is the behavior being removed — it manufactures
`token_src == token_dst` rows and corrupts stats. Rejected for new data; only
used as the lossy down-migration sentinel, and there with a neutral zero value
rather than a mirrored token.

### Only persist a transfer once both sides are known

**Cons:** pending transfers would be invisible, and genuinely one-sided cases
(destination-only collisions, source never indexed) would yield no transfer row
at all. Nullable sides preserve visibility without fabricating data.

## Consequences

### Positive

- `token_src_address == token_dst_address` is now meaningful, not a placeholder.
- Transfer data is event-derived and direction-correct; robust to mediator
  indirection.
- Smaller AMB surface: the entire calldata-decode/ABI-function path is gone.

### Negative

- All readers of the four columns must handle `Option`/NULL.
- Existing mirrored rows remain until a reindex/backfill (separate task).

### Neutral

- The Avalanche/ICTT writer always has both sides, so it simply wraps values in
  `Some(...)`; no behavioral change there.

## References

- `interchain-indexer-logic/src/indexer/amb/consolidation.rs` (`build_transfer`)
- `research/amb-omnibridge-token-reconstruction.md`
- Gotchas: *AMB Transfer Sides Are Nullable and Never Mirrored*,
  *AMB Source and Destination Events Can Arrive Out of Order*
