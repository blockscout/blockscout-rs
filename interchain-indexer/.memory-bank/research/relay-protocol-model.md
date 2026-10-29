# Relay (relay.link): Protocol Model

## Scope

Relay (https://relay.link, docs at https://docs.relay.link) as an *indexing
target*: how a cross-chain request is executed and settled, which parts of it
are observable on which chain, how a request is identified, and how the
protocol changed over time.

In scope:

- the Relay platform (quotes, routing, request tracking) versus the Relay
  Settlement Protocol (Depository, Relay Chain Hub/Oracle, Aurora Allocator)
- chain coverage, token/asset model, cross-asset routes, call execution
- identifiers (`requestId`, `orderId`, Relay Chain keys) and their on-chain
  presence
- the user-side deposit variants and the solver-side fill variants on EVM
- settlement on the Relay Chain
- protocol and contract history that changes what an indexer must recognise

Out of scope:

- data sources and ingestion: the REST API (payload, visibility, rate limits),
  on-chain fill discovery and the solver wallet set, the Relay Chain as a
  completion signal, the mapping onto interchain-indexer, and the candidate
  indexing schemes. These are in `relay-indexing-schemes.md`.
- an implementation plan or an ADR (no Relay indexer exists yet)
- solver economics, withdrawal/MPC mechanics beyond what they leave on chain
- Relay Vaults (`RelayPool`, `RelayBridge`) — a separate liquidity product
- per-VM details of non-EVM chains (Solana, Bitcoin, TON, XRP, Tron,
  Hyperliquid, Lighter) beyond their existence and depository shape
- gasless / permit / EIP-7702 / ERC-4337 submission paths in depth

Status: **pre-implementation research.** Evidence was gathered on
2026-10-26..27 from the docs, the upstream source repositories, the live API,
and on-chain reads on Ethereum, Base, Optimism, Arbitrum, Polygon and the Relay
Chain. Volumes and address lists are a snapshot of those dates.

## Short Answer

Relay is an **intent / solver network, not a lock-and-mint or message bridge.**
A user deposits funds on the origin chain into a per-chain **Depository**,
tagged with a 32-byte **`orderId`**. A solver pays the user on the destination
chain **from its own inventory**. Settlement — proving the fill and crediting
the solver — happens later and elsewhere: on the dedicated **Relay Chain**
(chain id `537713`). There is no cross-chain message, no payload, and no
destination-side bridge contract that must be called.

Consequences for indexing:

1. **The origin side is log-indexable; the destination side is not.**
   - Explicit deposits emit `RelayNativeDeposit` / `RelayErc20Deposit` from the
     Depository.
   - A destination fill is just a transaction whose **calldata ends with the
     `orderId`**:
     - a native fill to an EOA emits **zero logs**;
     - an ERC-20 fill emits only the token's own `Transfer`.
   - The only fills with Relay-specific logs are those routed through the
     RelayRouter, the ApprovalProxy or the Depository (withdraw-and-fill).
2. **The origin chain does not know the destination.**
   - On-chain at initiation: depositor, token, amount, origin chain, tx hash,
     `orderId`.
   - Destination chain, recipient, output token and amount live only in the
     off-chain order whose EIP-712 hash is the `orderId`.
3. **Every route is potentially a conversion.**
   - Any token on X can be delivered as any token on Y (USDC → WBTC, ETH → SPL
     memecoin).
   - Even USDT → USDT is solver inventory, not a mirrored representation.
4. **Settlement does not record the destination.** The Relay Chain only
   records hub-level accounting (MINT/TRANSFER of hub tokens), under keys
   derived from origin data and the `orderId`. It carries no destination
   chain, fill tx hash, recipient or output token — not even hashed into its
   keys.

Where the missing data can be obtained (the API, on chain, the Relay Chain)
and the two candidate indexing schemes are in `relay-indexing-schemes.md`.

## Why This Matters

Relay differs structurally from every bridge indexed so far. These protocol
facts bound what any ingestion design can observe:

- **No message and no destination contract.** A fill is an ordinary solver
  transaction; the only on-chain link to the deposit is the `orderId`
  calldata suffix.
- **Identity is split across layers.**
  - The API's `requestId` is not on chain.
  - The on-chain `orderId` is not what most API consumers see first.
  - Pre-settlement-protocol history (2024–mid 2025) used yet another,
    short, non-hash id.
- **Conversion by construction.** Even same-symbol routes are paid from solver
  inventory; there is no protocol-level evidence that two tokens are one
  asset.
- **Non-production deployments share mainnet chains** (see *EVM Depository
  deployments*).

## Source-of-Truth Files

Upstream (authoritative for protocol behaviour):

- Docs index: https://docs.relay.link/llms.txt
  - protocol pages under `references/protocol/`
  - changelog: https://docs.relay.link/changelog.md (starts 2024-02-05)
- https://github.com/relayprotocol/relay-protocol-oracle
  - `src/services/attestation/vm/ethereum-vm/index.ts`.
    **The operational spec of what counts as an EVM deposit and an EVM fill.**
    Read this before any docs page.
  - `src/services/attestation/index.ts` (`attestSolverFill`,
    `_getSolverFillOrRefundExecution`) and `src/services/attestation/utils.ts`
    (`getDeterministicId`): what a settlement writes to the Relay Chain and how
    its keys are derived.
- https://github.com/relayprotocol/relay-settlement
  - `packages/depository/packages/ethereum-vm/src/RelayDepository.sol`
  - `packages/depository/packages/ethereum-vm/deployments/addresses.prod.json`
  - `packages/networks/src/networks/*.ts` (prod/stag/dev depository per chain)
  - `packages/sdk/src/order/index.ts` (`Order` v1 / `OrderV2`, `getOrderId`)
  - `smart-contracts/contracts/RelayHub.sol`, `RelayOracleV2.sol`,
    `RelayOracleIdempotencyStore.sol`, `RelayAllocator.sol`
  - `smart-contracts/contracts/deposit-addresses/`
- https://github.com/relayprotocol/relay-periphery
  - `src/RelayRouter.sol`, `src/RelayApprovalProxy.sol`,
    `src/common/Multicall3.sol`, `src/receiver/RelayReceiver.sol`
  - git history = router version timeline
- Live chain registry: `GET https://api.relay.link/chains` (keyless)

API specifications, Relay Chain query endpoints and the interchain-indexer
files relevant to fit are listed in `relay-indexing-schemes.md`.

## Key Types / Tables / Contracts

### Actors

| Actor | Nature | Where |
|---|---|---|
| Relay API / platform | off-chain service: quotes, routing, request tracking | `api.relay.link` |
| Depository | per-chain escrow. EVM: non-upgradable contract. Solana: Anchor program. Bitcoin, XRP, TON (Highload V3 wallet), Hyperliquid, Lighter: MPC-controlled account (inferred from the Allocator payload builders) | every supported chain |
| Solver | off-chain operator. Protocol identity = one EVM EOA named in the order (`0xf70da978…dbef`); pays from 42 EVM hot wallets (see `relay-indexing-schemes.md`) | — |
| Hub | ERC-6909 ledger of deposited assets, one token id per (origin chain, currency) | Relay Chain `537713`, `0xDDD36172…9318` |
| Oracle | verifies attestations, executes MINT / TRANSFER / BURN / FAST_MINT on the Hub | Relay Chain, `0xd180dc3b…dacaf` (`RelayOracleV2`) |
| Allocator | MPC-signed withdrawal proofs for solvers | Aurora, `0x7EdA0492…Ff9f` |

The Relay Chain explorer (`explorer.chain.relay.link`) is a Blockscout instance
(≈12.4M txs, ≈6.0M blocks on 2026-10-26; average block time ≈2.86 s).

Oracle address mismatch, resolved on chain:

- The docs list the Oracle as `0xd4b9fdB8…87aa`.
- Live attestations in API responses name `0xd180dc3b…dacaf`.
- In a 200-block window on 2026-10-27, `0xd180…` emitted 2,933 `Executed`
  events in 147 txs, and `0xd4b9…` emitted none. The docs table is stale.

### Identifiers

| Id | Form | On chain? | Notes |
|---|---|---|---|
| `requestId` | 32 bytes | **not as protocol data** | API-level id from the quote; the API's `id` field and filter. Its leading digits read as the creation unix time (observed, undocumented). Seen on chain only as a calldata suffix of Relay-built txs outside the settlement protocol (see *Request kinds*) |
| `orderId` | bytes32, EIP-712 hash of the `Order` | **yes** | `id` arg of the Depository events, or the 32-byte calldata suffix of an implicit deposit, a fill, or a refund |
| `onchainId` | `gdi(slug, txId, logIndex)` | derived | per-deposit-log id used by the Oracle. For ERC-20 deposits `logIndex` is that of the token `Transfer` to the Depository, **not** of `RelayErc20Deposit` |
| Hub `idempotencyKey` | bytes32 | Relay Chain (`Executed` topic1) | deposit MINT = `gdi(slug, txId)`; fill or refund TRANSFER = `gdi(gdi(sorted onchainIds…), orderId)`. Not the `orderId` itself |
| legacy request id | 3–4 bytes (2024) | calldata only | e.g. `0x0132d0`, `0x011ac27a`. Predates the settlement protocol |

- `gdi(...)` is the Oracle's `getDeterministicId`:
  `"0x" + sha256(values.join(":").toLowerCase())`. `slug` is the Relay chain
  slug (`ethereum`, `polygon`), not the numeric id: `gdi("1", tx)` does not
  match, `gdi("ethereum", tx)` does.
- All three formulas were checked against live data (see *Relay Chain
  settlement as a completion signal* in `relay-indexing-schemes.md`).
- `requestId` → `orderId` is many-to-one at most. A live `orderId` lookup
  returned exactly one request.
- Duplicates and refunds are separate requests linked via
  `duplicateDeposits` / `supersededByRequestId`.
- "id" alone is ambiguous:
  - the API's `id` field and filter is the `requestId`;
  - the `id` argument of the Depository events is the `orderId` (`depositId`
    in the Oracle code);
  - the `id` of `RelayCallExecuted` is a Depository call-request id, neither
    of the two.

### Request kinds

Not every API request is a cross-chain order, and not every order is
cross-chain. Four kinds were inspected (2026-10-28):

| Kind | Example request | On chain | `protocol` | Calldata suffix |
|---|---|---|---|---|
| cross-chain order | `0x1793041449…a890` (Ethereum → Arbitrum USDT) | user deposit to the Depository on X; solver fill on Y; settlement on the Relay Chain | `orderId`, deposit, settlement | `orderId` (fill, refund, implicit deposit) |
| relayed call, `POST /execute` | `0x1793175266…55fb` (Ethereum) | one tx from a solver hot wallet to Multicall3 `aggregate3Value`, wrapping a user-signed payload | `null` | `requestId` |
| same-chain swap, self-executed (default) | `0x1793173350…f9cc` (Ethereum USDT → ETH) | one user tx to ApprovalProxy `transferAndMulticall` → Router; swap through a DEX aggregator; payout to the user in the same tx | `null` | `requestId` |
| same-chain swap, filled by the solver | `0x1793184451…d651` (Ethereum ETH → USDT) | same shape as a cross-chain order, both sides on one chain: user deposit to the Depository, then a solver fill; settlement on the Relay Chain | `orderId`, deposit, settlement | `orderId` |

- **Relayed call.** Relay's gasless execution: an app with an API key and a
  funded balance submits `to`, `data`, `value` and an optional EIP-7702
  `authorizationList`, and a solver hot wallet posts it, paying the gas.
  - The inspected one was a type-4 tx carrying the user's EIP-7702
    authorization: it delegated the EOA to `0x7702cb55…` and called
    `setImplementation` with the Coinbase Smart Wallet implementation
    `0x000100ab…9e72`, logging `Upgraded` and two `AddOwner`. No value moved.
  - The same relayer path also posts gasless origin deposits of cross-chain
    orders (e.g. MetaMask Pay, Base → Hyperliquid).
- **Same-chain swap.** The inspected tx:
  - pulled 162.04 USDT from the user;
  - paid the `gateway` fee (0.097224 USDT) to the main solver wallet
    `0xf70da978…`;
  - swapped through 0x AllowanceHolder (`0x…1ff3684f…`) on a Uniswap V3
    pool;
  - paid 0.0674 ETH to the user with `SolverNativeTransfer`.

  No Depository event; `outTxs` holds only `{chainId}` with no hash.
- **Same-chain swap filled by the solver.** Same-chain swaps are
  self-executed by default. The solver fills them only for:
  - quotes with `forceSolverExecution: true` ("Force executing swap requests
    via the solver (by default, same-chain swap requests are self-executed)");
  - deposit-address routes;
  - Tron TRX/USDT (changelog 2026-09-09).

  The inspected order had no deposit address and a direct `depositNative`, so
  it was most likely forced; the API does not return quote parameters. It ran
  as follows:
  1. The user deposited 0.0316 ETH through `depositNative` (`RelayNativeDeposit`,
     block 26290725).
  2. In the next block, hot wallet `0x18dd3c14…` called
     `USDT.transferFrom(main wallet, 0xf690a758…, 74.602952)` with the
     `orderId` suffix. The recipient is not the depositor.
  3. MINT and TRANSFER were settled in one Relay Chain tx, `0xa507ab05…`.

  Its route is `relay` on both sides (no DEX), and the fees are the solver's
  (`gas`, `fixed`, `price`, `gateway`).
- **The relay.link "Deposit" / "Fill" labels are the API's `inTxs` /
  `outTxs` slots, not protocol roles.** A relayed call shows the same tx in
  both.

### `Order` (format "v1", the one returned by live quotes)

From `packages/sdk/src/order/index.ts`; confirmed in `/quote/v2` and
`/requests/v2?includeOrderData=true` responses.

- `solverChainId` + `solver`: exclusive filler identity. In every sampled order
  it was `base` / `0xf70da978…dbef`.
- `salt`: random uniqueness salt.
- `inputs[]`, each with:
  - `payment{chainId, currency, amount, weight}`
  - `refunds[]{chainId, recipient, currency, minimumAmount, deadline, extraData}`
- `output`:
  - `chainId`
  - `payments[]{recipient, currency, minimumAmount, expectedAmount}`
  - `calls[]`
  - `deadline`
  - `extraData`: ABI-encoded **fill contract** (the router allowed to emit
    fill events)
- `fees[]`

Chain ids inside an order are **Relay slugs** (`"base"`, `"arbitrum"`), not
numeric ids. Live orders had one input, one payment, empty `calls`, and two
refund options (origin chain in the input currency, or destination chain in
USDC).

The SDK also defines `OrderV2`: a single input plus `output.withdraw` and
`output.fill`. It was not seen in production responses.

### EVM Depository deployments

Prod, from `addresses.prod.json` (78 EVM deployments, CREATE2 via
`0x4e59b448…956C`):

| Address | Chains |
|---|---|
| `0x4cd00e387622c35bddb9b4c962c136462338bc31` | 70 chains (Cancun EVM) |
| `0x59916da825d2d2ec1bf878d71c88826f6633ecca` | 7 London-EVM chains (Cronos, Metis, Mantle, Linea, Polygon zkEVM, …) |
| `0xa88cf7864951147a08707ed732237eaa9b1c3b9b` | Zero (543210) |

- On Ethereum the prod Depository's first tx is 2025-07-09 (block
  22883244); it is verified as `RelayDepository`.
- The live `/chains` registry lists 46 EVM chains on `0x4cd0…` and 4 on
  `0x5991…`.

Non-prod deployments on the **same mainnets**, from
`packages/networks/src/networks/*.ts`:

- `stag`: `0x9ddc6a541e8f8b50b0996786a3ec275ab4d3a76c` (Cancun) /
  `0xa4da4ec0558404cebba46bd112663723bc89829b` (London)
- `dev`: `0x5cb1de3603a71ac2f67b12bfbf095013fe4ac299`

The staging Depository on Base emitted 17 `RelayErc20Deposit` in ≈24 h
(2026-10-26).

Hyperliquid mismatch: the repo's prod depository is `0x865eb9ba…7081`, but
`/chains` reports `0x66cf0aac…2224`. Unresolved.

### Event and selector surface (EVM)

| Item | Signature | topic0 / selector |
|---|---|---|
| Depository | `RelayNativeDeposit(address from, uint256 amount, bytes32 id)` | `0x80320665…3477` |
| Depository | `RelayErc20Deposit(address from, address token, uint256 amount, bytes32 id)` | `0x49fed1d0…55e1` |
| Depository | `RelayCallExecuted(bytes32 id, (address,bytes,uint256,bool) call)` | `0x4be10945…c5ea` |
| Router (≤v3.1) / Multicall3 | `SolverCallExecuted(address to, bytes data, uint256 amount)` | `0x93485dcd…c324` |
| Router v3.2 | `SolverCallExecuted(address to, bytes32 dataHash, uint256 amount)`: call data replaced by its hash | `0x41b532d6…` (types recovered by keccak match on a live tx; parameter names inferred; ABI not published) |
| Router | `SolverNativeTransfer(address to, uint256 amount)` | `0xd3546797…dba6` |
| Router / ApprovalProxy (v3+) | `FundsMovement(address from, address to, address currency, uint256 amount, bytes metadata)` | `0xafbab204…2b91` |
| Legacy RelayReceiver | `FundsForwardedWithData(bytes data)` | `0x936c2ca3…f53e` |
| Relay Chain Oracle | `Executed(bytes32 indexed idempotencyKey, bytes[] actions)` | `0xa3c855af…7026` |
| Depository | `depositNative(address,bytes32)` | `0x49290c1c` |
| Depository | `depositErc20(address,address,uint256,bytes32)` | `0xe8017952` |
| Depository | `depositErc20(address,address,bytes32)` (full allowance) | `0x5a1ee3ac` |
| ApprovalProxy | `transferAndMulticall(address[],uint256[],Call[],address,address,bytes)` | `0xf9e4bab4` |

- No Depository event parameter is indexed, so the `orderId` cannot be
  topic-filtered.
- The EVM Depository has **no `receive`/`fallback`**: native deposits always go
  through `depositNative` and always emit `RelayNativeDeposit`.

### Router / ApprovalProxy versions

From the contract-addresses docs page, the `/chains` `contracts` object, and
`relay-periphery` history:

| Version | Router (Cancun / most chains) | Note |
|---|---|---|
| v2 | `0xf5042e6f…e222` | 2024–2025 |
| v2.1 | `0x3ec130b6…22b6` | deployed to all chains 2025-09-23 |
| v3 | `0xb92fe925…4fff4f` | 2025-10-30 → 2025-11-06; "events with explicit id" (`FundsMovement` + metadata) |
| v3.2 | `0x0729e2a4…97f7` (Cronos/Linea/Mantle/Metis: `0x6328ea02…0e05`) | deployed 2026-10-14; default from 2026-10-26 10:00 ET; `SolverCallExecuted` hashes call data |

- London and zkEVM chains use other addresses per version.
- ApprovalProxies are versioned in lockstep (v3.2: `0x5567954c…2e58`).
- v3 and v3.2 stay live in parallel during the transition.

## Step-by-Step Flow

### 1. Quote (off chain)

`POST /quote/v2` with `user`, `recipient`, `originChainId`,
`destinationChainId`, `originCurrency`, `destinationCurrency`, `amount`,
`tradeType` (`EXACT_INPUT` | `EXACT_OUTPUT`). The response carries:

- `requestId`
- `protocol.v2.orderId` and `orderData`
- `steps[]` with ready-to-sign transactions or signatures

The `orderId` depends on the solver's commitment and a random salt. **A valid
deposit cannot be constructed without a quote**, and the destination cannot be
read back from the origin chain.

### 2. User deposit on origin chain X

The EVM variants below were all observed in live quotes:

| Case | User txs | Observable on X |
|---|---|---|
| T1 = native | `Depository.depositNative(user, orderId)` with value | `RelayNativeDeposit` |
| T1 = solver-accepted ERC-20, explicit (default; smart wallets) | `T1.approve(Depository)`, then `depositErc20(user, T1, amount, orderId)` | `Transfer(→Depository)` + `RelayErc20Deposit` |
| Same, implicit (`explicitDeposit:false`, EOAs) | one `T1.transfer(Depository, amount)` with `orderId` appended to calldata | only `Transfer(→Depository)`; id is in calldata |
| T1 not in the chain's `/chains` `solverCurrencies` (origin swap) | `T1.approve(ApprovalProxy)`, then `ApprovalProxy.transferAndMulticall` → router: approve DEX, swap T1 → a solver currency, `cleanupErc20sViaCall` → `Depository.depositErc20(user, <solver currency>, orderId)` | `RelayErc20Deposit` shows the **post-swap currency**, not T1. T1 is visible only in `FundsMovement` / `Transfer` logs of the same tx. The order's input is the post-swap currency. In the six global windows of 2026-10-24..28, 246 of 1,727 EVM-origin cross-chain orders (14%) swapped: into USDC 138, USDG (Robinhood) 76, ETH from WETH 17, USDT 6, others; 82% deposited the user's token as is (17 distinct tokens) |
| Deposit address (`useDepositAddress`) | plain transfer of T1 to a counterfactual CREATE2 address derived from `(orderId, depositor)` | nothing Relay-specific. Later a Relay-sent sweep deploys the proxy and calls `depositErc20` / `depositNative`, emitting the Depository event with `depositor` = user |
| Via aggregators (LI.FI, smart accounts, ERC-4337 bundlers) | Depository call happens inside another contract's tx | same Depository event; `from` = the credited depositor, e.g. the real EOA rather than the LI.FI diamond |

The oracle's deposit detection is the operational definition (see
`relay-protocol-oracle`, EVM attestor):

