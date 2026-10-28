# Relay (relay.link): Data Sources and Indexing Schemes

## Scope

How Relay orders can be ingested into interchain-indexer:

- which data sources exist: the REST API, per-chain on-chain indexing, and the
  Relay Chain;
- what each source covers and costs;
- how the data maps onto the service schema and pipeline;
- two candidate indexing schemes — one assuming API access without visibility
  restrictions, one without such access.

In scope:

- the REST API as a data source: payload by `orderId`, visibility scoping,
  query surface, rate limits, lifecycle
- on-chain indexing: what to index on origin chains, how to find fills and
  refunds on destination chains, the solver wallet set, the Relay Chain
  settlement signal
- the mapping onto the interchain-indexer schema
- the two schemes, their comparison and a recommendation

Out of scope:

- protocol mechanics — actors, identifiers and key formulas, the `Order`,
  deployments, event surface, router versions, the deposit / fill /
  settlement flow, protocol history. See `relay-protocol-model.md`; this note
  builds on it.
- an implementation plan or an ADR (no scheme is decided)
- ingestion details for non-EVM chains

Status: **pre-implementation research.** Evidence was gathered on
2026-10-26..27 from the live API (keyless and with a freshly created
self-serve API key), the relay.link web app, and on-chain reads on Ethereum,
Base, Optimism, Arbitrum, Polygon and the Relay Chain. The schemes were drafted
on 2026-10-28 from that evidence; neither has been prototyped.

## Short Answer

- **With unrestricted API visibility, ingest the API (Scheme 1).** The
  `/requests/v3` change feed returns both sides of every order, on all chains
  including non-EVM, in one record.
- **A regular API key is not that.**
  - `/requests/v3` returns only the caller's integrator's requests plus
    requests made without a key. A fresh self-serve key saw ≈13% of the global
    feed.
  - Only relay.link's own server-side proxy was observed with global
    visibility.
  - The keyless, global `/requests/v2` sunsets on 2026-11-24.
- **Without it, three on-chain streams cover EVM chains (Scheme 2):**
  - deposits from the prod Depository logs on each origin chain;
  - fills and refunds, found by reading every block of each indexed chain and
    matching the last 32 bytes of calldata against `orderId`s. Logs alone are
    not enough: 61% of sampled fills are native transfers with no logs;
  - Relay Chain Oracle `Executed` events as a per-order "settled" signal whose
    key is computable from origin data.

  Scheme 2 records the actual payout, not the order's intended output, and
  sees non-EVM destinations only as "settled".
- **Volume** is orders of magnitude above the bridges indexed today: ≈2–3
  orders/s globally.

## Why This Matters

- **It breaks the "both sides emit bridge-contract logs" assumption** that
  `LogStream` + `Consolidate` are built on. A Relay indexer cannot pair source
  and destination events by streaming logs of configured contracts on both
  chains. Pairing needs either:
  - tx-level scanning (calldata suffix match), or
  - an off-chain source keyed by `orderId`.
- **It is a converting bridge by construction.** Per ADR-011, transfers must
  carry `asset_linkage`. Relay gives no protocol-level evidence that two tokens
  are one asset, so `mirror` would be a claim the indexer cannot back.
- **Identity differs per source.** The API is keyed by `requestId`, which is
  not on chain; on-chain data is keyed by `orderId`, which most API consumers
  do not see first.
- **Non-production traffic shares mainnet chains.** Staging and dev
  Depositories are deployed at different addresses on the same chains and
  receive real deposits. Filtering by event signature alone is wrong.
- **Volume.**
  - Base alone: ≈650 `RelayNativeDeposit` + ≈670 `RelayErc20Deposit` per hour
    (2026-10-26 evening UTC).
  - The global request feed showed ≈2–3 requests/s (≈120–180/min,
    ≈175k–250k/day).

## Source-of-Truth Files

This repository (for fit):

- `interchain-indexer-entity/src/codegen/crosschain_messages.rs`,
  `crosschain_transfers.rs`
  - `id: i64`, `native_id: bytes`, nullable `dst_chain_id`
  - per-transfer `token_src_*` / `token_dst_*`, `src_amount` / `dst_amount`,
    `asset_linkage`
- `interchain-indexer-logic/src/message_buffer/types.rs`
  - `Key { message_id: i64, bridge_id }` and the `Consolidate` trait
- `interchain-indexer-logic/src/indexer/amb/events.rs`
  - `key_from_message_id`: precedent for deriving the `i64` key as
    `keccak256(native id)[..8]`
- `interchain-indexer-logic/src/indexer/evm/`
  - `log_stream_builder`, `receipt_fetch`, `transaction_grouping`: the shared
    EVM log pipeline a Relay origin indexer would reuse
