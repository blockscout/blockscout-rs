# AMB / Omnibridge Token Transfer Reconstruction

## Scope

How AMB/Omnibridge `crosschain_transfers` rows get their token addresses and
amounts: the Omnibridge native/bridged token model, what the mediator handler
calldata actually carries, why the indexer relies on events instead of calldata,
and the nullable-column semantics. Out of scope: the generic AMB message
lifecycle (signatures, affirmations, collisions) — see `message-lifecycle.md`
and the AMB gotchas.

## Short Answer

A transfer's two sides are reconstructed **independently from on-chain events**:
the source side from the source-chain `TokensBridgingInitiated` event, the
destination side from the destination-chain `TokensBridged` event. Whatever
side has not been observed is left **NULL** (the four columns are nullable). The
AMB application calldata is **not** used — its token is the *native-chain* token
and is therefore source- or destination-side depending on transfer direction,
and it is unreliable when the message targets a contract that wraps the mediator.

## Why This Matters

Getting this wrong manufactured `crosschain_transfers` rows where
`token_src_address == token_dst_address` and corrupted stats volume/asset edges.
The fix (ADR-003) hinges on understanding *which* token the Omnibridge calldata
encodes — a non-obvious, direction-dependent fact verified from the contract
source.

## Source-of-Truth Files

- `interchain-indexer-logic/src/indexer/amb/consolidation.rs` — `build_transfer`
  (source-led) and `build_destination_only_transfer`; the only place transfer
  rows are constructed.
- `interchain-indexer-logic/src/indexer/amb/events.rs` —
  `find_tokens_bridging_initiated` (source) and `find_tokens_bridged`
  (destination) populate `source_transfer` / `destination_transfer` on the
  buffered message.
- `interchain-indexer-entity/src/codegen/crosschain_transfers.rs` — the four
  nullable columns.
- Config: `config/omnibridge/bridges.json` — mediator addresses + event ABIs.
- On-chain reference (Gnosis): `HomeOmnibridge` impl
  `0x2dbdCC6CAd1a5a11FD6337244407bC06162aAf92` behind proxy
  `0xf6A78083ca3e2a662D6dd1703c939c8aCE2e268d`; Ethereum mediator
  `0x88ad09518695c6c3712AC10a214bE5109a655671`.

## Key Types / Tables / Contracts

- `SourceTransferDetails { token, sender, amount }` ← `TokensBridgingInitiated`.
- `DestinationTransferDetails { token, recipient, amount }` ← `TokensBridged`.
- `crosschain_transfers` columns (all nullable): `token_src_address`,
  `token_dst_address`, `src_amount`, `dst_amount`. `sender_address` /
  `recipient_address` are also nullable.

### Native vs. bridged token (the crux)

Each token pair has a **native** token (the canonical ERC-20 on its home chain;
the mediator there locks/unlocks it) and a **bridged** representation (a
`TokenProxy` the other side mints/burns). The pairing is recorded by the
mediator's `NewTokenRegistered(nativeToken, bridgedToken)` event. "Native" is
per token pair, not per chain (e.g. USDC native on Ethereum / bridged on Gnosis;
GNO native on Gnosis / bridged on Ethereum).

The mediator handler functions, and what their `_token` argument means, on the
**receiving (destination) chain**:

| Calldata function | Operation | `_token` is the token on |
|---|---|---|
| `handleBridgedTokens*`, `deployAndHandleBridgedTokens*` | mint bridged | the **source** chain |
| `handleNativeTokens*` | unlock native | the **destination** chain |

Confirmed from `HomeOmnibridge` source: `handleBridgedTokens` doc says *"address
of the native ERC20/ERC677 token on the other side"* and calls
`bridgedTokenAddress(_token)` then mints; `handleNativeTokens` doc says *"native
ERC20 token … native to this chain"* and unlocks `_token` directly. So the
calldata token is always the token on its native chain — source-side for
lock→mint, destination-side for burn→unlock. That is why it cannot be used as a
single "source token".

## Step-by-Step Flow

1. Source chain emits `TokensBridgingInitiated`; `find_tokens_bridging_initiated`
   captures `source_transfer` into the buffered message.
2. Destination chain emits `TokensBridged`; `find_tokens_bridged` captures
   `destination_transfer`.
3. At consolidation, if either is present, `build_transfer` (source-led) or
   `build_destination_only_transfer` builds one row: each side from its own
   event, the missing side NULL. Amounts are the raw integer values from the
   events.
4. Persistence + stats projection consume the row (see Invariants).

## Invariants

- A side's columns are non-NULL **iff** its bridge event was observed.
- Source amount/token/sender ⇐ `TokensBridgingInitiated` only; destination
  amount/token/recipient ⇐ `TokensBridged` only. No cross-side fallback at write
  time.
- A transfer row exists iff at least one side's event was seen; non-token AMB
  messages have no transfer row.
- Chain-id columns (`token_src_chain_id`, `token_dst_chain_id`) remain NOT NULL.

## Failure Modes / Observability

- Destination-only completed rows (source never indexed, or a `messageId`
  collision where the source body was displaced) have NULL source side and NULL
  `sender_address`; `src_tx_hash` is NULL on the parent message.
- Stats projection skips a NULL endpoint for token enrichment/asset linking and
  uses the known side's amount for edge volume (`stats/projection.rs`,
  `transfer_amount_for_side`, `ensure_asset_for_transfer`).
- Proto layer returns `source_token`/`destination_token = None` and omits the
  amount string when the column is NULL.

## Edge Cases / Gotchas

- Pre-change mirrored rows (`token_src == token_dst`) persist until reindexed.
- Down migration backfills NULLs with a zero-address / zero-amount sentinel, not
  by mirroring, to avoid recreating the corrupt representation.
- The calldata `_value` is the source-initiated (gross, pre-fee) amount; the
  destination `TokensBridged` value is net of any destination fee — another
  reason the per-chain event amounts are preferred.

## Change Triggers

Update this note if: a new mediator version changes handler semantics or event
shapes; the transfer columns change nullability again; calldata decoding is
reintroduced; or stats endpoint/amount handling changes.

## Open Questions

- A backfill/reindex strategy for the legacy mirrored rows is not yet defined.