- Recognised: `RelayNativeDeposit`, `RelayErc20Deposit`, or any `Transfer`
  whose `to` is the Depository.
- The id comes from a following matching `RelayErc20Deposit`, or else from the
  32 bytes after a `transfer` / `transferFrom` encoding in the tx calldata.
- Ids beginning with 16 zero bytes are ignored. A deposit with no recoverable
  id gets `depositId = 0x0…0`.

### 3. Fill on destination chain Y (solver)

The Oracle accepts a fill tx when all of these hold:

1. it succeeded and is final;
2. its timestamp ≤ `output.deadline`;
3. **`tx.input` ends with `orderId`**;
4. the recipient received ≥ `minimumAmount` of the output currency, computed as:
   - **native**: either a direct transfer (`tx.to == recipient` and input
     starting with `orderId`), or `SolverNativeTransfer(to=recipient)` from the
     order's fill contract;
   - **ERC-20**: sum of `Transfer(to=recipient)` logs of that token in the tx;
5. each `output.calls[]` entry is matched in order by a `SolverCallExecuted`
   from the fill contract.

The Oracle **does not check `tx.from`**. Observed fill shapes:

- **Native, direct.** Solver EOA → recipient, `input == orderId`.
  **No logs at all.** Example: Arbitrum `0xe5435cd1…8003`.
