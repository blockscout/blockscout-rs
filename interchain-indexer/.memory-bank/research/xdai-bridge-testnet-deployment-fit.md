# xDai Bridge on Sepolia ↔ Chiado: Testnet Deployment and Indexing Fit

## Scope

Whether the classic xDai bridge (`ForeignBridgeErcToNative` /
`HomeBridgeErcToNative`, ERC20-locked-on-Ethereum ↔ native-minted-on-Gnosis)
exists on the **Sepolia (11155111) ↔ Chiado (10200)** testnet pair, what its
contract and upgrade history is, and whether the existing xDai indexer
(`interchain-indexer-logic/src/indexer/xdai/`) could index it by configuration
alone or needs new protocol grammar in code.

Companion to `xdai-bridge-protocol-and-indexing-fit.md`, which documents the
**mainnet** (Ethereum 1 ↔ Gnosis 100) deployment. That note is the protocol
primer; this one only records where testnet *differs*. Anything not
contradicted here is assumed to hold as documented there.

Out of scope: the AMB/Omnibridge testnet deployment already in
`config/full-testnet/bridges.json` (`bridge_id = 1001`), the historical
Kovan ↔ Sokol testnet pair (Sokol is retired; not investigated), and any
decision to actually enable an xDai testnet bridge.

Status: **implemented.** The testnet bridge is configured as `bridge_id` `1003`
in `config/full-testnet/bridges.json`, and `indexer/xdai/version.rs` now carries
the Sepolia/Chiado grammar windows beside the mainnet ones. Sections below that
describe the pre-implementation state are marked *(historical)*; *The
Code-vs-Config Bottom Line* records what was actually built and why it differs
from what this note originally proposed.

## Short Answer

**A classic xDai erc-to-native bridge does exist on Sepolia ↔ Chiado.** It is a
real, low-traffic deployment: 4 nonce-era Ethereum→Gnosis deposits and ~25
legacy-era ones, a single validator, and a mock 18-decimal `DAI` token minted
for the purpose. Both sides are `EternalStorageProxy` and both implementations
are the upstream `tokenbridge-contracts` sources.

| Side | Chain | Proxy |
|---|---|---|
| Foreign | Sepolia (11155111) | `0x180Ff98e734415Ecd35faC3d32940e1B45FaD0A2` |
| Home | Chiado (10200) | `0xccA0Dc2A058884e62082312F09541cC7566406f0` |

The **current** windows' event grammar is byte-for-byte the mainnet grammar:
every `topic0` the indexer asserts in `FOREIGN_CANONICAL_TOPICS`,
`HOME_V6_CANONICAL_TOPICS` and `HOME_V7_CANONICAL_TOPICS` is reproduced exactly
on testnet, and the message blob is `Len104` on both verified implementations.
So `assert_canonical_topics` would pass.

Everything *around* the grammar **was** mainnet-specific and hardcoded, and all
of it rejected testnet at the time of writing *(historical — see *The
Code-vs-Config Bottom Line*)*:

1. `FOREIGN_EPOCH_FLOOR_BLOCK = 22_273_407` and
   `HOME_EPOCH_FLOOR_BLOCK = 39_569_937` are mainnet block numbers. The testnet
   equivalents are Sepolia **8_239_484** and Chiado **20_553_827** (or
   **15_562_365**, see below). `abi.rs`'s `ensure!(started_at_block >= floor)`
   rejects any testnet config outright.
2. `legacy_ethereum_asset` bails for any Ethereum block below
   `LEGACY_DAI_EPOCH_START_BLOCK = 9_161_003`, so
   `assert_epoch_boundaries_agree` fails for the Sepolia Foreign window even
   before the floor check is reached.
3. `source_asset` is the mainnet DAI/USDS address pair. Testnet has **one**
   token for its whole life, a mock `DAI` at
   `0x084Ab2ef1cb3A75EB0fDd81636e9A95D15629c37`; there is no USDS and no
   reserve flip. `check_source_asset_matches_latest` would log `error` and pin
   `XDAI_SOURCE_ASSET_MISMATCH = 1` forever (non-fatal).
4. `grammar_for` accepts Foreign 9/10 and Home 6/7. The testnet proxies'
   `version()` counters read **2** and **3**. (This is cosmetic — see
   *Version numbering* — but it is a real mismatch if the config mirrors the
   chain.)

**Bottom line: no new protocol grammar is needed for the current epoch — only
a second set of deployment constants beside the mainnet ones.** No new `XDaiVersion` variant, no new
`XDaiGrammar`, no new `BlobLayout`, no `IdentityStrategy` arm is required to
index the testnet bridge from its 2025-05-02 upgrade onward. Indexing the
*pre-2025-05* testnet history would need new grammar in code, because the v1
implementations on both sides emit **two-argument** source events with
`topic0`s the grammar table does not contain.

One genuinely new hazard, which mainnet does not have: on testnet the
destination `bytes32` **interleaves** nonce and source-transaction-hash
semantics within a single implementation window, because that field is whatever
the oracle passes to `executeAffirmation`, not something the contract derives.
No block-based floor can separate the two eras on Chiado. Evidence and
consequences are in *The Home-side identity epoch does not exist on testnet*.

## Why This Matters

`config/full-testnet/bridges.json` today only has AMB/Omnibridge. If someone
adds an xDai section by copying the mainnet one and swapping addresses, the
service will refuse to start that bridge with a confusing epoch-floor error
about mainnet block 22273407 — and once the floors are made configurable, the
next trap is silent: duplicated messages from the interleaved identity, and a
permanently-red `source_asset` mismatch metric.