- `.memory-bank/adr/011-cross-asset-edges-and-per-transfer-linkage.md`,
  `.memory-bank/adr/012-chain-local-token-types.md`

Upstream:

- `relay-protocol-model.md` — the protocol facts this note relies on
  (identifiers and key formulas, deployments, event surface, fill rules)
- API documentation
  - API guides under `references/api/` in https://docs.relay.link/llms.txt
  - OpenAPI 3.0.3 spec: https://api.relay.link/documentation/json (same file
    on `api.testnets.relay.link`); Swagger UI:
    https://api.relay.link/documentation/static/index.html
  - AsyncAPI for the WebSocket: https://docs.relay.link/websocket-reference.json
    (server `wss://ws.relay.link`, API key required)
  - https://docs.relay.link/references/api/get-requests — the visibility scope
    is stated in the parameter descriptions, not in the page subtitle
  - https://docs.relay.link/references/api/api-keys.md — default and elevated
    rate limits
- https://github.com/relayprotocol/relay-protocol-oracle —
  `src/services/attestation/vm/ethereum-vm/index.ts`, the operational
  definition of an EVM deposit and fill, which on-chain matching must mirror
- `GET https://api.relay.link/chains` (keyless) — chain slugs, depositories,
  and the public solver hot-wallet list `solverAddresses`
- Relay Chain: RPC `https://rpc.chain.relay.link`; Blockscout at
  `https://explorer.chain.relay.link` (use its Etherscan-compatible
  `module=logs&action=getLogs` for topic-filtered lookups)

## Key Types / Tables / Contracts

### Mapping onto the interchain-indexer schema

- **`native_id` = `orderId`.** It is on chain on both sides (Depository event
  or calldata on the origin, calldata suffix on the destination), and the API
  accepts it as a filter.
- **`id` (`i64`) = `keccak256(orderId)[..8]`**, the AMB way (see
  `key_from_message_id`).
- `src_chain_id` is known at deposit; `dst_chain_id` stays nullable until the
  fill is known.
- **One transfer per (input, payment) pair.** Usually one per order; do not
  assume N = 1.
- **`asset_linkage = conversion`** on every transfer; native endpoints via the
  ADR-012 zero-address sentinel.
- API records are keyed by `requestId`. Keep it next to `orderId`;
  `requestId` → `orderId` is many-to-one at most.
- History before the settlement protocol (before 2025-07-09) used short
  calldata ids that are not globally unique and do not fit this key.

### Data known on chain at deposit

| Known on chain at deposit | Not known on chain |
|---|---|
| origin chain, tx hash, depositor (`from` arg), token (post-origin-swap), amount, `orderId` | destination chain, recipient, output token, output amount, calls, deadline, refund options, solver |

Deposit-address flows reveal nothing until the sweep tx.

### Data sources compared

| Source | Complete? | Limits |
|---|---|---|
| `GET /requests/v3`, regular key | **no**: own integrator + keyless requests only (≈13% of traffic) | API key required; off-chain trust |
| `GET /requests/v3`, privileged visibility | yes (all chains, incl. non-EVM) | only relay.link's own proxy observed with it; availability to third parties unknown |
| `GET /requests/v2` | yes, keyless and global | deprecated 2026-07-22, throttled from 2026-09-01, **sunset 2026-11-24**; `limit` ≤ 50 |
| `GET /intents/status/v3` | global, but one `requestId` per call | `requestId` is not on chain |
| Relay Chain Hub/Oracle | per-order "settled" signal, all chains incl. non-EVM | no destination, fill tx, recipient or output token; fill and refund look alike; pull-based, solver-triggered |
| per-chain on-chain indexing | origin: yes (EVM); destination: yes (EVM) | native direct fills are not log-addressable: tx-level scan; non-EVM out of reach |

- An `orderId` lookup on the keyless v2 endpoint returned another
  integrator's (LI.FI's) terminal request in full: v2 is not
  integrator-scoped.
- The same lookup on v3, by `orderId` or by `id`, returned 0 rows with a
  regular key, even 16 hours after the request completed. v3 *is* scoped.

## API

### Data by `orderId`

Only `/requests/v3?orderId=` (key; subject to the visibility scope below) and
`/requests/v2?orderId=` (keyless, global, `includeOrderData=true`) accept it.
Payload:

- **Request:**
  - `id`, `status`, timestamps, `requestType`, `features`
  - `user`, `sender`, `recipient`, `refundTo`, `depositAddress{…}`
  - `duplicateDeposits`, `supersededByRequestId` (v3)
  - `failReason`, `refundFailReason` (including `MANUAL_REFUND_REQUIRED`),
    `failedTxHash`