- **ERC-20, direct.** Solver EOA → `token.transfer(recipient, amt)` /
  `transferFrom`, with `orderId` appended. Only the token `Transfer` log.
  Example: Arbitrum `0x59db66d6…15a5`.
- **Via RelayRouter `multicall`** (`0xcd6e13f7`), used for destination swaps
  and calls. Emits `SolverNativeTransfer` / `SolverCallExecuted` /
  `FundsMovement` plus token transfers.
- **Via ApprovalProxy**, or **`Depository.execute`** on Y (withdraw-and-fill:
  funds leave the Y-side Depository itself). The latter emits
  `RelayCallExecuted`, whose id is a call-request id, not the `orderId`.

If T2 is not solver inventory, the solver swaps on Y itself (the quote's
`route.destination`). The order's output still names T2 directly.

### 4. Settlement on the Relay Chain

1. The solver requests attestations from the off-chain Oracle service. For a
   fill it sends the full `Order`, the deposit `{transactionId, onchainId}`s
   and `fill.transactionId`. The service checks them on both chains and signs.
2. It submits them to the Oracle contract (`execute` / `executeMultiple`).
   Only `Execution{bytes32 idempotencyKey, bytes[] actions}` reaches the
   chain. The fill tx hash, destination chain, recipient and output stay in
   the off-chain request.