It also matters as a design signal. The existence of a testnet deployment whose
only obstacle is three mainnet constants is what forced those constants to
become *per deployment*. They stayed in `indexer/xdai/version.rs` rather than
moving to `bridges.json`: every block boundary they express is a protocol fact
about a fixed, already-deployed contract set, and restating it in config would
create a second source of truth for the same number — the exact divergence
`assert_epoch_boundaries_agree` had to be written to catch.

## Source-of-Truth Files

### On chain

All read through Blockscout (`eth-sepolia.blockscout.com` /
`gnosis-chiado.blockscout.com`) on 2026-09-18. Every address below was
confirmed to hold matching bytecode and to answer the expected getters; none is
taken from documentation alone.

| Role | Chain | Address | Verified? |
|---|---|---|---|
| Foreign proxy (`EternalStorageProxy`) | Sepolia | `0x180Ff98e734415Ecd35faC3d32940e1B45FaD0A2` | yes |
| Foreign impl v1 | Sepolia | `0xc158633Aa217119436e6Cb4a7EB070BeD2EF2b67` | **no** (unverified source) |
| Foreign impl v2 (`XDaiForeignBridge`) | Sepolia | `0xa54349C84017566aFECB8AE818adc801138c6515` | yes (Sourcify full match, solc 0.4.24) |
| Foreign validator management | Sepolia | `0x3Ea1A9f92A99bC8e820541E7bed5d1F2419fFe59` | read back from `validatorContract()` |
| Mock DAI (`erc20token()`) | Sepolia | `0x084Ab2ef1cb3A75EB0fDd81636e9A95D15629c37` | yes, ERC-20 "Dai Stablecoin"/DAI, 18 dp |
| Home proxy (`EternalStorageProxy`) | Chiado | `0xccA0Dc2A058884e62082312F09541cC7566406f0` | yes |
| Home impl v1 | Chiado | `0x01aAd89DDda4e256885407DA316DBb724C6e448D` | **no** |
| Home impl v2 (`HomeBridgeErcToNative`) | Chiado | `0xbf7E72842A880a83B7EbC5C7D4536FAEE8b1Ac1c` | yes |
| Home impl v3 (current) | Chiado | `0x3F218F9A539da3A1262a1619404D1910a853CAdB` | **no** |
| Home validator management | Chiado | `0x138190e157d7604B8f89637AA10508Abd4c673B2` | read back from `validatorContract()`; `requiredSignatures() = 1` |
| Block reward (mints xDAI) | Chiado | `0x2000000000000000000000000000000000000001` | read back from `blockRewardContract()` (system address) |
| `Erc20ToNativeBridgeHelper` | Chiado | `0x9866D9d242Ac9D7EC4AC56ce61D0d957A02FD8e2` | yes |

Both proxy addresses and both validator addresses match the Gnosis Chain docs'
"Sepolia – Chiado" section of
<https://docs.gnosischain.com/bridges/About%20Token%20Bridges/xdai-bridge>;
the docs were the starting point, the chain is the evidence.

No `BridgeRouter`, no `XDaiBridgePeripheral`, no sDAI/sUSDS connector target
and no USDS deposit contract exists on testnet — the Foreign v2 implementation
*contains* `SavingsDaiConnector`, but there is nothing for it to point at and no
yield leg was observed.

### In this repo

- `interchain-indexer-logic/src/indexer/xdai/version.rs` — the grammar table
  and the four mainnet constants this note is about.
- `interchain-indexer-logic/src/indexer/xdai/abi.rs` — `AbiRegistry::from_chains`
  (`assert_canonical_topics`, the epoch-floor `ensure!`,
  `assert_epoch_boundaries_agree`).
- `interchain-indexer-logic/src/indexer/xdai/types.rs` — `MessageIdentity`,
  whose `destination()` is the nonce-vs-hash classifier.
- `interchain-indexer-logic/src/indexer/xdai/indexer.rs` —
  `check_source_asset_matches_latest`.
- `config/xdai/bridges.json` — the mainnet config this was modelled on.
  **Unchanged**: the schema did not move, so it needed no migration.
- `config/full-testnet/bridges.json` — the applied config, `bridge_id` `1003`.
- `config/xdai/chains-testnet.json` / `config/xdai/bridges-testnet.json` — the
  xDai-only testnet pair, following the `config/omnibridge` precedent; the
  bridge entry is byte-identical to the `config/full-testnet` one.
- `config/full-testnet/ENVs.md` — the operator-facing form of the same, with
  the two floors' rationale inline.
- `.memory-bank/adr/013-xdai-multi-deployment-grammar.md` — the decision to keep
  these constants in code, keyed by chain id, rather than in `bridges.json`.

## Key Types / Tables / Contracts

### Version numbering: `version()` is a proxy counter, not a protocol version *(superseded — see *Version numbering — resolved*)*

`bridge_contracts.version` in this repo mirrors `EternalStorageProxy.version()`,
an upgrade counter that starts at 1 for each deployment. Measured:

| Proxy | `version()` |
|---|---|
| Ethereum Foreign `0x4aa4…5016` | 10 |
| Gnosis Home `0x7301…0AA6` | 7 |
| Sepolia Foreign `0x180F…D0A2` | **2** |
| Chiado Home `0xccA0…06f0` | **3** |

