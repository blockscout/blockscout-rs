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
- growing the set of indexed chains without rescanning those already indexed

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
- **Without it, two on-chain streams cover EVM chains (Scheme 2):**
  - deposits from the prod Depository logs on each origin chain;
  - fills and refunds, found by reading every block of each indexed chain and
    matching the last 32 bytes of calldata against `orderId`s. Logs alone are
    not enough: 61% of sampled fills are native transfers with no logs.

  Scheme 2 records the actual payout, not the order's intended output. Orders
  filled outside the indexed chains stay "destination unknown"; ≈7–15% of
  sampled cross-chain orders went to non-EVM chains.
- **The Relay Chain is optional in Scheme 2.** Its `Executed` events can mark
  those orders as "settled", but say nothing about where, how, or whether by
  fill or refund. Everything else it offers is already on chain or does not
  work (see *Is the Relay Chain stream needed in Scheme 2?*).
- **The chain set can grow without rescans** if Scheme 2 keeps fill
  candidates in a durable table, recognised by tx shape rather than by known
  `orderId`s, and pairs them with deposits by a join on `orderId` (see
  *Extending the chain set without rescanning*).
- **Volume** is orders of magnitude above the bridges indexed today: ≈2.2
  cross-chain solver transfers/s globally, 1.6–3.0/s across sampled windows
  (see *Transfer intensity*).

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
  - The global request feed showed ≈2.5 requests/s, of which ≈2.2/s are
    cross-chain orders (≈190k/day; see *Transfer intensity*).

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

### Transfer intensity

Measured on 2026-10-28 over windows from 2026-10-24 to 2026-10-28.
Settlement on the Relay Chain is not counted: these
are solver transfers only. A cross-chain order = one Depository deposit plus
one solver fill or refund.

**Global feed.** Six 3-minute windows of the keyless, global
`/requests/v2`: five 6 hours apart over the last day, plus one four days
earlier (1,080 s, 2,736 requests):

| Window end (UTC) | All requests | Cross-chain orders | Same-chain orders (solver-filled) | No `orderId` |
|---|---|---|---|---|
| 10-24 11:34 (Sat) | 3.31/s | **3.03/s** | 0.09/s | 0.19/s |
| 10-27 11:34 | 2.61/s | **2.19/s** | 0.10/s | 0.32/s |
| 10-27 17:34 | 3.17/s | **2.78/s** | 0.11/s | 0.28/s |
| 10-27 23:34 | 1.93/s | **1.69/s** | 0.07/s | 0.17/s |
| 10-28 05:34 | 1.80/s | **1.56/s** | 0.05/s | 0.19/s |
| 10-28 11:04 | 2.38/s | **2.17/s** | 0.07/s | 0.14/s |
| **all windows** | 2.53/s | **2.24/s** | 0.08/s | 0.22/s |

- **≈2.2 cross-chain solver transfers per second** on average, ≈190k per day.
  Across windows: from ≈1.6/s at night UTC to ≈2.8/s in the US afternoon,
  and 3.0/s in one Saturday window. Day-to-day variation is not
  characterised by a single window.
- The "no `orderId`" column is self-executed same-chain swaps and `/execute`
  relays: not solver transfers. No cross-chain request without an `orderId`
  was seen.
- **Destination split of cross-chain orders:**

  | Destination | Share |
  |---|---|
  | Solana | 25% |
  | Robinhood | 17% |
  | BSC | 16% |
  | Base | 11% |
  | Ethereum | 10% |
  | Polygon | 7% |
  | Arbitrum | 4% |
  | Hyperliquid | 4% |
  | Tron, Monad | 1% each |

  About 30% go to non-EVM chains.

**On-chain cross-check**, without the API:

- **Solver fills** — fill-shaped txs from the `/chains` wallets, windows of
  1.5–5 minutes:

  | Chain | Fills/s |
  |---|---|
  | Ethereum | 0.37 |
  | BSC | 0.25 |
  | Polygon | 0.21 |
  | Base | 0.16 |
  | Arbitrum | 0.05 |
  | **sum of five chains** | **≈1.0** |

  These five destinations take 48% of cross-chain orders. At the 2.17/s of
  the 10-28 11:04 window, close to when the fills were counted, the feed
  predicts ≈1.04/s. The rest is refunds and same-chain orders.