3. That runs Hub actions:
   - **MINT** (deposit credited to an order address, hub token =
     origin chain + currency);
   - **TRANSFER** (order address → solver on a fill or refund; on a fill,
     plus one TRANSFER per `order.fees[]` entry, solver → fee recipient);
   - **BURN** (on solver withdrawal).

In both inspected orders, deposit MINT and fill TRANSFER were executed in
**one** Relay Chain tx (`0xe73983e7…08f2`, `0xc400eb4f…7c11`). Relay Chain
records carry no `orderId`, destination chain, fill tx, recipient or output
token. What they do carry, and how to use it, is in *Relay Chain settlement as
a completion signal* in `relay-indexing-schemes.md`.

### 5. Refunds

- **Fast refund**: the solver pays the user back on the origin chain, with the
  same calldata-suffix convention, and the refund is settled like a fill.
- The order lists refund options on both chains.
- **A destination-chain refund looks like a fill on chain.** That option pays
  the order's recipient on the destination chain — USDC in the live
  cross-chain orders; USDT to the recipient in the solver-filled same-chain
  order. It carries the same `orderId` suffix. Only the output token, which is
  in the off-chain order, tells it from a fill.

## Answers To The Protocol Questions

The original integration questions 5 (message identifier), 7 (single
collection point), 8 (what to index) and 9 (data known at initiation) concern
data sources and are answered in `relay-indexing-schemes.md`.