`getBridgeInterfacesVersion()` returns `6.1.0` on **all four** proxies, so it
cannot discriminate either. The number therefore carries no protocol meaning
across deployments: mainnet "Foreign v9" and Sepolia "Foreign v2" are the same
contract generation. Two readings follow, and the choice is a design decision
this note does not make:

- keep `version` faithful to the chain (2 / 3) and teach `grammar_for` that
  `(chain, version) → grammar` rather than `(side, version) → grammar`; or
- treat `version` as a grammar label and write 9 / 6 / 7 in the testnet config
  even though the proxies say 2 / 3. Nothing in the code reads the on-chain
  counter, so this works today — at the cost of a config that contradicts
  `EternalStorageProxy.version()`.

### Foreign side (Sepolia) — implementation history

Derived from the proxy's own `Upgraded(uint256 version, address indexed implementation)`
logs (`topic0 0x4289d619…`), read exhaustively; timestamps from
`get_block_info`.

| Window | Blocks | Impl | Source events emitted |
|---|---|---|---|
| v1 | 5339499 – 8239483 (2024-02-22 → 2025-05-02) | `0xc158633A…` (unverified) | `UserRequestForAffirmation(address,uint256)` — `topic0 0x1d491a42…`, **no bytes32 at all**; `RelayedMessage(address,uint256,bytes32)` — `topic0 0x4ab7d581…`, bytes32 = Chiado source tx hash |
| v2 | 8239484 – (current) (2025-05-02 →) | `0xa54349C8…` `XDaiForeignBridge` | `UserRequestForAffirmation(address,uint256,bytes32)` — `topic0 0xf6968e68…`, bytes32 = nonce; `RelayedMessage(address,uint256,bytes32)` — `topic0 0x4ab7d581…` |

The proxy was created at Sepolia block 5339352, upgraded to v1 at 5339499 and
initialized at 5339646 (`DailyLimitChanged`, `ExecutionDailyLimitChanged`,
`GasPriceChanged`, `RequiredBlockConfirmationChanged`). The first
`UserRequestForAffirmation` of any shape is at **5907051**.

All four nonce-era deposits, exhaustively:

| Sepolia block | timestamp | tx | nonce | value |
|---|---|---|---|---|
| 8261069 | 2025-05-05T12:55:12Z | `0xd30a84a8…` | 0 | 0.1 |
| 8261098 | — | `0xb0ca9731…` | 1 | 0.02 |
| 9311522 | — | `0xa5559dce…` | 2 | 1 |
| 10573684 | 2026-04-02T09:34:24Z | `0x24a6e680…` | 3 | 11 |

Exactly **one** `RelayedMessage` exists over the whole history, at block
8029906 (v1 window), carrying
`0x066da6e9c920273913bbd8a11591601b21dbe9594e325c516eae535924afb326` — which is
the Chiado transaction at block 15068593 that emitted the v1
`UserRequestForSignature`. That is direct confirmation that the testnet legacy
era keys on the **source transaction hash**, exactly as mainnet did before
2025-04-15. No Gnosis→Ethereum message has been relayed since.

Foreign v2's `contracts/upgradeable_contracts/BasicForeignBridge.sol` is the
current upstream source, including the Hashi hooks and

```solidity
uint256 currentNonce = nonce();
setNonce(currentNonce + 1);
emit UserRequestForAffirmation(_receiver, _amount, bytes32(currentNonce));
```

`contracts/libraries/Message.sol` has `isMessageValid(_msg) => _msg.length == 104`
— **no 124-byte variant**, so the Foreign side is `BlobLayout::Len104`,
matching `FOREIGN_V9_GRAMMAR` / `FOREIGN_V10_GRAMMAR`.

### Home side (Chiado) — implementation history

| Window | Blocks | Impl | `UserRequestForSignature` shape |
|---|---|---|---|
| v1 | 7618689 – 15562364 (2024-01-02 → 2025-05-02) | `0x01aAd89D…` (unverified) | `(address,uint256)` — `topic0 0x127650bc…`, **no nonce field** |
| v2 | 15562365 – 20553826 (2025-05-02 → 2026-04-02) | `0xbf7E7284…` `HomeBridgeErcToNative` | `(address,uint256,bytes32)` — `topic0 0xbcb4ebd8…` |
| v3 | 20553827 – (current) (2026-04-02 →) | `0x3F218F9A…` (unverified) | `(address,uint256,bytes32,address)` — `topic0 0xe1e0bc4a…`, `token` = mock DAI |

The Foreign v2 and Home v2 upgrades are **44 seconds apart** (Sepolia
2025-05-02T10:14:36Z, Chiado 2025-05-02T10:15:20Z) — one coordinated
maintenance window, mirroring the mainnet nonce migration two weeks earlier.

`AffirmationCompleted`, `SignedForAffirmation`, `SignedForUserRequest` and
`CollectedSignatures` keep the **mainnet `topic0`s across all three windows** —
they were never reshaped. `CollectedSignatures.NumberOfCollectedSignatures` is
always 1 (`requiredSignatures() = 1`).

Home v2's `Message.sol` is identical to Foreign v2's, so
`isMessageValid == (length == 104)` → `BlobLayout::Len104`, matching
`HOME_V6_GRAMMAR`. Home v3's source is **unverified**; its blob layout is
therefore unverified. Inference, not evidence: since Sepolia Foreign v2 accepts
only 104-byte messages, a 124-byte Home v3 blob could never be executed, so
either Home v3 still emits 104 bytes or the Gnosis→Ethereum direction is
currently unexecutable on testnet. No Home v3 message has been signed or
relayed, so this cannot be settled from logs.