- **Explicit Depository deposits**, last hour:

  | Chain | Deposits/s |
  |---|---|
  | BSC | 0.29 |
  | Ethereum | 0.24 |
  | Base | 0.20 |
  | Polygon | 0.19 |
  | Arbitrum | 0.08 |
  | Optimism | 0.01 |
  | **sum of six chains** | **≈1.0** |

  Implicit deposits (a bare `Transfer` to the Depository) are not counted, so
  this is a lower bound.
- **Non-fill solver traffic.** Solver wallets send another ≈15–20% of txs on
  top of fills: Depository withdrawals and `/execute` relays.

For an indexer, ≈2 orders/s means ≈4 relevant events/s across all chains (a
deposit on the origin, a fill on the destination). Indexing EVM chains only
cuts that by about a third.

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
| Multicall3 `aggregate3Value` (`0x174dea71`) | 27 | Relay's `/execute` relayer, `requestId` suffix rather than `orderId`: gasless **origin deposits** (e.g. MetaMask Pay Base → Hyperliquid, `0x0b07653b…`) and standalone same-chain calls (e.g. an EIP-7702 smart-wallet upgrade; see *Request kinds* in `relay-protocol-model.md`) |
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
     `SolverNativeTransfer`, `SolverCallExecuted`, `FundsMovement`. Same-chain
     swaps emit them too, so only the suffix match decides.
   - Depository `RelayCallExecuted` whose call recipient is not the main
     wallet: withdraw-and-fill (verified `0x5333c76a…`, Robinhood → Base).
2. **Native direct fills (no logs).** A tx from a listed wallet whose input is
   exactly the `orderId` and whose value is the payout. Options:
   - full blocks filtered by `from` (Base: read ≈180 tx/s to keep ≈0.4/s);
   - `trace_filter(fromAddress=…)` on Erigon/Reth archive nodes (untested);
   - a Blockscout per-address tx index for the wallet set.

   Skipping this step loses most fills (61% of the sample).
3. **Classify every match:**
   - `orderId` suffix on a chain other than the deposit's → fill, or a
     destination-chain refund, which looks the same on chain;
   - `orderId` suffix on the deposit's chain returning the deposit currency to
     the depositor → refund;
   - `orderId` suffix on the deposit's chain, any other payout → fill of a
     solver-filled same-chain order, which is out of scope;
   - `requestId` suffix → not a fill: a relayed `/execute` call from a
     solver wallet (an origin deposit or a standalone same-chain action), or a
     user's same-chain swap through ApprovalProxy / Router;
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

For Scheme 2 only the second use is unique, and it is weighed against its
cost in *Is the Relay Chain stream needed in Scheme 2?*.

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
- whether a payout on the destination chain is a fill or a destination-chain
  refund. Both carry the `orderId` suffix and pay the recipient there;
- the intended output token, recipient and `minimumAmount`. Only the actual
  payout is recorded; under- or over-payment cannot be judged;
- the original T1 of origin swaps, except through `FundsMovement` /
  `Transfer` logs of the deposit tx;
- fills on non-EVM chains (Relay Chain signal only);
- wallet-set completeness: a wallet added and used between refreshes is
  missed for destination-first fills until it is learned, unless the fill's
  shape alone makes it a candidate (see *Extending the chain set without
  rescanning*).

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
- Volume (≈2 orders/s, ≈4 relevant on-chain events/s) makes buffer sizing, the stats-projection batch
  limits (see `stats-projection-unbatched-pks-lookup-crash.md`) and the
  PostgreSQL bind limit first-order concerns.

### Scheme 1: API with unrestricted visibility

Assumes an App with global `/requests/v3` visibility and an elevated limit.

1. **Poll the change feed:** `/requests/v3` with `sortBy=updatedAt`,
   `sortDirection=asc`, `startTimestamp`, `limit=100`, `continuation`. The
   cursor (`updatedAt` + `continuation`) needs its own checkpoint; the
   existing checkpoints are block ranges per chain.