- **Transactions:** `data.inTxs[]` / `data.outTxs[]` with tx hash, chain,
  block, timestamp, status, fee, raw `{from,to,data,value}`, and
  `stateChanges[]` (per-address token balance diffs).
- **Route:** `data.route.quoted|actual.{origin,destination}` (input/output
  currency and amounts, router, `currencyGasTopup`) and `rate`. This is where
  the original T1 before an origin swap is visible.
- **Fees:** `fees`, `appFees`, `feeSponsorship`. `platformFee`, `referrer`,
  `appId` are owner-only.
- **`protocol`:**
  - `orderId`, `solver`
  - `deposit.origin{chainId, depository, depositor, currency, amount, transactionId, onchainId}`
  - `deposit.relay{537713 tx hash, mint, hub token, metadata}`
  - `deposit.attestation` (actions, idempotency key, oracle signatures)
  - `settlement.destination.fills[] / refunds[]` (chain, tx)
  - `settlement.relay{tx hash, transfer|burn|mixed, solver}`
- **`orderData`:** the full `Order` (v2 endpoint only; omitted for public
  non-terminal requests; absent from the v3 schema). It allows recomputing and
  verifying the `orderId`.

A refund shows up as `status = refund` with the refund tx in
`settlement.destination.refunds[]`.

### Visibility rule

Measured on 2026-10-27 with a newly created self-serve App and key. Nobody had
used the key before, so its "own integrator" set was empty.

The get-requests page's subtitle says "This API returns all the cross-chain
transactions". Its parameter descriptions say otherwise:

- `destinationChainId`, `chainId`, `currencyOut*` and `filters` all carry
  "Requests are limited to the caller’s integrator and requests without an API
  key".
- `referrer` adds "Existing visibility restrictions still apply".

Observed behaviour matches the parameter notes. For a key with no requests of
its own, v3 returns only **public keyless requests**.

| Check | Result |
|---|---|
| v3 with no filter vs relay.link proxy, same 49 s window (11:41:59–11:42:48 UTC) | proxy 100 requests; our key 14; 13 in both, 87 proxy-only. The 1 key-only row was indexed after the proxy snapshot and the proxy finds it on `id` lookup |
| 100 on-chain Base Depository `orderId`s (1–10 min old), one `filters` call | 17 found |
| v3 `id=` / `orderId=` for LI.FI (≈16 h old) and OpenSea (≈1 h old) requests | 0, with or without `includeChildRequests` |
| v2 keyless `orderId=` for v3-invisible orders | found (referrers `opensea-unlinked-recipient`, `relay.link`) |
| v2 lookups of v3-visible orders | `referrer` empty (3/3 checked) |
| `/intents/status/v3?requestId=` for a v3-invisible OpenSea request | `success` with in/out tx hashes, chain ids, timestamps: **status lookups are global** |
| `/intents/status/v3?requestId=<orderId or tx hash>` | `{"status":"unknown"}`: only `requestId` is accepted |
| `/protocol/executions/{idempotencyKey}` | Relay Chain status + tx hash, `oracleAddress` `0xd180dc3b…`; by `orderId` → `unknown` |

The hidden requests are the keyed integrators (`relay.link`, `lifi`,
`opensea`, `wallet.ronin`, `funxyz`, …). An empty `referrer` is not proof of
keyless: one invisible request had none.

### relay.link web app

`https://relay.link/transactions` calls a same-origin server proxy,
`GET https://relay.link/api/relay/requests/v3`. No key is visible in the
browser; the proxy attaches privileged visibility server-side.

- It whitelists parameters:
  - accepted: none, `limit=20`, `continuation`, `id`, `term`, `depositTxHash`;
  - rejected with `{"error":"query parameter not allowed"}`: `limit=50`,
    `orderId`.
- Its feed is a superset of a regular key's.

It is an undocumented internal backend and must not be treated as a data
source.

### Query surface worth knowing (v3)

- `filters=[{"orderId":[…]}]` — groups are OR-ed, array values are OR-ed. A
  batch of 100 `orderId`s resolved in one call; the documented 50-value cap
  covers `id` / `term` only.
- `sortBy=updatedAt` + `sortDirection=asc` + `startTimestamp` +
  `continuation` — an incremental change feed.
- `limit` ≤ 100 on v3 and ≤ 50 on v2.
- Other filters: `originChainId` with `startBlock` / `endBlock`,
  `requestType` (`Bridge`, `Crosschain Swap`, `Same-chain Swap`, `Send`,
  `Wrap`, `Unwrap`, `Call`), `status`, `features`.

Push channels are not discovery channels:

- The WebSocket (`wss://ws.relay.link?apiKey=`) subscription requires
  `filters.id` = a `requestId`.
- Webhooks fire only for requests made with the App's own keys.

### Rate limits and lifecycle

- **Documented defaults per App:**
  - `/quote` 50/min, `/requests` 100/min, `/requests/v3` 200/min (its own
    bucket), others 200/min.
  - Elevated: `/requests/v3` 20 rps, `/requests` + `/requests/v2` a shared
    5 rps.
- **The newly created App was configured** at `/requests` 2/min,
  `/withdrawals/status` 120/min, others 200/min. Whether v3 falls into its
  own bucket for that App was not established; all calls were paced to
  2/min.
- No rate-limit headers were returned.
- The keyless v2 is IP-throttled and returned `429` after a handful of calls.
- **Lifecycle:** `/requests/v2` was deprecated on 2026-07-22, throttled from
  2026-09-01 and sunsets on 2026-11-24; v3 requires an API key.

### Capacity with a regular key

Two API-based scenarios were measured against the regular-key visibility:

- **Scenario 1: on-chain Depository logs, then `orderId` lookup.**
  - Mechanically viable: 100 `orderId`s per `filters` call × 2/min = 200
    lookups/min, against ≈12–22 explicit deposits/min on Base alone.
  - Coverage ≈17%. The other ≈83% need on-chain destination matching
    (Scheme 2) or privileged visibility.
- **Scenario 2: API-only tracking** via an `updatedAt` feed.
  - Visible volume ≈23 new requests/min plus status updates: about one
    100-row call/min, little headroom for history.
  - Covers only the ≈13% visible slice of ≈120–180 requests/min.

**Both scenarios become complete only if Relay grants global v3 visibility to
the App**, and need the elevated v3 limit at full volume.

## On-Chain Indexing

Measured on 2026-10-26..27 from the keyless `/chains`, 186 fills reported by
the API across several destination chains, 301 consecutive Base blocks, and
the Relay Chain. The `/chains` call is a keyless registry read; nothing in
this section depends on `/requests`.

### What to index

- **Origin, per EVM chain: the prod Depository** (`0x4cd0…bc31` / `0x5991…`
  / chain-specific):
  - `RelayNativeDeposit`, `RelayErc20Deposit`;
  - plus `Transfer(topic2 = Depository)` from any token, with tx calldata for
    implicit deposits. This needs a topic-only `Transfer` filter and tx-input
    reads; whether the `Block` fetched by `receipt_fetch` exposes tx input is
    not verified.
- **Destination:** transactions whose calldata ends with an `orderId`. They
  are sent by solver hot wallets — `/chains` `solverAddresses`, 42 distinct
  EVM addresses, open-ended — directly or through router, ApprovalProxy or
  Depository.
- **Legacy (pre-2025-07):**
  - `RelayReceiver` (`FundsForwardedWithData`, per-chain addresses in
    `/chains` `contracts.relayReceiver`, e.g. `0xa5f56565…9934`);
  - plain transfers to solver EOAs (`0x81f91aca…` in Jan 2024, later
    `0xf70da978…`) with short ids.
  - Zero `FundsForwardedWithData` on Base in the last hour of 2026-10-26: the
    path is dead now.

### Solver identity and hot wallets

- **One protocol solver identity.** Every sampled order names
  `solver = 0xf70da978…dbef`, `solverChainId = base`. This address receives
  Depository withdrawals and, on Base, is the token source of ERC-20 fills.
- **`/chains` `solverAddresses`** lists the hot wallets per chain:

  | Chains | Listed addresses |
  |---|---|
  | 43 of 50 EVM chains | only `0xf70da978…` |
  | Base | 41 |
  | Robinhood | 31 |
  | Arc | 16 |
  | BSC | 13 |
  | Polygon | 12 |
  | Ethereum | 10 |
  | HyperEVM | 7 |
  | Hyperliquid | 3 |
  | Solana, Eclipse | 2 each |
  | Bitcoin, Tron, TON, XRP, Lighter, Robinhood-Lighter | 1 each |

- 42 distinct EVM addresses in total, the same wallets recurring across the
  multi-wallet chains. The list was unchanged between the two days, and all
  41 Base wallets sent transactions within 10 minutes.
- **Coverage.** All 186 sampled fills were sent by an address listed for their
  chain, and all 186 had calldata ending with the `orderId`.
- The list is a hint, not a constraint: the Oracle never checks `tx.from`.

The set can also be learned on chain:

- **Main wallet:** sender of Depository withdrawals and `from` of the
  `transferFrom` in ERC-20 fills.