### Legacy deposits with no source event

Four `AffirmationCompleted` `bytes32` values found on Chiado have **no
corresponding bridge event anywhere in the Sepolia proxy's complete log
history**: `0x479d74bd…` (Chiado 16803580), `0x0633869d…` (10136848),
`0x1c39585f…` (10532994), `0x6459b1df…` (10016617). These are almost certainly
the testnet instance of the mainnet note's *plain ERC-20 transfer to the bridge*
path — deposits the validator honoured off-chain, keyed on the transaction hash
because nothing else exists. They exercise the `reconstruct_source` /
`decode_legacy_source_event` path in `events.rs` rather than normal
consolidation. Not individually traced to a Sepolia `Transfer`; recorded as
**likely, unverified**.

## Step-by-Step Flow

Identical to mainnet (see the companion note), with three differences worth
stating:

1. There is **one** validator, so a deposit completes in a single
   `SignedForAffirmation` + `AffirmationCompleted` transaction rather than
   accumulating signatures across blocks.
2. There is **no** `BridgeRouter` / `XDaiBridgePeripheral` indirection. Users
   call `relayTokens` on the proxy directly.
3. There is no DAI→USDS conversion on payout; `erc20token()` has returned the
   same mock DAI at every block probed (5400000 and `latest`).

## Correlation Against `version.rs`

### 1. Do the testnet `topic0`s match the grammar table?

**Yes, for every event in every current window.** Computed with
`cast keccak` and compared against the raw `topics[0]` observed on chain:

| Event | Canonical signature in `version.rs` | `topic0` | Seen on testnet |
|---|---|---|---|
| `UserRequestForAffirmation` | `(address,uint256,bytes32)` | `0xf6968e68…` | Sepolia, v2 window ✅ |
| `RelayedMessage` | `(address,uint256,bytes32)` | `0x4ab7d581…` | Sepolia, both windows ✅ |
| `UserRequestForSignature` (Home v6) | `(address,uint256,bytes32)` | `0xbcb4ebd8…` | Chiado, v2 window ✅ |
| `UserRequestForSignature` (Home v7) | `(address,uint256,bytes32,address)` | `0xe1e0bc4a…` | Chiado, v3 window ✅ |
| `AffirmationCompleted` | `(address,uint256,bytes32)` | `0x6fc115a8…` | Chiado, all windows ✅ |
| `SignedForAffirmation` | `(address,bytes32)` | `0x5df9cc3e…` | Chiado, all windows ✅ |
| `SignedForUserRequest` | `(address,bytes32)` | `0xbf06885f…` | Chiado, all windows ✅ |
| `CollectedSignatures` | `(address,bytes32,uint256)` | `0x41555740…` | Chiado, all windows ✅ |

Two testnet-only signatures are **not** in the table, both in the v1 windows:

| Event | Signature | `topic0` | Where |
|---|---|---|---|
| `UserRequestForAffirmation` | `(address,uint256)` | `0x1d491a42…` | Sepolia 5907051 – 8116450 |
| `UserRequestForSignature` | `(address,uint256)` | `0x127650bc…` | Chiado 9966043 – 15162218 |

`assert_canonical_topics` would reject an ABI containing either, which is the
correct behaviour — those windows carry no identity field at all and the
indexer has nothing to key on. Do not configure them.

### 2. Do the observed versions map onto `XDaiVersion`?

By *grammar*, yes, one-to-one:

| Testnet window | Equivalent mainnet grammar |
|---|---|
| Sepolia Foreign v2 | `ForeignV9` (`Len104`, nonce identity, single ERC-20 reserve) |
| Chiado Home v2 | `HomeV6` (3-arg `UserRequestForSignature`, `Len104`) |
| Chiado Home v3 | `HomeV7` (4-arg `UserRequestForSignature` with `token`) |

By *number*, no: `grammar_for(Foreign, 2)` and `grammar_for(Home, 2|3)` bail.
See *Version numbering* for the two ways out. Neither needs a new grammar
struct — only a mapping change or a config convention.

Sepolia Foreign v2 is the **v9-generation** contract (`SavingsDaiConnector`,
`swapSDAIToUSDS` absent from its behaviour on chain, `erc20token()` = DAI), not
the v10 USDS generation. There is no testnet `ForeignV10` analogue.

### 3. Blob layout

`Len104` on both verified implementations (Foreign v2, Home v2), read directly
from their `Message.sol`. Home v3 unverified; inferred `Len104` (see above).
`Len104Or124` is not needed for testnet.

### 4. Epoch floors — and why testnet does not have one *(the Home half is settled in *Home floor — resolved*)*

The Foreign side does have a clean floor: **Sepolia 8239484**. Before it there
is no nonce field in the source event at all; from it every
`UserRequestForAffirmation` carries a nonce (0, 1, 2, 3). Unambiguous.

The Home side does **not**. `AffirmationCompleted.bytes32` is whatever the
oracle passed to `executeAffirmation` — the contract echoes the caller's
argument and derives nothing — so the semantics are an *oracle-side convention*,
and on testnet it flip-flopped:

| Chiado block | date | `bytes32` | reading |
|---|---|---|---|
| 9791227 … 15273356 | 2024–2025 | full 32-byte values | source tx hash (v1 era) |
| 14147474 / 14147476 | — | `0x…1f` / `0x…1e` (31 / 30) | **neither** — small integers with no matching foreign nonce |
| 15612527 | 2025-05-05T12:56:55Z | `0x…00` | nonce 0 (103 s after the Sepolia deposit) |
| 15612593 | — | `0x…01` | nonce 1 |
| 16803580 | 2025-07-17T20:52:50Z | 4 × full hashes, incl. `0xd30a84a8…`, `0xb0ca9731…` | source tx hash — **re-affirming the same two deposits already affirmed by nonce** |
| 18042501 | — | `0xa5559dce…` | source tx hash (Sepolia nonce-2 deposit) |
| 20553477 | 2026-04-02 (pre-v3) | `0x24a6e680…` | source tx hash (Sepolia nonce-3 deposit) |
| 20706963 | post-v3 | `0x…03` | nonce 3 — **the same deposit again** |

Two consequences the mainnet model does not anticipate:

- **No block number separates the eras.** Any `HOME_EPOCH_FLOOR_BLOCK` at or
  below the Home v2 upgrade (15562365) admits hash-keyed affirmations at
  16803580, 18042501 and 20553477. Choosing the Home v3 upgrade (20553827)
  instead yields a clean nonce-only window, at the cost of leaving deposits
  0–2 permanently `Initiated` (their affirmations are below the floor).
- **The same deposit can be affirmed twice under two identities.** Sepolia
  nonce 0 and nonce 1 were affirmed at 15612527/15612593 by nonce and again at
  16803580 by hash; nonce 3 at 20553477 by hash and again at 20706963 by nonce.
  The contract's own dedup is `keccak(recipient‖value‖bytes32)`, which differs
  between the two forms, so both executions succeed on chain. An indexer
  spanning both would create **two `crosschain_messages` rows for one deposit**.

`MessageIdentity::destination` already classifies per event (`value <= u64::MAX`
→ `Nonce`, otherwise `SourceTransactionHash`), so it survives the interleaving
without erroring — but it misclassifies the `0x…1e` / `0x…1f` values as nonces
30 and 31, and it cannot detect the duplicates.

### 5. Source asset

One token for the whole history: mock `DAI`
`0x084Ab2ef1cb3A75EB0fDd81636e9A95D15629c37`, 18 decimals, 200 000 total
supply, deployed by the same developer address that deployed the bridge.
`erc20token()` returns it at Sepolia block 5400000 and at `latest`. **There is
no testnet DAI→USDS switch, no USDS, and no second window.**

That is precisely what the single-deployment model could not express.
`FOREIGN_V9_GRAMMAR` hardcoded mainnet DAI and `legacy_ethereum_asset`
hardcoded mainnet's `9_161_003` / `23_748_179`; on testnet the correct table is
a single row with no cutover at all. Both are now per deployment — the Sepolia
arm bails below `SEPOLIA_BRIDGE_CREATION_BLOCK` (5339352, the Foreign proxy's
creation block) and resolves `SEPOLIA_MOCK_DAI` everywhere above it.

`erc20token()` on `0x180F…D0A2` was re-read at `latest` on 2026-09-18 and still
returns `0x084Ab2ef1cb3A75EB0fDd81636e9A95D15629c37`.

## The Code-vs-Config Bottom Line

**Resolved: code, not config. No schema change.** `bridges.json` is unchanged
except for the new bridge entry itself.

The three items below were originally proposed as new `bridges.json` fields.
That was rejected: every one of them is a block boundary that `contracts[]`
already expresses, or a protocol fact about a fixed contract set, so declaring
them in config would state the same number twice. The constants instead became
**per deployment**, selected by chain id, and stayed in
`indexer/xdai/version.rs` beside the grammar they belong to.

### What was built

1. **Epoch floors hang off `XDaiGrammar`.** `FOREIGN_EPOCH_FLOOR_BLOCK` /
   `HOME_EPOCH_FLOOR_BLOCK` became `XDaiGrammar::epoch_floor_block`, so each
   version window declares the floor of the deployment it belongs to:
   `ETHEREUM_EPOCH_FLOOR_BLOCK` 22273407, `GNOSIS_EPOCH_FLOOR_BLOCK` 39569937,
   `SEPOLIA_EPOCH_FLOOR_BLOCK` 8239484, `CHIADO_EPOCH_FLOOR_BLOCK` 20553827.
   The `ensure!` in `AbiRegistry::from_chains` is unchanged apart from reading
   the floor off the grammar it just selected.

2. **`source_asset` gained a testnet Foreign window.**
   `SEPOLIA_FOREIGN_V2_GRAMMAR` carries `SEPOLIA_MOCK_DAI`
   (`0x084Ab2ef1cb3A75EB0fDd81636e9A95D15629c37`). `DAI`/`USDS` and the mainnet
   windows are untouched.

3. **`legacy_ethereum_asset` gained a `foreign_chain_id` first parameter** and
   a Sepolia arm. The mainnet arm is byte-for-byte what it was, including the
   `bail!` below `LEGACY_DAI_EPOCH_START_BLOCK` (9161003); the Sepolia arm
   bails below `SEPOLIA_BRIDGE_CREATION_BLOCK` (5339352) and resolves the mock
   DAI everywhere above it, in both directions. An unregistered Foreign chain
   id is an error.

   The companion `consolidation.rs` fallback — `source.event.token.unwrap_or(DAI)`,
   the Home v6 104-byte `parseMessage` default — became
   `legacy_home_ethereum_asset(chain_ids.foreign)`. It is deliberately **total**
   rather than fallible: `Consolidate::consolidate` runs inside the maintenance
   plan, where an `Err` aborts the whole bridge's cycle, so an unrecognised
   chain id falls back to DAI exactly as the pre-multi-deployment code did
   unconditionally.

