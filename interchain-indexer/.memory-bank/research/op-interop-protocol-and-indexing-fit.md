# OP Stack Interop (Lagoon): Protocol Model, Deployment Status, and Indexing Fit

## Scope

OP Stack native interoperability ("Superchain interop"), shipped by the
**Lagoon** L2 hardfork, as an *indexing target*: what is deployed and when, the
contract and event surface, how a message is identified end to end, what a
transfer can and cannot carry, the safety/finality model, and which parts of the
existing interchain-indexer architecture it fits.

In scope (primary target):

- `L2ToL2CrossDomainMessenger` messages (`SentMessage` → `RelayedMessage`)
- native ETH transfers via `SuperchainETHBridge` / `ETHLiquidity`
- message expiry and ETH refunds (`UndeliveredMessageExported`,
  `MessageExpired`, `RefundETH`)
- `CrossL2Inbox.ExecutingMessage` as an attribute of a messenger relay

Reference only, deferred to a second phase: the **pull model** (contracts
reading arbitrary logs through `CrossL2Inbox.validateMessage` without the
messenger).

Reference only, not a target: **ERC-7802** token events (see *ERC-7802 Is Not
Part of the Protocol*).

Out of scope: an implementation plan, concrete schema changes, the Super Fault
Dispute Game / super-root proofs, L1 contract upgrades (ETHLockbox, OPCM
`migrate`) beyond what an indexer observes on L2, and node-operator topology
beyond what constrains RPC choice.

Status: **pre-implementation; no OP interop indexer exists in this repo.**
Interop is **not live on any network** as of the snapshot date. Every status
fact below is a snapshot taken **2026-11-11**; protocol facts were read at
optimism `develop` `09c4bf5a3a` (2026-11-11) and specs `main` `1849ed23c2`
(2026-11-10). Evidence tags: **[V]** verified against source code, a primary
document, or on-chain state; **[R]** reported by a secondary source; **[I]**
inferred.

## Short Answer

Lagoon is not live anywhere. The first activation is a *target*, not a
scheduled timestamp: OP Sepolia + Unichain Sepolia around 2026-12-03, then OP
Mainnet + Unichain around 2026-12-15 or 2027-01-11 (the operator notice gives
both), subject to a governance vote that has not been posted. Each pair forms a
two-chain dependency set; launch scope is **native ETH transfers only**. Base
left the OP Stack and is not part of interop.

The protocol has two layers. `CrossL2Inbox` (`0x4200…0022`) lets any
transaction "execute" any log from any dependency-set chain and emits
`ExecutingMessage(msgHash, Identifier)`. `L2ToL2CrossDomainMessenger`
(`0x4200…0023`) builds addressed, replay-protected messages on top of it:
`SentMessage` on the source, `relayMessage` by anyone on the destination, which
emits `ExecutingMessage` + target logs + `RelayedMessage` in one transaction.

A messenger message is identified by `(source_chain_id, nonce)`, equivalently
by the messenger `messageHash`, which `RelayedMessage`, `MessageExpired` and
`RefundETH` carry. Shape-wise it is a Teleporter-like two-sided message with
plain EVM chain IDs, so the existing buffer/consolidation pipeline fits. The new
parts are a status model with **no failure event** (a failed relay reverts and
leaves nothing), **late terminal events** (expiry/refund arrive on the source
chain 8+ days later, keyed only by `messageHash`), and **unsafe-block
replacement** when an executing message is invalid. `LogStream` currently reads
to `latest` with no reorg handling.

There is no canonical ERC-20 bridge at launch: `SuperchainTokenBridge` and
`SuperchainERC20` were deleted from the code, although the spec and parts of the
docs still describe them.

## Why This Matters

1. **The spec and docs lag the code.** The spec still defines
   `SuperchainTokenBridge` (`0x…28`), `OptimismSuperchainERC20Factory`/`Beacon`
   (`0x…26`/`0x…27`) and their events. None of them ships in Lagoon. The docs
   claim `CrossL2Inbox` checks that the source log exists. It does not; validity
   is enforced out of protocol. Design from the contracts, not the prose.
2. **Failure is invisible in logs.** A relay whose target reverts reverts as a
   whole, including `ExecutingMessage`. "Not relayed yet", "relay attempted and
   failed" and "will never be relayed" look identical until expiry.
3. **Two different hashes.** `ExecutingMessage.msgHash` is the hash of the
   source *log*. The messenger `messageHash` is the hash of the *message
   fields*. Mixing them breaks every join.
4. **Unsafe data can disappear.** A block with an invalid executing message is
   replaced by a deposits-only block at the same height. `latest` is not final,
   and `safe` is cross-chain-checked only on supernode-wired nodes.
5. **The ABI is still moving.** `RelayedMessage` changed `topic0` in 2025;
   `resendMessage` was removed and expiry/refund added in October–November 2026;
   `ETHLiquidity` renames are announced in code comments. Blockscout's existing
   indexer already filters on a stale `topic0` (see *Prior Art*).