- **Hot wallets:** `tx.from` of `transferFrom(0xf70da978…, recipient, amount)`
  calls carrying a 32-byte suffix. On Base this found 39 of 41 wallets in
  10 minutes, and 11 of the 12 native-fill senders were among them.
- **Self-learning:** any tx whose suffix matches an indexed `orderId` reveals
  its sender.
- **Not a source:** `RelayCallExecuted` recipients. Of 163 distinct recipients
  over ≈2 h on Base, only the main wallet is a solver; the rest are users paid
  by withdraw-and-fill.

### Solver traffic on one chain

Base, 301 blocks from 53157537 (≈10 min): 108,667 txs, of which 241 (0.22%,
≈0.4/s) came from 41 listed wallets.

| Shape | Count | Meaning |
|---|---|---|
| hot wallet → `token.transferFrom(main, recipient, amt)` + suffix | 96 | ERC-20 fill |
| `Depository.execute` (`0x2d9fb478`) | 41 (35 hot, 6 main) | from a hot wallet: withdraw-and-fill (`RelayCallExecuted`, recipient = user); from main: withdrawal |
| Multicall3 `aggregate3Value` (`0x174dea71`) | 27 | includes solver-submitted gasless **origin deposits** with a `requestId` suffix, not an `orderId` (e.g. MetaMask Pay Base → Hyperliquid, `0x0b07653b…`) |
| input is exactly 32 bytes | 23 | native direct fill, zero logs |
| RelayRouter v3.2 `multicall` (`0xcd6e13f7`) | 20 | fill with swap or calls |
| ApprovalProxy v3.2 (`0x0a2b8f36`) | 14 | proxy-routed action, not classified further |
| main → `0x1df43ab5…`, `0xd80c6c1b` | 15 | solver operations, not fills |
| other | 5 | — |

Across the 186-fill cross-chain sample: 61% native direct, 24% ERC-20 direct,
15% through Relay contracts.

### Finding fills on a destination chain

Per EVM chain, given the pending `orderId`s from the origin indexer. Steps 1
and 2 are alternatives to a full-block read; Scheme 2 reads full blocks, which
covers both.

1. **Log-addressable fills.** For each hit, fetch the tx input and match its
   last 32 bytes against pending `orderId`s.
   - `Transfer` with `topic1` = main solver, any token: the hot-wallet
     `transferFrom` ERC-20 fills (all ERC-20 fills seen on Base). Recipient,
     token and amount come from the log.
   - Router and ApprovalProxy events of every live version:
     `SolverNativeTransfer`, `SolverCallExecuted`, `FundsMovement`.
   - Depository `RelayCallExecuted` whose call recipient is not the main
     wallet: withdraw-and-fill (verified `0x5333c76a…`, Robinhood → Base).
2. **Native direct fills (no logs).** A tx from a listed wallet whose input is
   exactly the `orderId` and whose value is the payout. Options:
   - full blocks filtered by `from` (Base: read ≈180 tx/s to keep ≈0.4/s);
   - `trace_filter(fromAddress=…)` on Erigon/Reth archive nodes (untested);
   - a Blockscout per-address tx index for the wallet set.

   Skipping this step loses most fills (61% of the sample).
3. **Classify every match:**
   - `orderId` suffix on a chain other than the deposit's → fill;
   - `orderId` suffix on the deposit's chain, paying the depositor → refund;
   - `requestId` suffix (solver Multicall3) → an origin deposit the solver
     submitted, not a fill;
   - `Depository.execute` from main with no id → solver withdrawal.
4. **Accept destination-first arrival.** A fill can land before the deposit:
   in the Polygon → Arbitrum example, the fill came 1 s before the deposit
   block.

### Relay Chain settlement as a completion signal

Per order, the Oracle contract (`0xd180dc3b…`) emits
`Executed(bytes32 indexed idempotencyKey, bytes[] actions)`:

- **Fill actions:** `TRANSFER(order address → solver hub alias, hub token,
  amount)` plus one TRANSFER per fee. The hub token and amount are the
  **deposit's** (origin chain + currency), not the payout's.
- **The keys are computable from origin data alone** (formulas in
  `relay-protocol-model.md`, *Identifiers*):
  - MINT = `gdi(slug, depositTx)`;
  - fill or refund = `gdi(gdi(sorted onchainIds…), orderId)`.

  Multi-input orders need every deposit of the order.

Verified on two orders:

| Order | Deposit | Fill | Relay Chain tx | Lag |
|---|---|---|---|---|
| Ethereum USDT → Arbitrum USDT, `0x5966cc21…` | `0x02699d49…` (`Transfer` log 255) | Arbitrum `0x59db66d6…`, ERC-20 from main | `0xe73983e7…`, block 6034886: MINT `0x94be9adf…`, TRANSFER `0x612a58d2…` | 57 s after the fill |
| Polygon USDC → Arbitrum ETH, `0x3f6a36c3…` | `0x40e972e3…` (`Transfer` log 1381) | Arbitrum `0x978a8b5a…`, native, 0 logs | `0xc400eb4f…`, block 6056017: MINT `0x01539aa8…`, TRANSFER `0xa973c631…` | 5 s after the deposit |

Neither Relay Chain tx contains the fill tx hash, the `orderId`, the
recipient or the destination chain id, in calldata or in logs. Both credit
the same solver hub alias, `0xd5fa4f58…`.

What the signal is good for:

- marking an order as settled, and bounding how long to keep searching for
  its fill;
- registering orders filled on unindexed or non-EVM chains as "settled,
  destination unknown".

Limits:

- **Fill and refund look alike.** Same key formula, same order → solver
  TRANSFER. Only fee TRANSFERs mark a fill, and fee-less fills exist (the
  Ethereum example had a single action).
- **Not strict proof.** The Oracle API's `force` flag, gated to operator API
  keys, signs a fill or refund without verifying it.
- **Volume:** ≈5 `Executed`/s, ≈20 per tx (2,933 in 147 txs over 200 blocks;
  block time ≈2.86 s).
- **Query path:** the public RPC's `eth_getLogs` filtered by `topic1` over
  3,000 blocks took over 5 minutes and returned nothing for a MINT key that
  is present in the tx receipt. The Blockscout `getLogs` answered at once and
  completely. The public RPC was also slow on short address-filtered ranges.

### What stays unknown without the API

- the destination chain, until the fill is found;
- the intended output token, recipient and `minimumAmount`. Only the actual
  payout is recorded; under- or over-payment cannot be judged;
- the original T1 of origin swaps, except through `FundsMovement` /
  `Transfer` logs of the deposit tx;
- fills on non-EVM chains (Relay Chain signal only);
- wallet-set completeness: a wallet added and used between refreshes is
  missed for destination-first fills until it is learned.

## Indexing Schemes

Drafted 2026-10-28. Neither scheme is decided or prototyped.

### Common to both

- A new bridge type, mapped as in *Mapping onto the interchain-indexer
  schema*: `native_id` = `orderId`, `id` = `keccak256(orderId)[..8]`,
  `asset_linkage = conversion`, one transfer per (input, payment) pair.
- Only the prod Depositories count; staging and dev Depositories share the
  mainnets.
- Chain slugs and Relay numeric ids map to chain ids through `/chains`. The
  `chains.json` model is EVM- and RPC-centric, so non-EVM chains need a
  mapping layer or stay "unknown chain" under the existing filtering rules.
- Volume (≈2–3 orders/s) makes buffer sizing, the stats-projection batch
  limits (see `stats-projection-unbatched-pks-lookup-crash.md`) and the
  PostgreSQL bind limit first-order concerns.

### Scheme 1: API with unrestricted visibility

Assumes an App with global `/requests/v3` visibility and an elevated limit.

1. **Poll the change feed:** `/requests/v3` with `sortBy=updatedAt`,
   `sortDirection=asc`, `startTimestamp`, `limit=100`, `continuation`. The
   cursor (`updatedAt` + `continuation`) needs its own checkpoint; the
   existing checkpoints are block ranges per chain.
2. **Filter:** `protocol.orderId` present, origin chain ≠ destination chain,
   both chains configured (or handled by the unknown-chain rules).
3. **Map each request to a whole message**, written by upsert:
   - source: `inTxs[]` and `protocol.deposit.origin` (depositor, token,
     amount, tx);
   - destination: `outTxs[]` and `protocol.settlement.destination.fills[]`;
     recipient, output token and amount from the request metadata and
     `stateChanges` (field names seen in the v2 payload; to be confirmed on
     v3);
   - original T1 before an origin swap: `route.origin.inputCurrency`;
   - status: `success` → completed; `refund` → refunded, with the tx from
     `settlement.destination.refunds[]`; `failure` → failed; the in-flight
     values → initiated.
4. **Updates** of the same request overwrite the row. The buffer's
   source/destination pairing is not needed: both sides arrive in one record.
5. **History** through the same feed from the chosen start. At today's
   ≈175k–250k requests/day, a year is at most ≈65–90M rows: ≈2–3 days at the
   default 200 calls/min × 100 rows, ≈9–13 h at 20 rps. Live traffic needs
   ≈5 calls/min.
6. **Optional, later:** spot-check EVM chains in the config on chain — the
   deposit and fill receipts, and the `orderId` in the event or calldata
   suffix. About 2 RPC calls per order.