4. **`grammar_for` is keyed on `(chain_id, side, version)`.** See *Version
   numbering* below, which this supersedes.

5. **`assert_epoch_boundaries_agree` stays**, now comparing config against the
   *deployment's own* asset table rather than against mainnet's.

6. **`check_source_asset_matches_latest` compares against the configured
   deployment's newest window**, so it is no longer guaranteed-wrong on
   testnet. Still non-fatal, for the reason documented at its definition.

### Version numbering — resolved

Write the proxies' **real** `version()` counters in the config: Sepolia Foreign
`2`, Chiado Home `3`. No contradiction with the chain, and nothing to remember.
This works because the counters restart per deployment and therefore *do not
collide* with mainnet's 9/10 and 6/7 — but for the same reason they are not a
key on their own, so `grammar_for` takes the chain id as well. A mainnet chain
id with `version: 2` is now a hard startup error that names every registered
deployment, instead of silently selecting the Sepolia floor (14M blocks too
low) and a `source_asset` that does not exist on Ethereum.

Re-verified directly on chain on 2026-09-18, not taken from this note:
`version()` returns `2` on `0x180F…D0A2` and `3` on `0xccA0…06f0`.

### Home floor — resolved at the v3 upgrade, 20553827

The decision *The Home-side identity epoch does not exist on testnet* leaves
open is settled in favour of the clean window, and verified by exhaustive
`eth_getLogs` over `AffirmationCompleted` rather than by argument:

- `[20553827, latest]` contains **exactly one** affirmation, at 20706963,
  `bytes32 = 0x…03` — nonce-keyed.
- `[20553477, 20553827)` contains **exactly one**, at 20553477,
  `bytes32 = 0x24a6e680…` — the hash-keyed re-affirmation of that *same*
  deposit.

So the floor sits strictly between the last hash-keyed affirmation and the only
later nonce-keyed one, and no deposit can produce two `crosschain_messages`
rows. The alternative floor at the Home v2 upgrade (15562365) would admit the
re-affirmations at 16803580, 18042501 and 20553477 and duplicate nonces 0, 1
and 3.

The cost is deliberate and is **not** papered over: Sepolia deposits with
nonces 0, 1 and 2 are above the Foreign floor but their affirmations are below
the Home floor, so they stay permanently `Initiated`. Raising the Foreign floor
to hide them was considered and rejected — no block between 9311522 and
10573684 is a protocol boundary, and inventing one to suppress honest
unfinalized rows is worse than showing them.

### Why the two sides' ranges differ by eleven months, and what blocks closing it

The Foreign side is indexed from 2025-05-02 and the Home side from 2026-04-02.
That asymmetry is a consequence of the oracle's behaviour, not an oversight,
and it cannot be closed by moving the floor. The two floors answer different
questions — Foreign: "from where is the source event *decodable*", Home: "from
where is the destination identity *unambiguous*" — and there is no reason they
would coincide.

Every `AffirmationCompleted` in `[15562365, latest]` (the range a Home v2 floor
would open), with its `bytes32` resolved to its Sepolia transaction and that
transaction's bridge-proxy log read directly. Blockscout was not used; these are
`eth_getLogs` / `eth_getTransactionReceipt` results, 2026-09-18:

| Chiado block | `bytes32` | Sepolia tx block | proxy log `topic0` | what it is |
|---|---|---|---|---|
| 15612527 | `0x…00` | — | — | nonce 0 |
| 15612593 | `0x…01` | — | — | nonce 1 |
| 16803580 | `0x479d74bd…` | 8116793 | **none** — tx sent to the token, only an ERC-20 `Transfer` | plain-transfer deposit |
| 16803580 | `0xb0ca9731…` | 8261098 | `0xf6968e68…` **modern 3-arg** | duplicate of nonce 1 |
| 16803580 | `0xd30a84a8…` | 8261069 | `0xf6968e68…` **modern 3-arg** | duplicate of nonce 0 |
| 16803580 | `0x906b0bef…` | 8116450 | `0x1d491a42…` legacy 2-arg | legacy deposit, below the Sepolia floor |
| 18042501 | `0xa5559dce…` | 9311522 | `0xf6968e68…` **modern 3-arg** | nonce 2 — no nonce-keyed twin |
| 20553477 | `0x24a6e680…` | 10573684 | `0xf6968e68…` **modern 3-arg** | duplicate of nonce 3 |
| 20706963 | `0x…03` | — | — | nonce 3 |

Three findings, in the order they bite:

1. **Four of the six hash-keyed affirmations would hard-error today, not
   duplicate.** Their source transactions are in the modern epoch and emit
   `UserRequestForAffirmation(address,uint256,bytes32)`.
   `decode_legacy_source_event` ends with an `ensure!` that rejects a source
   receipt carrying modern source-request grammar with no legacy event —
   "source receipt contains unsupported modern xDai source-request grammar".
   That error propagates out of `handle_affirmation_completed` into the batch
   result, so the block lands in the failure ledger and is retried forever.
   Lowering the floor without further work is therefore *worse* than the
   duplicate-row problem it was meant to avoid.

   The remaining two are the ones the existing legacy path exists for and must
   keep working: `0x479d74bd…` emits no bridge event at all (the
   plain-ERC-20-transfer deposit this note predicted, now confirmed), and
   `0x906b0bef…` emits the legacy two-argument event below the Sepolia floor.
   Both yield `Ok(None)` or a decoded legacy event, and neither trips the
   `ensure!`.

