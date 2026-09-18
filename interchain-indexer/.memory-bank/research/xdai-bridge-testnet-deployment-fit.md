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

Status: **investigation only.** `config/full-testnet/bridges.json` has not been
touched. The config sketch below is a proposal, not an applied change.

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

Everything *around* the grammar is mainnet-specific and hardcoded, and all of it
rejects testnet today:

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

**Bottom line: configuration alone is enough for the current epoch, once those
three mainnet constants become config.** No new `XDaiVersion` variant, no new
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
only obstacle is three mainnet constants is the concrete argument for moving
`FOREIGN_EPOCH_FLOOR_BLOCK`, `HOME_EPOCH_FLOOR_BLOCK` and the `source_asset`
table out of `indexer/xdai/version.rs` and into `bridges.json`.

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
- `config/xdai/bridges.json` — the mainnet config this would be modelled on.
- `config/full-testnet/bridges.json` — where an xDai section would go.
  **Untouched by this note.**

## Key Types / Tables / Contracts

### Version numbering: `version()` is a proxy counter, not a protocol version

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

### 4. Epoch floors — and why testnet does not have one

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

That is precisely what today's model cannot express. `FOREIGN_V9_GRAMMAR`
hardcodes mainnet DAI and `legacy_ethereum_asset` hardcodes mainnet's
`9_161_003` / `23_748_179`; on testnet the correct table is a single row with
no cutover at all.

## The Code-vs-Config Bottom Line

### Config alone suffices, once these three things become config

1. **`FOREIGN_EPOCH_FLOOR_BLOCK` / `HOME_EPOCH_FLOOR_BLOCK`** → per-bridge (or
   per-chain) fields in `bridges.json`. Mainnet keeps 22273407 / 39569937;
   testnet uses 8239484 / 20553827. The `ensure!` in `abi.rs` stays, it just
   compares against a configured floor. This is the single blocking change.
2. **The `source_asset` table** → a per-Foreign-window `source_asset` field in
   `bridges.json`, replacing `XDaiGrammar::source_asset`. On testnet every
   window carries the mock DAI; on mainnet v9 carries DAI and v10 carries USDS,
   unchanged. `check_source_asset_matches_latest` then compares the *configured*
   asset against `erc20token()` and stops being permanently red on testnet.
3. **`legacy_ethereum_asset` / `LEGACY_DAI_EPOCH_START_BLOCK` /
   `USDS_EPOCH_START_BLOCK`** → the legacy reconstruction path must consult the
   same configured window table instead of mainnet constants, and
   `assert_epoch_boundaries_agree` must compare config against config. Without
   this, the Sepolia window at block 8239484 fails the "no legacy Ethereum
   asset" bail before anything else runs.

Plus one small, non-blocking decision: how `version` is written for a
deployment whose proxy counter is 2/3 rather than 9/6/7 (see
*Version numbering*).

### New code is required only if you want pre-2025-05 history

To index the testnet v1 windows you would need genuinely new grammar:
`UserRequestForAffirmation(address,uint256)` / `UserRequestForSignature(address,uint256)`
carry **no identity field**, so a new `XDaiVersion`, a new canonical-topic list
and — crucially — a new `IdentityStrategy` arm that derives identity from the
source transaction hash rather than the event. That is the same shape of work
the mainnet note deferred for the pre-2019 Foreign v1/v2 era. **Recommendation:
do not; floor the testnet config at the 2025-05-02 upgrade, exactly as the
mainnet config floors at its own epoch.**

### Illustrative config (not applied)

```jsonc
// config/full-testnet/bridges.json — proposal only
{
  "bridge_id": 1003,
  "name": "xDai Bridge (testnet)",
  "type": "xdai",
  "indexer_type": "xdai",
  "enabled": true,
  "contracts": [
    { "chain_id": 11155111, "address": "0x180Ff98e734415Ecd35faC3d32940e1B45FaD0A2",
      "version": 9,  // grammar label; proxy version() is 2
      "started_at_block": 8239484 },
    { "chain_id": 10200, "address": "0xccA0Dc2A058884e62082312F09541cC7566406f0",
      "version": 6, "started_at_block": 15562365 },
    { "chain_id": 10200, "address": "0xccA0Dc2A058884e62082312F09541cC7566406f0",
      "version": 7, "started_at_block": 20553827 }
  ]
}
```

ABIs are the mainnet ones verbatim — the `topic0`s are identical, so the exact
JSON blobs in `config/xdai/bridges.json` can be reused per window.

Flooring Home at 15562365 (rather than 20553827) is what makes deposits 0 and 1
resolve, and is the reason to prefer it — at the price of the duplicate rows
described above. Flooring at 20553827 is clean but yields a nearly empty bridge.

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

If an xDai testnet section is added without the three config changes:

| Symptom | Cause |
|---|---|
| Bridge refuses to start, error names mainnet block 22273407 / 39569937 | epoch-floor `ensure!` in `abi.rs` |
| Bridge refuses to start, "has no legacy Ethereum asset" | `assert_epoch_boundaries_agree` → `legacy_ethereum_asset` bail below `LEGACY_DAI_EPOCH_START_BLOCK` |
| `no xDai grammar registered for side Foreign version 2` | `grammar_for`, if the config mirrors `EternalStorageProxy.version()` |
| `XDAI_SOURCE_ASSET_MISMATCH{bridge_id} = 1` + `error` log, indexing continues | mock DAI ≠ mainnet DAI in `check_source_asset_matches_latest` |
| Two messages for one deposit | Home-side nonce/hash re-affirmation (16803580, 20706963) |
| Messages keyed on `native_id` 30 / 31 | `MessageIdentity::destination` reading the v1-era `0x…1e` / `0x…1f` as nonces |

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
- the epoch floors or the `source_asset` table move from `version.rs` into
  `bridges.json` — the *Code-vs-Config* section then becomes historical;
- an xDai section is actually added to `config/full-testnet/bridges.json`;
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