### 1. Chain coverage and registry

- `GET /chains` (2026-10-26) returns **59 mainnet chains**:
  - 50 EVM;
  - 2 `svm` (Solana, Eclipse), 2 `lvm` (Lighter, Robinhood-Lighter);
  - one each of `hypevm` (Hyperliquid), `xrpvm`, `bvm` (Bitcoin), `tonvm`,
    `tvm` (Tron).
- The testnet API (`api.testnets.relay.link`) returns 2 EVM chains.
- The docs' "80+" (and "69+" on another page) is stale. Chains were removed
  (Soon, Sui, and others), and 78 prod EVM Depositories exist versus 50 live
  EVM chains.
- **No authoritative on-chain registry.** The closest is
  `RelayAllocator.payloadBuilders[chainSlug][depository]` on Aurora
  (`PayloadBuilderSet` events with a hashed `string indexed chainId`). It
  enumerates withdrawal-enabled `(chain, depository)` pairs, keyed by Relay
  slug, not EVM chain id.
- Non-EVM chains carry Relay-assigned numeric ids (e.g. Solana `792703809`,
  Bitcoin `8253038`). They fit a `u64` but are not EVM chain ids and have no
  EVM RPC.

### 2. Token types

- **Input:** native and ERC-20 (SPL on Solana) only — the Depository API has
  nothing else.