2. **Re-keying to the nonce fixes (1) but then collides with a different
   invariant.** When the `bytes32` is a source transaction hash *and* that
   transaction emits a modern nonce-bearing source event, the message's true
   identity is that nonce, so `fetch_reconstructed_source` — which already has
   the receipt — could re-derive the key. For nonce 2 that alone would be a
   clean win: it has no nonce-keyed twin, so it would simply complete.

   For nonces 0, 1 and 3 it would not. Each already has a nonce-keyed
   `AffirmationCompleted` in a **different transaction**, so re-keying makes two
   distinct `AnnotatedEvent<CompletionEvent>`s land on one buffer key, and
   `ensure_completion_compatible`'s "conflicting xDai completion payload"
   rejects the second. That is the same permanent-retry failure mode as (1),
   moved one step later.

3. **The underlying fact is not an indexing artifact.** The same deposit really
   was affirmed and paid out twice. Nonce 0's 0.1 deposit produced
   `AffirmationCompleted` in Chiado tx `0x50f4ed68…` (block 15612527, `bytes32
   = 0x…00`) and again in `0x2e50d68b…` (block 16803580, `bytes32 =
   0xd30a84a8…`), same recipient, same value. The contract's dedup is over
   `keccak(recipient‖value‖bytes32)`, which differs between the two identity
   forms, so both executions are valid on chain.

So closing the asymmetry is not a mechanical fix. It requires deciding which of
two genuine on-chain payouts is *the* completion of a message, and relaxing
`ensure_completion_compatible` — an invariant shared with mainnet whose job is
to catch indexing bugs — to accommodate a testnet oracle that double-paid. The
cost/benefit is poor: the gain is three more completed messages on a bridge with
four deposits; the risk is a weakened correctness check on the mainnet path.

**Decision: keep the Chiado floor at 20553827.** Revisit only if the same
double-affirmation pattern is ever observed on mainnet, which would make the
invariant change necessary on its own merits rather than as testnet
accommodation.

A useful consequence of this floor: the legacy reconstruction path is **entirely
dormant** in the shipped testnet config. The only in-range affirmation is
nonce-keyed, so `reconstruct_source` never runs; and the only `RelayedMessage`
in the whole Sepolia history is at block 8029906, below the Foreign floor, so
the Gno→Eth reconstruction added by `e1df12ee` never runs either. The `ensure!`
hazard in (1) is not live — the floor is what keeps it so.

Mainnet is untouched by all of this: no code path changed, and on mainnet a
hash-keyed destination event's source transaction is pre-epoch and emits the
legacy two-argument event, which is exactly the case
`decode_legacy_source_event` handles.

### New code is required only if you want pre-2025-05 history

Unchanged from the original conclusion. To index the testnet v1 windows you
would need genuinely new grammar:
`UserRequestForAffirmation(address,uint256)` /
`UserRequestForSignature(address,uint256)` carry **no identity field**, so a new
`XDaiVersion`, a new canonical-topic list and — crucially — a new
`IdentityStrategy` arm that derives identity from the source transaction hash
rather than the event. Deliberately not done.

There is also **no Chiado Home v2 grammar window**, for the same reason in
reverse: Home v2 lives entirely below `CHIADO_EPOCH_FLOOR_BLOCK`, so no config
can reference it without failing the floor check, and a grammar the config
cannot select is a claim the code cannot back.

### The applied config

`config/full-testnet/bridges.json`, `bridge_id` `1003` (`1001` is the AMB
bridge; mainnet xDai is `3`):

```jsonc
{
  "bridge_id": 1003,
  "name": "xDai Bridge (testnet)",
  "type": "xdai",
  "indexer_type": "xdai",
  "enabled": true,
  "contracts": [
    { "chain_id": 11155111, "address": "0x180Ff98e734415Ecd35faC3d32940e1B45FaD0A2",
      "version": 2, "started_at_block": 8239484 },
    { "chain_id": 10200, "address": "0xccA0Dc2A058884e62082312F09541cC7566406f0",
      "version": 3, "started_at_block": 20553827 }
  ]
}
```

ABIs are the mainnet ones verbatim, copied from `config/xdai/bridges.json`: the
Sepolia Foreign entry reuses the mainnet Foreign ABI and the Chiado entry
reuses the mainnet Home v7 ABI. The `topic0`s are identical, which
`assert_canonical_topics` enforces.

Both RPC endpoints already in `config/full-testnet/chains.json` were checked
against the blocks this config needs and serve them: `tenderly` returned the
nonce-0 deposit's log at Sepolia 8261069, and `gateway_archive` returned the
`AffirmationCompleted` at Chiado 20706963.

## Invariants

- Event `topic0`s are stable across the whole testnet history for the four
  Home-side confirmation events; only the two *source* events were reshaped,
  and only at the v1→v2 boundary on each side.
- The proxy address never changed on either side; all version windows share one
  address, exactly as in `config/xdai/bridges.json`.
- `erc20token()` on the Sepolia Foreign proxy has one value for all time.
- `requiredSignatures() = 1` on Chiado, so `CollectedSignatures.NumberOfCollectedSignatures`
  is always 1 and finality is single-transaction.

## Failure Modes / Observability

What the shipped configuration actually produces, and what breaks it.

Expected steady state, not a fault:

| Symptom | Cause |
|---|---|
| Exactly **one** finalized message on this bridge (Sepolia nonce 3) | the Chiado floor at 20553827 admits one affirmation; see *Home floor — resolved* |
| Sepolia deposits with nonces 0, 1 and 2 stuck at `Initiated`, forever | their affirmations (15612527, 15612593, 18042501) are below the Home floor. The accepted cost of not duplicating rows |
| No Gnosis→Ethereum message ever completes | none has been relayed since the v1 era; `CollectedSignatures` → `executeSignatures` is untested end to end on this pair |

Misconfiguration, all of which now fail at startup rather than silently:

| Symptom | Cause |
|---|---|
| `no xDai grammar registered for chain 11155111 side Foreign version 9` | a mainnet version number under a testnet chain id (or the reverse). The error lists every registered deployment |
| `… is below the Foreign epoch floor 8239484` | a Sepolia window below the nonce epoch. The error names *this deployment's* floor, not mainnet's |
| `… is below the Home epoch floor 20553827` | a Chiado window lowered toward the Home v2 upgrade — the duplicate-row trap. Do not "fix" this by lowering the constant |
| `xDai chain 11155111 Foreign version … declares source_asset … but the legacy reconstruction path resolves …` | `assert_epoch_boundaries_agree`, e.g. an env override shifting `started_at_block` across an asset boundary |
| `no xDai legacy asset table for Foreign chain N` | a third deployment configured without adding its constants to `version.rs` |

Runtime, non-fatal:

| Symptom | Cause |
|---|---|
| `XDAI_SOURCE_ASSET_MISMATCH{bridge_id} = 1` + `error` log, indexing continues | the Sepolia proxy's `erc20token()` stopped returning `SEPOLIA_MOCK_DAI` — i.e. the testnet bridge was upgraded and `version.rs` was not updated. This is now a real signal on testnet; before this change it was pinned at 1 unconditionally |
| Two messages for one deposit | only reachable by lowering the Chiado floor below 20553827 |
| Messages keyed on `native_id` 30 / 31 | `MessageIdentity::destination` reading the v1-era `0x…1e` / `0x…1f` (Chiado 14147474 / 14147476) as nonces. Both are far below the Home floor and therefore unreachable in the shipped config |

## Edge Cases / Gotchas

- **The Home-side `bytes32` is oracle-controlled, not contract-derived.** The
  mainnet note treats the 2025-04-15 switch as an epoch with a block boundary;
  that is true of mainnet's oracle deployment, not of the protocol. Testnet is
  the counterexample.
- **`getBridgeInterfacesVersion()` is `6.1.0` everywhere** and is useless for
  identifying a generation. Use the proxy's `Upgraded` log history plus the
  observed event signatures.
- **The Foreign v1 `RelayedMessage` shares its `topic0` with the current one**
  but its `bytes32` is a Chiado transaction hash, not a nonce — the same silent
  break mainnet had, reproduced on testnet at a different block.
- Testnet volume is tiny (4 nonce-era deposits, 1 relayed withdrawal ever). It
  is adequate as a smoke test of the Ethereum→Gnosis direction and **not**
  adequate to exercise Gnosis→Ethereum completion: no `RelayedMessage` has been
  emitted since the v1 era, so `CollectedSignatures` → `executeSignatures`
  finality is untested end-to-end on this pair.

## Change Triggers

Update this note when:

- either testnet proxy is upgraded again (watch `Upgraded` on both proxies);
- a third xDai deployment is added — it needs its own grammar windows, floor
  constants and asset constants in `version.rs`, not a schema change;
- the Chiado oracle emits another affirmation above 20553827 under the *hash*
  convention, which would invalidate the floor's no-duplicates guarantee;
- Chiado Home v3's source is verified, which would settle its blob layout;
- a Gnosis→Ethereum testnet withdrawal is relayed, which would exercise the
  path this note records as untested.

## Open Questions

Explicitly **not** established:

1. **Home impl v3 (`0x3F218F9A…`) and Foreign impl v1 (`0xc158633A…`) and Home
   impl v1 (`0x01aAd89D…`) are unverified.** Their behaviour is known only from
   emitted logs. Home v3's `Message.isMessageValid` length is inferred, not read.
2. **Why the Chiado oracle reverted to hash-keying on 2025-07-17** (block
   16803580) after two months of correct nonce-keying. No on-chain signal
   explains it; a rolled-back or re-synced oracle is a guess.
3. **What `0x…1e` / `0x…1f` (Chiado 14147474 / 14147476) actually identify.**
   They are not nonces (the Foreign side had none then) and not transaction
   hashes. Manual test affirmations is the working assumption.
4. **The four unmatched affirmation hashes** (`0x479d74bd…`, `0x0633869d…`,
   `0x1c39585f…`, `0x6459b1df…`) are assumed to be plain-ERC-20-transfer
   deposits honoured off-chain; not traced to their Sepolia `Transfer` logs.
5. **Whether a Kovan ↔ Sokol xDai bridge historically existed** and whether any
   of its state matters. Sokol is retired; not investigated.
6. **Whether the testnet bridge is actively maintained.** The v3 upgrade
   (2026-04-02) suggests yes, but there has been no user traffic since.
7. **Whether Chiado's `Upgraded` log history really places the v3 upgrade at
   20553827.** The block is taken from this note's original reading. It was
   *not* re-derived from `Upgraded` logs during implementation — what was
   re-verified is the property the floor actually has to have (no hash-keyed
   affirmation at or above it, one nonce-keyed affirmation above it), which is
   what the code depends on. If the upgrade block turns out to be slightly
   different, the floor is still correct for its purpose.