- **Closes:**
  - all 59 chains, including non-EVM;
  - full destination data and the intended output of the order;
  - refunds and the original T1;
  - history from 2024-01.
- **Does not close:**
  - trust: the data is off chain and Relay-operated;
  - the dependency on API availability and format (v2 is already being
    retired);
  - visibility and limits stay an agreement with Relay;
  - a new ingestion path for this service: a poller with its own cursor, not
    a `LogStream`.

### Scheme 2: on chain only

Uses only chain data and the keyless `/chains` registry.

1. **Deposits, per EVM origin chain.**
   - A `LogStream` on the prod Depository: `RelayNativeDeposit`,
     `RelayErc20Deposit`, plus `Transfer(topic2 = Depository)` with tx input
     for implicit deposits.
   - Yields `orderId`, depositor, token, amount and tx.
   - The Relay Chain keys (MINT and fill/refund) are computed here.
2. **Fills and refunds, per EVM chain.** Read every block with its
   transactions (`eth_getBlockByNumber` with full txs) and take the last 32
   bytes of each tx input. A tx is a candidate when:
   - the suffix matches a pending `orderId`; or
   - the sender is in the solver wallet set, even if no deposit is known yet
     (a fill can precede its deposit).

   For each candidate, fetch the receipt and read amount and recipient:
   - native direct: `value` and `to`;
   - ERC-20: the token `Transfer`;
   - through the Router: `SolverNativeTransfer` / `FundsMovement`.

   A match on the deposit's own chain that pays the depositor is a refund.

   This one stream covers every fill shape, so the log paths of *Finding
   fills on a destination chain* are not needed. It is a new component: the
   existing EVM pipeline streams logs, not block bodies. Cost: one call per
   block (Base ≈0.5/s carrying ≈180 tx/s; Arbitrum ≈4 blocks/s), plus
   receipts for candidates only (≈0.4/s on Base). For fast chains,
   `trace_filter` by sender set is an alternative (untested).
3. **Settlement, on the Relay Chain (537713).** A `LogStream` on the Oracle
   `0xd180…`, event `Executed`, key in `topic1`; ≈5 events/s.
   - When a fill was found, it confirms it.
   - When no fill or refund was found on the indexed chains (non-EVM or
     unindexed destination), it marks the order "settled, destination
     unknown". It cannot tell a fill from a refund.
4. **Pairing** in the existing buffer (`Consolidate`), keyed by `orderId`.
   - Deposit, fill or refund, and settlement arrive in any order.
   - Wallet-set candidates with no matching deposit expire by TTL. These are
     mostly gasless deposits with a `requestId` suffix and withdrawals.
   - Statuses: deposit → initiated; fill → completed; refund → refunded;
     settlement alone → terminal with unknown outcome. The current status set
     has no such value (see *Open Questions*).

The wallet set comes from `/chains` and grows with the senders of matched
fills. It matters only for destination-first arrivals; a suffix match on a
pending `orderId` does not depend on the sender.

- **Closes:**
  - verifiability: everything is checkable on chain;
  - independence from the API;
  - refunds;
  - non-EVM destinations partly, as "settled, destination unknown".
- **Does not close:**
  - the intended recipient, token and `minimumAmount`: only the actual payout
    is recorded, and the recipient is inferred;
  - the original T1, except through `FundsMovement`;
  - fills on non-EVM chains;
  - deposits from non-EVM chains: their EVM fills are visible but have no
    source;
  - history before 2025-07-09, when the Depositories appeared;
  - RPC cost: every block of ~50 chains, from an RPC with full block and
    receipt history for backfill;
  - a fill from an unknown wallet that lands before its deposit (0 cases in
    the sample).

### Rejected variant: a regular key as enrichment

Scheme 2 plus `orderId` lookups with a regular key (earlier "hybrid"
option) was considered and dropped:

- it covers ≈13–17% of orders and no class of orders completely;
- it adds the key and rate-limit dependency of Scheme 1 without its
  completeness.

### Comparison

| | Scheme 1 (API) | Scheme 2 (on chain) |
|---|---|---|
| Chains | all 59, incl. non-EVM | configured EVM chains; non-EVM only as the settlement signal |
| Destination data | chain, tx, recipient, token, amount | actual payout; recipient inferred |
| Intended output of the order | yes | no |
| Trust | off chain, Relay | verifiable on chain |
| History | from 2024-01 | from 2025-07-09 |
| Load | ≈5 calls/min live | every block of every chain + candidate receipts + one Relay Chain stream |
| Architecture | new path: API poller with its own cursor | 2 `LogStream`s, a new block scanner, the buffer as is |
| Dependencies | key, visibility, limits, API format | RPC (full history for backfill), `/chains` |