- **NFTs are not bridgeable.** On the destination the router can only receive
  and forward ERC-721 / ERC-1155 minted or bought by a call (`nftRecipient`,
  `onERC721Received`); failure reasons include `MINT_NOT_ACTIVE`,
  `SEAPORT_INEXACT_FRACTION`.
- Hub tokens are fungible only (ERC-6909).

### 3. Cross-asset

Yes, any-to-any, including stable → non-stable. Observed: Base ETH → Solana
PUMP, Polygon USDT → Robinhood USDG, a quote Base USDC → Arbitrum WBTC.

### 4. Arbitrary messages and calls

- There is no AMB-style message channel.
- `output.calls[]` lets the solver execute arbitrary calls on Y through the
  router. "Call execution" means cross-chain mints, deposits and so on, paid
  with any token.
- The calls are verifiable only via `SolverCallExecuted` (hashed since v3.2).

### 6. Transfers per message

- **Usually one.** Every live order had one input and one payment.
- The `Order` v1 schema allows **multiple inputs** (multi-chain) and
  **multiple output payments**.
- The API's `settlement.destination.fills[]` is an array.
- Gas top-up adds destination native on top of the token. That it is a second
  payment entry is an inference, not verified.
- Duplicate deposits against one request become linked child requests.
- Do not assume one transfer per order.