2. **Filter:** `protocol.orderId` present, origin chain ≠ destination chain.
   This drops same-chain swaps and relayed `/execute` calls, which have
   `protocol = null`. It also drops solver-filled same-chain swaps, which do
   have an `orderId`, so both conditions are needed.
   Store requests for every chain and apply the configured-chain rules only
   when exposing data, so adding a chain later needs no API re-read.
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
   ≈220k requests/day (all kinds), a year is at most ≈80M rows: ≈2.8 days at
   the default 200 calls/min × 100 rows, ≈11 h at 20 rps. Live traffic needs
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
2. **Fills and refunds, per EVM chain.** Read every block with its
   transactions (`eth_getBlockByNumber` with full txs) and take the last 32
   bytes of each tx input. A tx is a candidate when, whatever the set of
   indexed chains:
   - the suffix matches a pending `orderId`; or
   - it has a fill shape, from any sender: input exactly 32 bytes with
     `value > 0` (native direct), `transfer` / `transferFrom` with 32 extra
     bytes, or a call to a Relay contract (Router, ApprovalProxy,
     Depository); or
   - the sender is in the solver wallet set.

   The second and third rules catch fills whose deposit is not known yet: a
   fill can precede its deposit, or its deposit is on a chain not indexed yet.
   The rules were measured on Base (see *Extending the chain set without
   rescanning*).

   For each candidate matched to a deposit, fetch the receipt and read amount
   and recipient:
   - native direct: `value` and `to`;
   - ERC-20: the token `Transfer`;
   - through the Router: `SolverNativeTransfer` / `FundsMovement`.

   A match on the deposit's own chain that returns the deposit currency to the
   depositor is a refund. Any other payout there fills a solver-filled
   same-chain order.

   This one stream covers every fill shape, so the log paths of *Finding
   fills on a destination chain* are not needed. It is a new component: the
   existing EVM pipeline streams logs, not block bodies. Cost: one call per
   block (Base ≈0.5/s carrying ≈180 tx/s; Arbitrum ≈4 blocks/s), plus
   receipts for matched candidates only (fewer than ≈0.4/s on Base). For fast
   chains, `trace_filter` by sender set is an alternative (untested); it sees
   only the wallet-set rule.
3. **Pairing** in the existing buffer (`Consolidate`), keyed by `orderId`.
   - Deposit and fill or refund arrive in either order.
   - Candidates with no matching deposit are not dropped at the TTL: they
     move to a durable candidate table (see *Extending the chain set without
     rescanning*). Most never match: gasless deposits with a `requestId`
     suffix, withdrawals, user calls to Relay contracts, and fills of orders
     from unindexed or non-EVM chains.
   - Statuses: deposit → initiated; fill → completed; refund → refunded.
   - A solver-filled same-chain order looks like any other deposit until its
     fill shows up on the same chain. Then it is dropped or hidden, like
     self-executed same-chain swaps.
   - A deposit with no fill or refund found within the TTL is persisted as
     initiated with an unknown destination. That is the honest state: the
     deposit is known, the outcome is not. ADR-010 (unresolved Avalanche
     destinations) is the precedent for keeping such sends.
   - A fill found after the TTL must still update the stored message.
     `amb-destination-only-missing-source-gap.md` shows what happens when a
     late event cannot be merged into an already finalized entry.

An optional fourth stream, the Relay Chain Oracle `Executed` events, is
discussed in *Is the Relay Chain stream needed in Scheme 2?*. It is not part
of the base scheme.

The wallet set comes from `/chains` and grows with the senders of matched
fills. With the shape rules it is a safety net for fills of an unexpected
shape from known wallets. A suffix match on a pending `orderId` does not
depend on the sender.

- **Closes:**
  - verifiability: everything is checkable on chain;
  - independence from the API;
  - refunds to the origin chain, which is always indexed when the deposit is;
  - adding chains later without rescanning the ones already indexed.
- **Does not close:**
  - the intended recipient, token and `minimumAmount`: only the actual payout
    is recorded, and the recipient is inferred;
  - the original T1, except through `FundsMovement`;
  - destination-chain refunds: they are recorded as completed fills, since
    only the off-chain order tells them apart;
  - the outcome of orders filled or refunded outside the indexed chains (all
    non-EVM destinations); they stay "destination unknown";
  - deposits from non-EVM chains: their EVM fills are visible but have no
    source;
  - history before 2025-07-09, when the Depositories appeared;
  - RPC cost: every block of ~50 chains, from an RPC with full block and
    receipt history for backfill;
  - a fill of an unexpected shape, outside the Relay contracts, from a wallet
    not in the set, that lands before its deposit (0 cases in the sample).