6. **The same addresses mean different things on non-members.** On Base,
   `0x4200…0030` is `BaseTime`, not `UndeliveredMessageExporter`.

## Deployment Status (snapshot 2026-11-11)

### Lagoon

- **[V]** Lagoon is the L2 hardfork that activates interop. It was named
  "Interop" until optimism PR #21105 (merged 2026-06-03); the registry field
  `interop_time` became `lagoon_time` in superchain-registry PR #1255
  (2026-06-11).
- **[V]** The spec says Lagoon "is not finalized and should be considered
  experimental" (https://specs.optimism.io/protocol/lagoon/overview.html). The
  docs hardfork registry marks it "In development"
  (https://docs.optimism.io/op-stack/protocol/hardforks/lagoon.md).
- **[V]** Lagoon also carries non-interop changes that appear in the same
  blocks: post-exec transactions (tx type `0x7D`, last in a block, no logs), a
  receipt field `opGasRefund` from sequencer-defined metering, smaller BLS12-381
  precompile inputs, and an interop transaction pool. The Super Fault Dispute
  Game is explicitly not part of it.
- **[V]** The L2 upgrade bundle `op-core/nuts/bundles/lagoon_nut_bundle.json`
  deploys five interop predeploys only: `CrossL2Inbox` (1.1.0),
  `L2ToL2CrossDomainMessenger` (2.1.0), `SuperchainETHBridge` (2.0.0),
  `ETHLiquidity` (2.0.0), `UndeliveredMessageExporter` (1.0.0).
- **[V]** The contracts release is `op-contracts/v10.0.0-rc.1` (2026-11-05),
  described as "op-contracts for Upgrade 22 … the contracts for Lagoon interop".
- **[V]** No chain in superchain-registry `main` (2026-11-10) has `lagoon_time`.
  op-supernode v1.1.1 release notes (2026-11-05): "Lagoon not scheduled on any
  registry chain".

### Targets and governance

| Network | Chains | Target | Source |
|---|---|---|---|
| Testnet | OP Sepolia (11155420) + Unichain Sepolia (1301) | 2026-12-03 | interop-prep notice |
| Mainnet | OP Mainnet (10) + Unichain (130) | 2026-12-15 *or* 2027-01-11 | interop-prep notice (contradicts itself) |

- **[V]** https://docs.optimism.io/notices/interop-prep.md: "the exact
  timestamps are not yet set"; mainnet is "pending governance approval". Its
  first sentence says January 11, 2027 (changed by optimism PR #23721,
  2026-10-27); its title and note still say December 15, 2026. The target moved
  repeatedly before: "July 2026" (May 2026), then late July, then
  December (2026-10-23).
- **[V]** Forum post "Towards Mainnet Interop", 2026-10-22
  (https://gov.optimism.io/t/towards-mainnet-interop-introducing-the-inaugural-unichain-and-op-mainnet-set/10919):
  the first release focuses on "native ETH transfers between Unichain and OP
  Mainnet". It explicitly excludes "generalized ERC20 transfer support for every
  existing token" and interoperability with every OP Stack chain. Other chains
  "join through later proposals". A separate activation proposal is still to
  come.
- **[V]** Prerequisites: Upgrade 20 (super-root dispute games) executed on OP
  Mainnet 2026-09-24. Upgrade 21 "Interop readiness" (ETHLockbox for every
  chain, OPCM `migrate()`) executed on Sepolia 2026-11-09, mainnet targeted
  2026-11-16. U21 "does not activate interop"
  (https://docs.optimism.io/notices/upgrade-21.md).

### On-chain verification

- **[V]** Proxies exist at `0x4200…0022`–`0x…0030` on OP Mainnet, Unichain,
  Base, Ink, Soneium, their testnets, and OP Labs staging devnets
  `sepolia-devnet-2..6`. Their EIP-1967 implementation slot is zero on all of
  them. The control `0x4200…0007` (L2CrossDomainMessenger) has an
  implementation.
- **[V]** Zero logs at `0x…22/23/24/25/28/30` in the last 10,000 blocks of each
  network listed above.
- **[V]** The L1 `SystemConfig` feature flag `INTEROP` is `false` for OP
  Mainnet, Unichain, OP Sepolia and Unichain Sepolia.

### Chains, devnets, and Base

- **[V]** All public interop devnets are offline: interop-alpha (420120000/1),
  rc-alpha, rc-betanet, interop-flash, rehearsals, interop-jnt-v0..v3,
  dev-preview, dev-preview-v2 (https://devnets.optimism.io/). The docs
  get-started page still points to interop-alpha, which returns 404. The only
  usable environment before testnet activation is local Supersim (chains
  901/902).
- **[V]** Explorers for the first pairs are Blockscout instances:
  `explorer.optimism.io`, `testnet-explorer.optimism.io`,
  `unichain.blockscout.com`, `unichain-sepolia.blockscout.com`.
- **[V]** Base announced on 2026-02-18 that it is moving to its own stack
  (https://base.org/blog/next-chapter-for-base-chain-1). It was removed from the
  superchain-registry on 2026-04-27 (superchain-registry PR #1212) and was not
  part of U20/U21. **[I]** Base does not participate in Superchain interop.
- **[R]** Unichain moved to "OP Enterprise" in October 2026 "to prepare for
  native interoperability" (https://blog.uniswap.org/unichain-is-upgrading-to-op-enterprise).
- **[V]** `op-supervisor` was deleted in May 2026 (optimism PR #20997). Its
  replacement is **op-supernode** (stable v1.0.0 August 2026, v1.1.1
  2026-11-05), which derives every chain of a dependency set in one process,
  plus "Light CL" op-nodes that follow it (`--l2.follow.source`).

## Source-of-Truth Files

### Protocol (external)

Contracts, in `ethereum-optimism/optimism` `packages/contracts-bedrock/`:

- `src/L2/CrossL2Inbox.sol` (1.1.0): `validateMessage`, `ExecutingMessage`,
  checksum
- `src/L2/L2ToL2CrossDomainMessenger.sol` (2.1.0): `sendMessage`,
  `relayMessage`, `expireMessage`, nonce, hashes
- `src/L2/SuperchainETHBridge.sol` (2.0.0), `src/L2/ETHLiquidity.sol` (2.0.0)
- `src/L2/UndeliveredMessageExporter.sol` (1.0.0)
- `src/libraries/Hashing.sol` (`hashL2toL2CrossDomainMessage`),
  `src/libraries/Encoding.sol` (versioned nonce), `src/libraries/Predeploys.sol`

Off-chain validators and tooling:

- `op-supernode/supernode/activity/interop/` (`algo.go`, `cycle.go`,
  `logdb.go`): the cross-safe verifier
- `op-interop-mon/`: a watchdog that validates every executing message against
  its source log, with a reference resolution algorithm
- `op-interop-filter/`: tx-pool validation (`interop_checkAccessList`)
- `op-core/interop/messages/`: Go decoders; `op-core/interop/depset/`:
  dependency-set links and expiry-window override
- `rust/kona/crates/protocol/interop/src/message.rs`: alloy `sol!` binding for
  `ExecutingMessage`

Specs, at https://specs.optimism.io/interop/: `messaging.html`,
`predeploys.html`, `message-expiry.html`, `superchain-eth-bridge.html`,
`eth-liquidity.html`, `derivation.html`, `verifier.html`, `sequencer.html`,
`dependency-set.html`. `token-bridging.html` is **stale** (describes deleted
code).

Registry: `ethereum-optimism/superchain-registry`, chain TOML `[hardforks]
lagoon_time` and `[interop] dependencies`.

### In this repo (what an OP indexer would sit beside or reuse)

- `interchain-indexer-logic/src/message_buffer/types.rs`: `Key`,
  `Consolidate`, `ConsolidatedMessage`
- `interchain-indexer-logic/src/indexer/evm/`: `log_stream_builder.rs`,
  `transaction_grouping.rs`, `receipt_fetch.rs`, `abi_registry.rs`
- `interchain-indexer-logic/src/log_stream.rs`: realtime head =
  `get_block_number()` (`latest`), no reorg handling
- `interchain-indexer-logic/src/indexer/avalanche/`: Teleporter + ICTT, the
  closest structural analog (send/receive message plus a token layer in the
  same transactions)
- `interchain-indexer-logic/src/indexer/xdai/types.rs`
  (`key_from_native_id`) and `amb/events.rs` (`key_from_message_id`): the
  32-byte-id → `i64` key pattern
- `interchain-indexer-logic/src/protocol_metadata.rs`: namespaced JSONB for
  protocol-specific fields
- `interchain-indexer-entity/src/codegen/sea_orm_active_enums.rs`:
  `MessageStatus` (`initiated`, `completed`, `failed`, `ready_to_claim`)
- `interchain-indexer-server/src/config.rs`: `IndexerType`,
  `BridgeConfig.process_unknown_chains`, `home_chain_id`
- `.memory-bank/adr/006-contract-versioning-by-block.md`: version-by-block
  decoding, relevant to ABI churn

## Key Types / Tables / Contracts

### Predeploys (identical address on every member chain)

| Address | Contract | Lagoon version | Indexer role |
|---|---|---|---|
| `0x4200…0022` | `CrossL2Inbox` | 1.1.0 | executing messages (relay attribute; pull reads in phase 2) |
| `0x4200…0023` | `L2ToL2CrossDomainMessenger` | 2.1.0 | message send / relay / expiry |
| `0x4200…0024` | `SuperchainETHBridge` | 2.0.0 | ETH transfer and refund |
| `0x4200…0025` | `ETHLiquidity` | 2.0.0 | optional; no user attribution |
| `0x4200…0030` | `UndeliveredMessageExporter` | 1.0.0 | expiry export (destination side) |

`0x…26`/`0x…27`/`0x…28` (OptimismSuperchainERC20 Factory/Beacon,
SuperchainTokenBridge) were removed from `Predeploys.sol` in optimism PR #19999
(2026-04-16, audit finding #25 in optimism issue #20078) and are not in the
Lagoon bundle. **[V]** Interop implementations are installed only on chains whose
dependency set has two or more members (`L2ContractsManager`,
`isFeatureEnabled(INTEROP)`).

### Event grammar (current; `topic0` recomputed with `cast keccak`)

| Contract | Event | `topic0` |
|---|---|---|
| CrossL2Inbox | `ExecutingMessage(bytes32 indexed msgHash, Identifier id)` | `0x5c37832d2e8d10e346e55ad62071a6a2f9fa5130614ef2ec6617555c6f467ba7` |
| L2ToL2CDM | `SentMessage(uint256 indexed destination, address indexed target, uint256 indexed messageNonce, address sender, bytes message)` | `0x382409ac69001e11931a28435afef442cbfd20d9891907e8fa373ba7d351f320` |
| L2ToL2CDM | `RelayedMessage(uint256 indexed source, uint256 indexed messageNonce, bytes32 indexed messageHash, bytes32 returnDataHash)` | `0xc270d73e26d2d39dee7ef92093555927e344e243415547ecc350b2b5385b68a2` |
| L2ToL2CDM | `MessageExpired(bytes32 indexed messageHash, uint256 undeliveredAt)` | `0x1eeb4f50372d18bdf2560c875fcaf19adee0adbe82a4242b47cd3ec5a3f3c32d` |
| UndeliveredMessageExporter | `UndeliveredMessageExported(bytes32 indexed messageHash, uint256 indexed source, address sourceMessenger, uint256 undeliveredAt)` | `0x1f8c424adc6dfc9721173830dfcfcc32cfa4495584f4e8eda281baddc12bfc6d` |
| SuperchainETHBridge | `SendETH(address indexed from, address indexed to, uint256 amount, uint256 destination)` | `0xed98a2ff78833375c368471a747cdf0633024dde3f870feb08a934ac5be83402` |
| SuperchainETHBridge | `RelayETH(address indexed from, address indexed to, uint256 amount, uint256 source)` | `0xe5479bb8ebad3b9ac81f55f424a6289cf0a54ff2641708f41dcb2b26f264d359` |
| SuperchainETHBridge | `RefundETH(address indexed from, uint256 amount, bytes32 indexed messageHash)` | `0x505de91519e0cf74e6679d1c1cff0321504bf8ad91b6af9be0b30f0150edad95` |
| ETHLiquidity | `LiquidityBurned(address indexed caller, uint256 value)` | `0x875e07afd7ce17c6531b1a6b7b34829dcd8b7e6639448afbd6a8e29fa1422b82` |
| ETHLiquidity | `LiquidityMinted(address indexed caller, uint256 value)` | `0x85719716ac5bd2744ae7ed3d16702129383049b97123b506320e7a5826ebbbba` |

Historical (devnet/pre-Lagoon only, never on a public network): 3-argument
`RelayedMessage(uint256,uint256,bytes32)` = `0x59480765…a379` (until v1.1.0;
`returnDataHash` added in v1.2.0); `FailedRelayedMessage` (dropped in 2024);
`resendMessage` re-emitting `SentMessage` (v1.1.0–1.3.2, removed in v2.0.0,
optimism PR #23286); SuperchainETHBridge was named `SuperchainWETH` until
2025-04 (same address, same events). Spec-only names `SentERC20`/`RelayedERC20`
never existed in code (code had `SendERC20`/`RelayERC20`).

### `Identifier` and the two hashes

```solidity
struct Identifier { address origin; uint256 blockNumber; uint256 logIndex; uint256 timestamp; uint256 chainId; }
```

- `origin`: the contract that emitted the source log. `blockNumber`: the source
  block. `logIndex`: **0-based index across all logs of the source block**
  (deposits included, not per-transaction). `timestamp`: the source block's
  timestamp. `chainId`: the source chain. Bounds: block and timestamp `< 2^64`,
  `logIndex < 2^32`.
- `ExecutingMessage.msgHash = keccak256(topic0‖topic1‖…‖data)` of the source
  log, the **payload hash**.
- Messenger `messageHash = keccak256(abi.encode(destination, source, nonce,
  sender, target, message))`, the **message hash**. It is not emitted in
  `SentMessage`; compute it on the source side. `RelayedMessage`,
  `MessageExpired`, `UndeliveredMessageExported` and `RefundETH` carry it.
- The payload hash of a `SentMessage` log is computable from the indexed
  `SentMessage` alone. It does not include the source chain ID, so join on
  `(id.chainId, payloadHash)`.

### Message identity

- **[V]** Nonce: one `uint240 msgNonce` per source messenger, incremented on
  every `sendMessage`, **global across destinations**. It is exposed versioned:
  `(version << 240) | msgNonce`, version currently 0.
- **[V]** Unique key: `(source_chain_id, nonce)` ≡ `messageHash` (which also
  commits to the destination). `SentMessage` is emitted at most once per hash.
- **[V]** `sender` is the `msg.sender` of `sendMessage`. For ETH transfers it is
  `SuperchainETHBridge` itself (sender = target = `0x…24`); the user is inside
  the `relayETH(from, to, amount)` payload and in `SendETH`.

### Dependency set

- **[V]** A set of chain IDs per cluster, configured in client software and
  sourced from the superchain-registry (`[interop] dependencies`). It is a
  mesh; registry CI requires identical sets for all members. Chains can be
  added only by a network upgrade and can never be removed. Chain IDs fit in
  `uint64`. The proof system caps a set at 127 chains.
- **[V]** A chain is always in its own set, so same-chain reads are legal.
- **[I]** L1 and non-OP chains cannot be members: L1→L2 deposits cannot execute
  messages, and membership needs a shared ETHLockbox and dispute-game factory.

### Safety labels

- **[V]** `unsafe` (`latest`) → cross-unsafe (internal, not exposed over RPC) →
  `safe` (cross-safe) → `finalized`. Executing messages in unsafe blocks are not
  verified. A block is cross-safe only when everything it depends on is safe.
- **[V]** `safe` is cross-checked only when the RPC's op-node runs as a Light CL
  following an op-supernode. Otherwise "the op-node's safe head advances past
  the activation block without cross-chain validation" (interop-prep notice).

## Step-by-Step Flow

### Messenger message (push)

1. **Source, any caller → `sendMessage(destination, target, message)`**: not
   payable. The only check is `destination != block.chainid`, so messages to
   arbitrary or non-member chain IDs are accepted and can never be relayed. It
   stores `sentMessages`/`sentMessageTimestamps` and emits `SentMessage`.
2. **Destination, anyone → `relayMessage(Identifier, sentMessage)`**: payable
   and `nonReentrant`. It checks `origin == 0x…23` and calls
   `CrossL2Inbox.validateMessage`, which emits `ExecutingMessage`. It decodes
   the payload, checks `destination == chainid`, rejects unsafe targets
   (L2CrossDomainMessenger, L2ToL1MessagePasser), checks
   `!successfulMessages[hash]`, sets it, and calls `target`. Target logs follow.
   Then it emits `RelayedMessage(source, nonce, messageHash, returnDataHash)`.
3. **Log order in a successful relay tx**: `ExecutingMessage` (`0x…22`) →
   target logs → `RelayedMessage` (`0x…23`). Relays never nest; a multicall
   holds several sequential groups.
4. A target with no code counts as a successful relay and consumes the message.

### ETH transfer

- **Source** (`sendETH{value}(to, chainId)`): `LiquidityBurned` →
  `SentMessage(dest, target = 0x…24, nonce, sender = 0x…24,
  relayETH(from, to, amount))` → `SendETH(from, to, amount, destination)`.
- **Destination** (inside the relay): `ExecutingMessage` → `LiquidityMinted` →
  `RelayETH(from, to, amount, source)` → `RelayedMessage`.
- `SendETH`/`RelayETH` carry **neither nonce nor hash**. Link them through the
  co-located `SentMessage`/`RelayedMessage` in the same transaction, or decode
  `relayETH(...)` from `SentMessage.message`. Amount is identical on both sides,
  a mirror transfer. The recipient is paid via `SafeSend` (no code execution).
- While `ETHLiquidity` is paused (mirroring the L1 pause), send, relay and
  refund all revert; relays can be retried after unpause.

### Failed relay

A target revert bubbles up and reverts the whole relay, `ExecutingMessage`
included. There is no event and no state change. Anyone can retry until expiry.
Detecting attempts needs transaction/receipt or trace scanning, and a filter on
`tx.to == 0x…23` misses relays made through wrappers (multicall, interop-lib
`GasTank`).

### Expiry and refund

1. After `sentAt + expiryPeriod` (production `expiryPeriod` = 8 days;
   protocol `EXPIRY_WINDOW` for executing messages = 7 days; test dependency
   sets may override the window), anyone calls
   `UndeliveredMessageExporter.exportUndeliveredMessage(...)` on the
   **destination**. It reverts if the message was relayed, and emits
   `UndeliveredMessageExported(messageHash, source, sourceMessenger,
   undeliveredAt)`. It can be forced via a deposit.
2. The L2→L1 withdrawal is proven and finalized. The source chain's
   `L1CrossDomainMessenger.relayUndeliveredMessage` deposits `expireMessage`
   into the source.
3. **Source**: `expireMessage` requires `undeliveredAt > sentAt +
   expiryPeriod`, sets `expiredMessages[hash]`, and emits
   `MessageExpired(messageHash, undeliveredAt)`. A message is never both relayed
   and expired.
4. **ETH only**: the original `sendETH` caller calls `refundETH(destination,
   nonce, to, amount)` once, getting `LiquidityMinted` → `RefundETH(from, amount,
   messageHash)`.

Earliest observable expiry on the source is 8 days plus the destination's
withdrawal proof/finalization delay. Messages without a recorded send timestamp
never expire; messages to non-member chains stay pending forever.

### Pull read (reference; phase 2)

- **[V]** `validateMessage` on-chain only rejects deposits, range-checks the
  `Identifier`, and requires the type-3 checksum slot to be warm, which only an
  EIP-2930 access-list entry under `0x…22` can achieve. Then it emits. The
  existence, origin, payload and timestamp of the source log, dependency-set
  membership, the 7-day window and cycles are all enforced **out of protocol**,
  by the sequencer/tx-pool and op-supernode. An invalid block is replaced with
  a deposits-only block.
- **[V]** There is no replay protection: one log can be executed many times in
  one transaction (N identical `ExecutingMessage`s), across transactions, and on
  every chain that has the source in its set.
- **[V]** The event does not carry the consumer. Only a call trace (the frame
  calling `validateMessage`, selector `0xab4d6f75`) identifies it. `tx.to` is
  often a wrapper, and EOAs call `0x…22` directly in OP's own tests.
- **[V]** Source resolution (spec reference algorithm, also used by
  op-interop-mon): `eth_getLogs(address = origin, block = id.blockNumber)`, pick
  the log whose **`logIndex` field** equals `id.logIndex` (not the array
  position), check `keccak(topics‖data) == msgHash` and block timestamp ==
  `id.timestamp`. Resolve number → hash first to avoid racing a reorg. The
  source RPC needs ≥ 7 days of receipts.
- **[V]** In Lagoon, only the messenger calls `validateMessage` among the
  predeploys. Known pull-model code is limited to supersim demos (TicTacToe,
  Contests), interop-lib `Promise`/`GasTank` (last commit 2025-07-10), and the
  test `EventLogger`. **[R]** No production app is announced.

## Invariants

- **[V]** One `SentMessage` per `messageHash`; at most one successful
  `RelayedMessage` per `messageHash` per destination.
- **[V]** Relayed and expired are mutually exclusive.
- **[V]** Every successful relay transaction contains, for each relay, an
  `ExecutingMessage` with `id.origin == 0x…23` before its `RelayedMessage`.
- **[V]** `init_ts ≤ exec_ts ≤ init_ts + 604800` for every valid executing
  message; logs in the activation block are never valid initiating messages.
- **[I]** Either side may be ingested first: chains are scanned independently,
  and same-timestamp execution is allowed.
- **[V]** Deposits can never execute messages, so every `ExecutingMessage` sits
  in a user transaction.

## Architecture Fit

### Maps directly

- **Two-sided message in `MessageBuffer`.** `SentMessage` creates or updates
  the source half; `RelayedMessage` the destination half; arrival order is free.
  Structurally this is Teleporter send/receive.
- **Key.** `messageHash` (32 bytes) goes in `crosschain_messages.native_id`;
  `Key.message_id` is a keccak prefix to `i64`, like `xdai::key_from_native_id`.
  `(source, nonce)` from `RelayedMessage` and the hash computed on the source
  side agree. No new message columns are needed.
- **Plain EVM chain IDs.** No Avalanche-style blockchain-ID resolution.
- **Same addresses on every member.** The config is the chain list plus five
  fixed contracts. Sends to non-configured chains fall under the existing
  `process_unknown_chains` gate.
- **ETH transfer.** One `crosschain_transfers` row per ETH message: native
  token, `src_amount == dst_amount` (mirror linkage), sender/recipient from
  `SendETH`/`RelayETH` or the `relayETH` payload. Correlating with the
  messenger event uses the existing transaction grouping
  (`indexer/evm/transaction_grouping.rs`), as ICTT does with Teleporter.
- **Relay attribute.** The `ExecutingMessage` paired with a relay gives the
  exact source pointer (`chainId`, `blockNumber`, `logIndex`, `timestamp`); it
  fits in a `protocol_metadata` namespace.

### New or open

- **Status model.** No failure event exists. Terminal outcomes are relayed,
  expired (`MessageExpired`, 8+ days, on the source), refunded (ETH only), or
  never (non-member destination, never exported). `MessageStatus` has no
  `expired`; mapping it to `failed` loses the distinction. A purely time-based
  "expired" (as `multichain-aggregator` does) is a heuristic, not the on-chain
  outcome.
- **Late terminal events.** `UndeliveredMessageExported`, `MessageExpired` and
  `RefundETH` arrive days after the message may already be flushed. They are
  keyed by `messageHash` only and must update a stored row. This is the same
  class of problem as AMB/xDai late confirmations (ADR-014, ADR-015).
- **Finality.** `LogStream` follows `latest` with no reorg handling. Options:
  cap the realtime head at the `safe` tag (lag of minutes, needs a
  supernode-wired RPC), or accept `latest` and mark rows provisional until safe.
  Replacement happens at the same height with a new hash, so per-chain reorg
  handling suffices; no cross-chain invalidation logic is needed.
- **ABI churn.** Pin current signatures and use version windows per ADR-006 if
  pre-release ABIs ever matter. There is no public history to backfill:
  activation starts from zero.
- **Receipts on Lagoon chains.** Tx type `0x7D` and the `opGasRefund` receipt
  field are new. Whether `receipt_fetch.rs` (alloy `Ethereum` network types)
  decodes them must be checked if an OP indexer needs receipts. `eth_getLogs`
  is unaffected.
- **Stats semantics.** Messenger messages and (phase 2) pull reads must not be
  mixed in `stats_messages`. Either use separate bridges or a kind namespace in
  `protocol_metadata`.

### Pull reads (phase 2)

- One record per `ExecutingMessage` log, keyed by `(dst_chain_id, dst_tx_hash,
  dst_log_index)` → keccak → `i64`. Never key by `Identifier`, which
  legitimately repeats.
- From logs alone: `src_chain_id`, source block/logIndex/timestamp, `origin`
  (as sender), payload hash, destination tx. Not available without extra work:
  `src_tx_hash` and payload (source resolution, asynchronous enrichment), the
  consumer (traces), and application meaning.
- No lifecycle: a read exists or it does not, so no buffering is needed beyond
  enrichment.
- Discovery is cheap: `eth_getLogs(address = 0x…22, topic0 = ExecutingMessage)`.
  Always filter by address; anyone can emit the same `topic0`.
- Double-counting rule: an `ExecutingMessage` paired with a `RelayedMessage` in
  the same transaction belongs to that messenger message. Pair on
  `id.chainId == RelayedMessage.source` and `msgHash == payloadHash(SentMessage)`,
  taking the closest preceding one; do not pair by `tx.to`. Everything else is a
  pull read, including `origin == 0x…23` without a match (someone validated a
  `SentMessage` log directly; that does not deliver it).
- Same-chain reads (`id.chainId == dst`) and reads of chains outside the
  configured cluster are legitimate.
- Deferred because value at launch is low: no production users, and the data
  says only "chain B read log L of chain A".

### ERC-7802 Is Not Part of the Protocol

- **[V]** ERC-7802 (Draft, created 2024-10-30, largely OP Labs authors) is a
  bridge-agnostic mint/burn interface: `crosschainMint`/`crosschainBurn`,
  `CrosschainMint(address indexed to, uint256 amount, address indexed sender)`
  (`0xde22baff…ea04`), `CrosschainBurn(address indexed from, uint256 amount,
  address indexed sender)` (`0xb90795a6…1fd4`). `sender` is whichever bridge
  the token authorized (https://eips.ethereum.org/EIPS/eip-7802).
- **[V]** Lagoon gives it no role. Its only protocol caller,
  `SuperchainTokenBridge`, was deleted along with `SuperchainERC20` and the
  `IERC7802` interface in contracts-bedrock (optimism PR #19999). It is absent
  from the Lagoon bundle, absent from the current docs index, and the launch
  scope excludes ERC-20. Remnants: the stale spec page `token-bridging.md`
  (still linked from the Lagoon overview), orphan tutorial files
  `docs/public-docs/public/tutorials/CustomBridge.sol`/`InteropToken.sol` (an
  app bridge over the messenger using an ERC-7802 token), and interop-lib
  (unmaintained since 2025-07-10).
- **[V]** ERC-7802 events already occur on OP Mainnet from other bridges:
  `CrosschainBurn` from USDT0 (`0x01bf…1071`) with sender `0xF03b…A0aD`
  (implementation `OUpgradeable`, the LayerZero OFT), 7 logs in blocks
  159387942–159389942. No ERC-7802 logs on Unichain in blocks
  63629319–63630318.
- **[I]** If an app bridge over the messenger uses an ERC-7802 token, the only
  attribution to OP interop is co-location: `CrosschainBurn` with `SentMessage`
  in the source transaction, `CrosschainMint` inside the relay transaction
  between `ExecutingMessage` and `RelayedMessage`. A topic-wide scan
  (no address filter) would mostly catch unrelated bridges. Revisit when OP
  ships its announced "broader token support".

## Prior Art: Blockscout

### Elixir backend (`blockscout/blockscout`)

- `apps/indexer/lib/indexer/fetcher/optimism/interop/`: `message.ex`,
  `message_failed.ex`, `message_queue.ex`, `multichain_export.ex`. Schema
  `op_interop_messages`, primary key **`(nonce, init_chain_id)`**; the message
  hash is never stored. API `GET /api/v2/optimism/interop/messages` with
  statuses `Sent`/`Relayed`/`Failed`; no expiry.
- **Each instance indexes only its own chain** and exchanges the other half
  with peer instances (`MessageQueue`): a signed `POST
  /api/v2/import/optimism/interop/` every 3 s per pending message, with peer
  URLs from Chainscout and public keys fetched over HTTP. A second channel,
  `MultichainExport`, pushes halves to blockscout-rs `multichain-aggregator`.
- **[V] It is stale.** It still filters `RelayedMessage` on the old
  `0x59480765…` `topic0` (`message.ex:48`), so it would see no relays on
  Lagoon contracts. `MessageFailed` decodes only direct `tx.to == 0x…23` calls.
  Chain IDs are `int4`. The import route was broken from 2026-01 to 2026-10
  unnoticed (blockscout issue #14882). The frontend glossary marks the feature
  **Deprecated**. No live instance enables it: on `unichain.blockscout.com` and
  OP explorers the list is empty and `/public-key` reports no key.
- Payload decoding recognizes `relayETH` and the old `relayERC20` selectors
  only.

### `multichain-aggregator` (this monorepo)

- `multichain-aggregator-migration/src/m20250427_051405_add_interop_messages.rs`:
  tables `interop_messages` (`UNIQUE (nonce, init_chain_id)`) and
  `interop_messages_transfers`. Statuses `PENDING/FAILED/SUCCESS/EXPIRED`,
  where `EXPIRED` = no relay and `timestamp + 7 days < now`
  (`multichain-aggregator-logic/src/types/interop_messages.rs`). Fed only by
  Blockscout `MultichainExport`; the production `interop` cluster does not
  exist. Last interop change 2025-08.

### Lessons

1. One indexer reading every chain of the set removes the whole peer-exchange
   protocol. That is the interchain-indexer model.
2. Store the messenger `messageHash` alongside `(source, nonce)`: expiry and
   refund events carry only the hash.
3. Pin the current ABI and alert on "sends with no relay past the window"; the
   stale `topic0` went unnoticed for over a year.
4. Statuses must be monotonic and order-independent; a successful relay beats
   any earlier observation.
5. Use wide types: chain ID `u64`, nonce `u256` (versioned top bits).

## Failure Modes / Observability

- **Pending forever.** Destination is not a member, or the message is
  never exported. Only a time-based metric (sends with no relay after the
  window) surfaces it.
- **Silent loss of relays.** A wrong `RelayedMessage` `topic0` makes every
  message look pending. Detect by comparing relay counts against
  `ExecutingMessage` counts with `origin == 0x…23`.
- **Unsafe replacement.** A row indexed from a block later replaced by a
  deposits-only block becomes a phantom relay or read. Indexing at `safe`
  avoids it; at `latest`, it needs reorg detection (parent-hash continuity, as
  op-interop-mon does).
- **RPC not supernode-wired.** `safe` advances without cross-chain checks; the
  index looks consistent but carries no cross-chain guarantee.
- **Late events without a row.** `MessageExpired` or `RefundETH` for a message
  not indexed (before the scan floor) must not create phantom messages; same
  rule as ADR-015.

## Edge Cases / Gotchas

- `SentMessage.sender` for ETH transfers is `0x…24`, not the user.
- `sendMessage` accepts any destination except itself; ETH sent to a non-member
  chain is lost until expiry and refund (spec aSEB-004).
- The nonce is global per source, so per-destination counts have gaps.
- `ExecutingMessage.msgHash` ≠ `RelayedMessage.messageHash`.
- `logIndex` in `Identifier` is block-level; after an address-filtered
  `eth_getLogs`, select by the field, not the array position.
- Public Unichain RPC caps `eth_getLogs` at 1,000 blocks.
- `0x4200…0030` on Base is `BaseTime`.
- Docs inconsistencies: two mainnet dates; the predeploy reference still lists
  `SuperchainTokenBridge`; the reading-logs guide says the contract checks log
  existence; the explainer says sets need not be symmetric, while spec and
  registry enforce a mesh.

## Change Triggers

Update this note when:

- `lagoon_time` and `[interop] dependencies` land in the superchain-registry,
  or the testnet (target 2026-12-03) / mainnet activation happens or slips;
- the governance activation proposal is posted or voted;
- release contract versions differ from `CrossL2Inbox` 1.1.0 /
  `L2ToL2CrossDomainMessenger` 2.1.0 / `SuperchainETHBridge` 2.0.0, or the
  announced `ETHLiquidity` `burn`→`deposit` / `mint`→`withdraw` rename lands;
- OP announces token support (a new canonical ERC-20 bridge would turn the
  ERC-7802 section into a target);
- new chains join a set, or a second set appears;
- an OP interop indexer is implemented here (replace proposals with the code);
- production pull-model apps appear (re-evaluate phase 2).

## Open Questions

- Which mainnet date holds (2026-12-15 vs 2027-01-11), and does the testnet
  date hold, given no registry timestamps or activation releases exist yet?
- Will public RPC endpoints for OP Mainnet/Unichain be supernode-wired Light
  CLs (trustworthy `safe`), and will they expose `eth_getBlockReceipts` and
  `debug_traceTransaction` (callTracer) for source resolution and consumer
  attribution?
- Does `receipt_fetch.rs` decode Lagoon receipts (tx type `0x7D`,
  `opGasRefund`) with alloy's `Ethereum` network types?
- Status model: add `expired` to `MessageStatus`, or keep terminal outcome in
  `protocol_metadata`? How to surface "relay window passed, not yet exported"?
- Messenger messages and pull reads: one bridge with a kind namespace, or two
  bridges over the same chains?
- Will the production dependency set override the 7-day expiry window?
- Will testnet traffic be dominated by OP test calls (EOA-direct
  `validateMessage`, `EventLogger`), skewing any early pull-read volume?