### Recommendation

- **If Relay grants global visibility:** Scheme 1. It is cheaper and more
  complete. Add the on-chain spot-check only when it is needed.
- **If not:** Scheme 2 with the three streams above. A regular key is not
  worth adding (see *Rejected variant*).

## Invariants

- The only on-chain link between a deposit and its fill or refund is the
  `orderId` calldata suffix (see `relay-protocol-model.md`, *Invariants*).
  Any on-chain scheme matches on it.
- Scheme 2 is complete only if every block of every indexed chain is read.
  A skipped block loses its fills, and nothing else on chain records them
  except the Relay Chain signal.
- Relay Chain keys are deterministic functions of origin data and the
  `orderId`, so settlement can be joined without the API.
- API records are keyed by `requestId`, on-chain records by `orderId`. A
  design that mixes sources must keep both.

## Failure Modes / Observability

- **Non-production deposits are indexed** if filtering by event signature
  instead of the prod Depository address (staging on Base: ≈17/day).
- **Fills are missed** if only router events are watched. Direct native and
  ERC-20 fills emit nothing Relay-specific.
- **Destination-first fills are missed** when a new solver wallet starts
  sending before the wallet set is refreshed or learned.
- **Relay Chain lookups silently miss** when made through the public RPC's
  topic-filtered `eth_getLogs`. Query the Blockscout instance instead.
- **Origin token is misreported** for origin-swap routes (Depository event
  shows USDC, the user sent T1).
- **History is lost** after 2026-11-24 if relying on keyless v2.
- **Silent partial coverage.** With a regular key, v3 answers `200` with an
  empty or short list for requests it hides. A lookup miss is not evidence
  that a request does not exist.
- **Status drift:** the API flips `inTxs[].status` to `failure` on reorged or
  canonically failed deposits (Base, since 2026-06-10).

## Edge Cases / Gotchas

- `user` in the API can be an aggregator contract (LI.FI diamond `0x1231deb6…`)
  while the Depository's `from` is the end user.
- Not every solver tx with a 32-byte suffix is a fill. Solver-submitted
  gasless origin deposits carry a `requestId` suffix.
- ERC-20 fill shape varies: on Base hot wallets pull from the main wallet via
  `transferFrom`; on Arbitrum the main wallet paid with a direct `transfer`.
  This matters for the log-based paths, not for a full-block read.
- A fill can land before its deposit (Polygon → Arbitrum example: 1 s
  earlier).
- **Duplicate deposits may share an `orderId`.** The Oracle includes the
  deposit set in the fill/refund key "to support duplicate deposit fills /
  refunds", and the API reports duplicates as linked child requests. A
  message key from `orderId` alone would merge them. Not observed on chain.

## Change Triggers

Update this note when any of these happen:

- a Relay indexer is implemented (record the chosen scheme, link an ADR, mark
  the other as superseded)
- API changes: v2 sunset, v3 auth or scoping, `orderData` in v3
- Relay grants (or refuses) global v3 visibility or elevated limits to the
  App
- `/chains` `solverAddresses` changes, a second solver identity appears in
  orders, or a fill is seen from an unlisted sender
- the Oracle contract is replaced, or `getDeterministicId` / the
  idempotency-key formulas change

## Open Questions

- Will Relay grant a third-party App the global v3 visibility that the
  relay.link proxy has, and at which rate limit? The answer picks the scheme.
  (Answered so far: v3 does **not** return other integrators' requests to a
  regular key.)
- Which bucket `/requests/v3` uses for an App whose `/requests` limit is
  2/min.
- Scheme 1: the v3 field names for recipient, output token and amount, and
  the full list of in-flight `status` values.
- Scheme 2: which status a message gets when the Relay Chain shows it settled
  but no fill or refund was found on indexed chains.
- Duplicate deposits: do they share the `orderId`, and should the message
  key be `orderId` or `(orderId, deposit onchainId)`?
- Whether the `Block` fetched by `receipt_fetch` exposes tx input (needed for
  implicit deposits).
- Share of implicit ERC-20 deposits (public RPCs rejected the topic-only range
  query used to measure it).
- Whether ERC-20 fills pull from the main wallet on chains other than Base
  (Arbitrum showed a direct `transfer` from main). This decides whether the
  `Transfer(topic1 = main)` filter is enough for a log-based design.
- Which RPC providers for the target chains expose `trace_filter`, and its
  cost relative to full-block scanning.
- A reliable on-chain way to tell a fill from a refund on the Relay Chain when
  the order has no fees.