### Extending the chain set without rescanning

The indexer will most likely start with a subset of the 50 EVM chains — the
most active ones, or those with a Blockscout instance — and grow. The goal is
that adding a chain never rescans the chains already indexed. Assessed on
2026-10-28.

Adding chain C to indexed chains A and B:

| Direction | Candidates dropped at the TTL | Durable candidate table |
|---|---|---|
| deposit on A → fill on C | works: A's deposit waits as "destination unknown"; C's backfill finds the fill and matches it against unresolved `orderId`s | same |
| deposit on C → fill on A | **breaks**: A was scanned while C's `orderId`s were unknown, the candidate expired, and A must be rescanned | works: C's deposits are looked up among A's stored candidates |
| refund | to the deposit's chain: always visible; to the destination chain: as one of the rows above | same |

**Design rule:** each chain's scan extracts and keeps everything
Relay-relevant on that chain, whatever other chains are indexed. Pairing is a
join on `orderId` that is rerun when a chain is added.

- **Candidate table.**
  - Narrow and append-only: the suffix (or the `i64` key derived from it),
    `chain_id`, block, tx hash; indexed by the key.
  - Amount and recipient are read from the receipt only on a match — a
    targeted fetch of one tx, not a rescan.
- **Unresolved deposits** stay in `crosschain_messages` indefinitely. They are
  found by the same derived key, so no new index is needed.
- **Adding C:**
  - backfill C;
  - look up C's deposits among the stored candidates of every indexed chain;
  - look up C's candidates among the unresolved deposits.

  The per-chain checkpoints, independent within a bridge, already let C catch
  up on its own.
- **Not `pending_messages`.** The buffer is built for short waits with
  give-up semantics (see `amb-destination-only-missing-source-gap.md`) and
  stores JSON payloads. Candidates live for years and are numerous. The buffer
  stays for live pairing; the table holds the long tail.

Candidate rule, measured on the 301 Base blocks (≈10 min):

| Rule (any sender) | Solver wallets | Others |
|---|---|---|
| input exactly 32 bytes, `value > 0` (native direct) | 23 | 1 |
| `transfer` / `transferFrom` + 32 bytes | 91 | 8 |
| call to a Relay contract (Router, ApprovalProxy, Depository) | 75 | 67 |
| matched by none of the above | 47 | — |

- The shape and contract rules covered every fill shape in the sample
  without the wallet set.
- The 47 solver txs outside them are Multicall3 calls (those checked carried
  a `requestId`, not an `orderId`), solver operations, and `transferFrom`
  without a suffix.
- The wallet set stays as an extra rule. It also matters for backfill: today's
  `/chains` may lack wallets retired long ago, and the shape rules still catch
  their fills.
- The 67 calls to Relay contracts from senders outside the wallet set were not
  classified one by one. They are expected to be user txs: Depository
  deposits, origin swaps of cross-chain orders, and same-chain swaps. Their `requestId` suffix never matches; they cannot be excluded by
  shape, because a router call from an unknown solver wallet looks the same.
- Without `value > 0`, the 32-byte rule picks up 285 unrelated txs in 10
  minutes; with it, 1.
- Candidate rate: ≈0.44/s on Base, against ≈180 tx/s.

Costs:

- **The candidate table.** Base gives ≈40–45k rows/day; 5–10 chains give tens
  of millions of rows and a few GB per year. This is an estimate: the noise was
  not measured on other chains.
- **It must exist from the first scan of every chain.** It cannot be added
  later without a rescan.
- **Many unresolved deposits early on.** With few chains indexed, most
  deposits stay "destination unknown" until their chain is added. Whether to
  expose them before resolution is a product decision.
- **Stats change retroactively.** When a chain is added, old orders close and
  are projected into past days. Whether the stats projection handles
  late-finalized messages for old dates correctly is unverified.

What still forces a rescan:

- **Widening the candidate rule** (a new fill shape, a router v4): rescan the
  affected period.