### 10. History and protocol changes

| Date | Change | Indexing impact |
|---|---|---|
| 2024-01-10 | earliest request in the API; deposits = native transfers to solver EOA `0x81f91aca…` with 3-byte ids | no contract events |
| ≤ 2024-06 | `RelayReceiver` (`FundsForwardedWithData`), solver `0xf70da978…`, 4-byte ids | receiver events; fills matched by id in calldata |
| 2024-07 → 2026-10-12 | "canonical" / "canonical+" routes through native rollup bridges; removed from the quote engine 2026-10-12 | a separate, non-solver request shape in history |
| 2024-12 | deposit-address fallback (UI); later DepositAddressFactory sweeps into the Depository | depositor transfer ≠ Depository event tx |
| 2025-07-09 | Depository / settlement protocol ("protocol v2") with 32-byte `orderId`; rollout gated per chain and by USD threshold until 2025-09-19 (threshold removed) | mixed v1/v2 traffic in 2025-07..09 |
| 2025-09 / 2025-10-30 / 2026-10-26 | router v2.1 / v3 (`FundsMovement` + metadata) / v3.2 (`SolverCallExecuted` hashes calldata) | new addresses and event shapes per version |

API lifecycle changes (`/requests/v2` sunset, v3 key requirement) are in
`relay-indexing-schemes.md`.