- **A fill of an unexpected shape, outside the Relay contracts, from a wallet
  not in the set** (0 in the sample). On chains with a Blockscout instance, a
  newly found wallet's history can be fetched per address instead of reading
  blocks.
- **Full independence from the rules** would need the suffix of every tx, or
  per-range bloom filters: billions of rows a year on Base alone. Not worth
  it.

Scheme 1 extends trivially: store every cross-chain request and filter by
configured chains only when exposing data.

### Is the Relay Chain stream needed in Scheme 2?

Mostly not (assessed 2026-10-28). The stream would be a `LogStream` on the
Oracle `0xd180…`, event `Executed`, key in `topic1`, ≈5 events/s, joined to
deposits through keys computed from origin data.

| Expected benefit | In Scheme 2 |
|---|---|
| confirms a found fill | redundant. The fill is on chain: the suffix matched in a successful tx. The Oracle checks the same plus `minimumAmount` and `deadline`, which Scheme 2 cannot check anyway, and `force` makes its confirmation non-strict |
| tells a fill from a refund | no: same key formula, same order → solver TRANSFER |
| detects missed fills | no: "settled, nothing found" looks the same as "destination not indexed" |
| bounds how long a deposit waits | a TTL does the same; fills arrive within seconds to minutes |
| closes orders whose outcome lies outside the indexed chains | **the only unique value** |

How much that value is worth:

- **Share.** In two v3 regular-key samples, 6 of 87 and 7 of 90 cross-chain
  requests went to non-EVM chains (Solana, Hyperliquid); in a v2 global sample,
  6 of 39. That is ≈7–15%, from small one-day samples, and more if not every
  EVM chain is indexed.
- **Refunds to the origin chain are visible without it.** That is the
  deposit's own chain, and the refund carries the same suffix. Only refunds to
  an unindexed destination are invisible.
- **The result is "closed, outcome unknown".** No destination chain, no tx, no
  fill/refund distinction, and a status value the schema does not have.
- **Stats gain nothing.** `stats_messages.dst_chain_id` is non-nullable, so an
  order without a destination chain is not projected either way.

What it costs:

- **Another chain to stream.** The public RPC is slow and silently drops
  topic-filtered logs. Running a node for a Sovereign Stack rollup on Celestia
  is a project of its own. The Blockscout instance suits lookups, not a live
  stream.
- **Brittle keys.** `getDeterministicId` is off-chain Oracle code that can
  change without an on-chain trace. The fill key needs every deposit of the
  order, and for a multi-input order some deposits may sit on unindexed chains.
- **More moving parts.** A third stream in pairing and a new status in the
  schema and API.

Decision: the base Scheme 2 has two streams. Add the Relay Chain only if the
product must separate "stuck" from "closed elsewhere" for these orders, and
then as status enrichment outside the buffer's pairing.

### Rejected variant: a regular key as enrichment

Scheme 2 plus `orderId` lookups with a regular key (earlier "hybrid"
option) was considered and dropped:

- it covers ≈13–17% of orders and no class of orders completely;
- it adds the key and rate-limit dependency of Scheme 1 without its
  completeness.

### Comparison

| | Scheme 1 (API) | Scheme 2 (on chain) |
|---|---|---|
| Chains | all 59, incl. non-EVM | configured EVM chains; other destinations stay "destination unknown" |
| Destination data | chain, tx, recipient, token, amount | actual payout; recipient inferred; destination-chain refunds look like fills |
| Intended output of the order | yes | no |
| Trust | off chain, Relay | verifiable on chain |
| History | from 2024-01 | from 2025-07-09 |
| Load | ≈5 calls/min live | every block of every chain + candidate receipts |
| Architecture | new path: API poller with its own cursor | a `LogStream`, a new block scanner, the buffer as is, a durable candidate table |
| Adding a chain later | free, if all requests are stored | scan only the new chain |
| Dependencies | key, visibility, limits, API format | RPC (full history for backfill), `/chains` |

### Recommendation

- **If Relay grants global visibility:** Scheme 1. It is cheaper and more
  complete. Add the on-chain spot-check only when it is needed.
- **If not:** Scheme 2 with its two streams. Neither a regular key (see
  *Rejected variant*) nor, by default, the Relay Chain stream (see *Is the
  Relay Chain stream needed in Scheme 2?*) is worth adding.

## Invariants

- The only on-chain link between a deposit and its fill or refund is the
  `orderId` calldata suffix (see `relay-protocol-model.md`, *Invariants*).
  Any on-chain scheme matches on it.
- Scheme 2 is complete only if every block of every indexed chain is read.
  A skipped block loses its fills, and nothing else on chain records them
  except the Relay Chain signal.
- In Scheme 2, what a chain's scan extracts must not depend on which other
  chains are indexed. Otherwise adding a chain forces rescans of the others.
- Relay Chain keys are deterministic functions of origin data and the
  `orderId`, so settlement can be joined without the API.
- API records are keyed by `requestId`, on-chain records by `orderId`. A
  design that mixes sources must keep both.

## Failure Modes / Observability

- **Non-production deposits are indexed** if filtering by event signature
  instead of the prod Depository address (staging on Base: ≈17/day).
- **Fills are missed** if only router events are watched. Direct native and
  ERC-20 fills emit nothing Relay-specific.
- **Non-bridge activity is indexed** if router events or Relay-tagged txs are
  taken as orders. Same-chain swaps and relayed `/execute` calls emit the same
  router events or carry a 32-byte suffix, but have no `orderId`.
- **Refunds are reported as completed transfers.** In Scheme 2, a
  destination-chain refund matches like a fill: same suffix, payout to the
  recipient. Only the API's `status = refund` tells it apart.
- **Destination-first fills are missed** when a new solver wallet sends a
  fill of an unexpected shape, outside the Relay contracts, before the wallet
  set is refreshed or learned.
- **Relay Chain lookups silently miss** when made through the public RPC's
  topic-filtered `eth_getLogs`. Query the Blockscout instance instead.
- **Old orders stay unmatched after a rule change.** Widening the Scheme 2
  candidate rule without rescanning the affected period leaves fills of the
  new shape unmatched for that period.
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
- Not every tx with a 32-byte suffix is a fill. Relayed `/execute` calls
  (from solver wallets) and same-chain swaps (from users) carry a `requestId`
  suffix. Only a match on a known `orderId` identifies a fill or refund.
- The main solver wallet receives `gateway` fees as ERC-20 `Transfer`s. The
  ERC-20 fill filter must use `topic1` (from) = main wallet, never `topic2`.
- Same-chain swaps are out of scope for an interchain index. Indexing them
  would create messages with the same source and destination chain.
  - Self-executed ones have no Depository and no `orderId`.
  - Solver-filled ones (`forceSolverExecution`, deposit-address routes, Tron
    TRX/USDT) are full orders. Their deposit cannot be told from a
    cross-chain one, and they are recognised only once the fill shows up on
    the deposit's chain.
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
- a new fill shape or router version appears (widen the Scheme 2 candidate
  rule and rescan the affected period)

## Open Questions

- Will Relay grant a third-party App the global v3 visibility that the
  relay.link proxy has, and at which rate limit? The answer picks the scheme.
  (Answered so far: v3 does **not** return other integrators' requests to a
  regular key.)
- Which bucket `/requests/v3` uses for an App whose `/requests` limit is
  2/min.
- Scheme 1: the v3 field names for recipient, output token and amount, and
  the full list of in-flight `status` values.
- Scheme 2: the TTL after which a deposit without a found fill is persisted
  as "destination unknown", and how a later fill is merged into it.
- Scheme 2, only if the optional Relay Chain stream is added: which status a
  message gets when it is settled but no fill or refund was found on the
  indexed chains.
- The share of cross-chain orders to non-EVM destinations, measured on a
  larger and global sample than the ≈7–15% from one day.
- Scheme 2 candidate rule on chains other than Base: its noise, the
  resulting candidate-table volume, and whether any solver Multicall3 tx
  carries an `orderId` (only some were checked).
- Whether to expose deposits that stay "destination unknown" until their
  destination chain is indexed.
- Share of destination-chain refunds among terminal orders. It sizes how many
  refunds Scheme 2 reports as completed.
- Share of solver-filled same-chain orders among Depository deposits. They sit
  among "destination unknown" until their fill shows up.
- Whether the stats projection handles messages finalized long after their
  date (orders closed when a chain is added).
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