`protocolVersion` is no longer a quote parameter, and all sampled cross-chain
requests carry `protocol.orderId`. The current platform therefore appears to
route every cross-chain order through the settlement protocol — an inference,
not a documented guarantee.

## Invariants

- On EVM, every native Depository deposit emits `RelayNativeDeposit` (no
  `receive`/`fallback`). ERC-20 deposits always produce a token `Transfer` to
  the Depository, but `RelayErc20Deposit` only for explicit deposits.
- A valid fill or refund tx's calldata ends with the 32-byte `orderId`. This
  is the only on-chain link between origin and destination.
- Depository contracts are non-upgradable. A changed address means a new
  deployment, not an upgrade.
- An order names exactly one solver identity. Fill senders are unconstrained.
- Only the destination chain itself records where and how an order was
  filled. The Relay Chain records that it was settled, under a key derived
  from origin data and the `orderId`.

## Failure Modes / Observability

Ingestion-side failure modes (missed fills, non-production deposits, silent
API coverage gaps) are in `relay-indexing-schemes.md`. On the protocol side:

- **Docs drift.** The docs' contract tables and chain counts lag the live
  deployments (the Oracle address, "80+" chains). Prefer `/chains` and
  on-chain reads.

## Edge Cases / Gotchas

- `requestId` ≠ `orderId`. The UI and most integrators show `requestId`; only
  `orderId` is on chain.
- Implicit-deposit ids starting with 16 zero bytes are treated as "no id" by
  the Oracle. A deposit with no recoverable id is still a deposit with
  `id = 0x0`.
- zkSync-stack chains: ERC-20 `Transfer`s of the ETH system token
  (`0x…800a`) are skipped by the Oracle.
- Deposit-address strict `EXACT_INPUT` addresses are one-time use since
  2026-10-05. Extra deposits are refunded as separate legs.
- Router v3 and v3.2 run in parallel during migrations. Approvals do not
  carry over between ApprovalProxy versions.
- An ERC-20 deposit's `onchainId` uses the `Transfer` log index, one below the
  `RelayErc20Deposit` log in the observed txs.
- Router events (`SolverNativeTransfer`, `FundsMovement`,
  `SolverCallExecuted`) are emitted by same-chain swaps too. They do not by
  themselves mean a cross-chain fill.
- Solver hot wallets are also Relay's `/execute` relayer, so a tx from a
  solver wallet is not necessarily a fill.
- An `orderId` does not imply a cross-chain order. Solver-filled same-chain
  swaps are full protocol orders whose origin and destination are one chain.

## Change Triggers

Update this note when any of these happen:

- `/chains` adds or removes chains, or Depository addresses change (e.g. the
  Hyperliquid mismatch resolves)
- a new router or ApprovalProxy version, or the v3.2 `SolverCallExecuted` ABI
  is published (confirm the inferred parameter names)
- a new request kind appears, or `requestId` / `orderId` suffix usage changes
- `OrderV2` appears in production orders, or multi-input / multi-payment
  orders are observed
- a second solver identity appears in orders
- the Oracle contract is replaced, or `getDeterministicId` / the
  idempotency-key formulas change

## Open Questions

- Direction of the 2025-07..09 USD threshold (which orders used legacy versus
  the Depository), and the exact date legacy `RelayReceiver` deposits stopped.
- Shape of canonical / canonical+ requests on chain, if they are to be covered
  historically.
- Whether gas top-up and destination swaps produce one or several
  `output.payments` entries.
- Non-EVM origins: how Solana / Bitcoin / TON deposits carry the `orderId`.
