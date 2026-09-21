# xDai Bridge: Sepolia–Chiado Upgrade History and Mainnet Correspondence

## Scope

Independent reconstruction of the classic xDai bridge deployment between
Sepolia (`11155111`, Foreign side) and Chiado (`10200`, Home side), current as
of 2026-09-18. It covers:

- every proxy implementation installed on the two testnet sides;
- the concrete contract-surface change made by each upgrade;
- the temporal relationship between upgrades on the two sides;
- the closest corresponding Ethereum/Gnosis mainnet implementations;
- the separate, oracle-controlled transition from transaction-hash identity
  to nonce identity;
- consequences for the testnet xDai indexer configuration.

The existing `xdai-bridge-testnet-deployment-fit.md` was intentionally not
used as evidence. Its conclusions are neither inherited nor treated as an
authority here. `xdai-bridge-protocol-and-indexing-fit.md` was used only as a
map of the mainnet concepts to compare, and the relevant mainnet upgrade logs
and implementation ABIs were checked again on chain.

Out of scope: assigning an off-chain organizational motive to the one-sided
2026 upgrade, reconstructing the exact unverified Solidity source of Chiado
v3, or proposing a change to the current indexer.

## Short Answer

The May 2025 testnet upgrade was not temporally asymmetric: Sepolia Foreign
v2 and Chiado Home v2 were installed only 44 seconds apart. Both introduced
nonce-bearing source events and Hashi support. Sepolia additionally jumped
from a Compound/cDAI-era implementation directly to the sDAI-era public
surface, combining the effects of mainnet Foreign v7→v8 and v8→v9.

The real asymmetry began on 2026-04-02. Ownership of both proxies moved to the
same EOA, but only Chiado was upgraded, from Home v2 to v3. Chiado v3 adopts
the four-argument, token-bearing source event of mainnet Home v7, while
Sepolia remains on the mainnet-Foreign-v9 generation. It does not have the
Foreign v10 USDS reserve migration or its 124-byte message support.

Chiado v3 is also not a complete copy of mainnet Home v7. Its runtime selector
set lacks `setUSDSDepositContract(address)` and `usdsDepositContract()`, and
the two observed v3 source events name the Sepolia mock DAI. It is best
described as a DAI-only/test-token adaptation of the Home v7 event grammar.

Separately, the meaning of `AffirmationCompleted.bytes32` did not change at a
clean implementation boundary. The Chiado oracle alternated between source
transaction hashes and source nonces and executed three modern deposits under
both identities. Contract version and destination identity convention must
therefore be modeled as different facts.

## Why This Matters

The current pair is a hybrid of two mainnet generations:

- Sepolia Foreign v2 has the event and message grammar of mainnet Foreign v9;
- Chiado Home v3 has the source-event shape of mainnet Home v7, but without
  Home v7's complete USDS deposit surface;
- no completed Chiado-v3 → Sepolia-v2 transfer has been observed, so their
  message-blob compatibility remains an inference rather than a demonstrated
  end-to-end fact.

For indexing, three superficially similar boundaries must not be conflated:

1. the block where a proxy implementation changes;
2. the block where a source event changes `topic0`;
3. the interval in which the oracle changes the meaning of an unchanged
   destination `bytes32` field.

On Chiado, block `20553827` is both the v2→v3 implementation boundary and the
three-argument→four-argument source-event boundary. It is not a clean boundary
for destination identity semantics: the final hash-keyed completion occurred
350 blocks earlier, and a nonce-keyed re-execution occurred eleven days later.

## Source-of-Truth Files

Repository sources:

- `config/xdai/bridges-testnet.json` — configured proxy versions, ABIs, and
  scan floors;
- `config/full-testnet/ENVs.md` — generated deployment description and env
  form of the same configuration;
- `interchain-indexer-logic/src/indexer/xdai/version.rs` — per-deployment
  grammar, epoch floors, message-blob assumptions, and mock-DAI asset table;
- `interchain-indexer-logic/src/indexer/xdai/abi.rs` — ABI/version validation;
- `interchain-indexer-logic/src/indexer/xdai/events.rs` — event decoding and
  legacy source reconstruction;
- `interchain-indexer-logic/src/indexer/xdai/consolidation.rs` — identity and
  transfer consolidation;
- `.memory-bank/research/xdai-bridge-protocol-and-indexing-fit.md` — mainnet
  conceptual context only, not primary evidence for this note.

On-chain contracts:

| Role | Chain | Proxy |
| --- | --- | --- |
| Foreign xDai bridge | Sepolia | [`0x180Ff98e…D0A2`](https://eth-sepolia.blockscout.com/address/0x180Ff98e734415Ecd35faC3d32940e1B45FaD0A2) |
| Home xDai bridge | Chiado | [`0xccA0Dc2A…06f0`](https://gnosis-chiado.blockscout.com/address/0xccA0Dc2A058884e62082312F09541cC7566406f0) |
| Foreign xDai bridge | Ethereum | [`0x4aa42145…5016`](https://eth.blockscout.com/address/0x4aa42145Aa6Ebf72e164C9bBC74fbD3788045016) |
| Home xDai bridge | Gnosis | [`0x7301CFA0…0AA6`](https://gnosisscan.io/address/0x7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6) |

Evidence method:

1. Reconstruct proxy histories from
   `Upgraded(uint256,address)` (`topic0 = 0x4289d619…`).
2. Compare verified ABI signature sets and primary source units where source
   is available.
3. For unverified Chiado v3, compare extracted runtime function selectors and
   embedded event topics instead of guessing its source.
4. Check actual logs before and after every boundary.
5. Correlate modern Sepolia deposits to Chiado completions by both source
   nonce and source transaction hash.

## Key Types / Tables / Contracts

- `EternalStorageProxy.version()` — a per-proxy monotonically increasing
  counter. Its number is not globally comparable between deployments.
- `XDaiVersion::{SepoliaForeignV2, ChiadoHomeV3}` — current testnet grammar
  variants.
- `XDaiGrammar` — event topics, side, asset, blob-layout assumption, and epoch
  floor for a `(chain_id, side, version)` tuple.
- `UserRequestForAffirmation` — Sepolia→Chiado source event.
- `UserRequestForSignature` — Chiado→Sepolia source event.
- `AffirmationCompleted` — Chiado destination event. Its third `bytes32` is
  supplied by the oracle; its ABI name does not prove its semantics.
- `RelayedMessage` — Sepolia destination event.
- Sepolia mock DAI: `0x084Ab2ef1cb3A75EB0fDd81636e9A95D15629c37`.
- Current common proxy owner after the 2026 transfers:
  `0x328A29513E345e482dAeb571DeFc1eDa3F06d545`.

## Step-by-Step Flow

### 1. Testnet proxy upgrade history

#### Sepolia Foreign proxy

| v | Block | UTC | Implementation | Evidence |
| --- | ---: | --- | --- | --- |
| 1 | 5339499 | 2024-02-22 07:33:36 | [`0xc158633A…2b67`](https://eth-sepolia.blockscout.com/address/0xc158633Aa217119436e6Cb4a7EB070BeD2EF2b67) | [upgrade tx](https://eth-sepolia.blockscout.com/tx/0x2ee79dfd83b07519c0654ae781e0761f662ee4219aae67b2c8001bc838a68958) |
| 2 | 8239484 | 2025-05-02 10:14:36 | [`0xa54349C8…6515`](https://eth-sepolia.blockscout.com/address/0xa54349C84017566aFECB8AE818adc801138c6515) | [upgrade tx](https://eth-sepolia.blockscout.com/tx/0x5fa20e7a1a01ca5930352b2617de4fa70b9bcc892798fb4a1c63b191339d2fa3) |

There has been no Sepolia implementation upgrade after v2 through the date of
this note.

#### Chiado Home proxy

| v | Block | UTC | Implementation | Evidence |
| --- | ---: | --- | --- | --- |
| 1 | 7618689 | 2024-01-02 04:12:15 | [`0x01aad89d…448d`](https://gnosis-chiado.blockscout.com/address/0x01aad89ddda4e256885407da316dbb724c6e448d) | [upgrade tx](https://gnosis-chiado.blockscout.com/tx/0xf90f89a78f06a46ad67d3e9d56b2abc92911c93e653f8d07945f346db84e5cfb) |
| 2 | 15562365 | 2025-05-02 10:15:20 | [`0xbf7E7284…Ac1c`](https://gnosis-chiado.blockscout.com/address/0xbf7E72842A880a83B7EbC5C7D4536FAEE8b1Ac1c) | [upgrade tx](https://gnosis-chiado.blockscout.com/tx/0x6d4c147e01e1623a0353a5e9ccc15d820ae033cb3659f68853693c715fd03da9) |
| 3 | 20553827 | 2026-04-02 10:09:55 | [`0x3F218F9A…CAdB`](https://gnosis-chiado.blockscout.com/address/0x3F218F9A539da3A1262a1619404D1910a853CAdB) | [upgrade tx](https://gnosis-chiado.blockscout.com/tx/0x512103c5e2bd375b06aaa674d9a5bb1617864ccc03674fb7e65500f27df58d97) |

The initial deployments were 51 days apart, but that is not an implementation
upgrade asymmetry. The v2 pair was coordinated within 44 seconds. The current
asymmetry is the later Chiado-only v3.

On 2026-04-02, both proxy owners were transferred to the same EOA:

- Chiado at 08:06:45 UTC: [transaction](https://gnosis-chiado.blockscout.com/tx/0xee4baf7290a843e67a605cb0b199176236841343bd94058b13656734100abbe6);
- Sepolia at 08:38:48 UTC: [transaction](https://eth-sepolia.blockscout.com/tx/0xd50ebe90413dacbe8a4a478b9be8cae0df0c893fdf118212aea7894598b297c5).

That owner deployed the unverified Chiado v3 implementation at 09:53:55 UTC
([creation transaction](https://gnosis-chiado.blockscout.com/tx/0x1d6dfc009a80d88fe11688d6acd3814a514597e6a5d18f96703408ab28408d9f))
and installed it 16 minutes later. No matching Sepolia implementation was
installed.

### 2. Sepolia Foreign v1→v2

The source event changes from:

```solidity
UserRequestForAffirmation(address recipient, uint256 value)
```

to:

```solidity
UserRequestForAffirmation(address recipient, uint256 value, bytes32 nonce)
```

The verified ABI diff also shows:

- added Hashi surface: `hashiManager`, `setHashiManager`,
  `isApprovedByHashi`, `resendDataWithHashi`, `onMessage`, and the enabled /
  mandatory flags;
- added the monotonic `nonce()` getter;
- added `recoverLegacyTransfer(address)`;
- removed the Compound/cDAI surface: `cDaiToken`, `compToken`, `comptroller`,
  and `claimCompAndPay`;
- replaced `payInterest(address)` with `payInterest(address,uint256)`;
- added the sDAI-era surface: `sDaiToken`, `previewWithdraw`, and
  `refillBridge`;
- removed the test-specific `upgradeTo530(address)` helper.

The full Sepolia v2 ABI signature set equals mainnet Foreign v9. Relative to
Sepolia v1, however, this one upgrade combines two mainnet transitions:

- Foreign v7→v8: Compound/cDAI yield integration → sDAI-style integration;
- Foreign v8→v9: source nonce, Hashi, and legacy-transfer recovery.

The bridge's ERC-20 remains the same Sepolia mock DAI. This is a yield/control
surface change, not a testnet DAI→USDS reserve migration.

### 3. Chiado Home v1→v2

The source event changes from:

```solidity
UserRequestForSignature(address recipient, uint256 value)
```

to:

```solidity
UserRequestForSignature(address recipient, uint256 value, bytes32 nonce)
```

The verified ABI diff additionally adds the same Hashi surface and `nonce()`
getter as the Foreign side and removes `requiredMessageLength()`. Chiado v1's
ABI signature set equals mainnet Home v5; Chiado v2's equals mainnet Home v6.

This is the Home half of the coordinated May 2025 nonce/Hashi rollout. The
first two v2 source requests emitted nonces 0 and 1.

### 4. Chiado Home v2→v3

Chiado v3 is not verified, so its internally executed Solidity cannot be
asserted from source. Three runtime facts are nevertheless direct:

1. The old event topic for
   `UserRequestForSignature(address,uint256,bytes32)` disappears and the topic
   for `UserRequestForSignature(address,uint256,bytes32,address)` appears.
2. The `fixAssetsAboveLimits(bytes32,bool,uint256)` selector disappears and
   `fixAssetsAboveLimits(bytes32,bool,uint256,address)` appears.
3. Every other extracted function selector matches Chiado v2, while comparison
   with mainnet Home v7 finds only two additional mainnet selectors absent from
   Chiado v3:
   `setUSDSDepositContract(address)` and `usdsDepositContract()`.

The two observed v3 source events are:

| Block | Nonce | Token | Evidence |
| ---: | ---: | --- | --- |
| 20554257 | 2 | Sepolia mock DAI | [transaction](https://gnosis-chiado.blockscout.com/tx/0xbfde13ea62d4940746ed6cccf8cf0b8fbf6d6b9aabe7adb990513e76cea58bd9) |
| 20706993 | 3 | Sepolia mock DAI | [transaction](https://gnosis-chiado.blockscout.com/tx/0x319d1982cc17a49491c86556dc0a5e0bec4bf675ded1e6871a2fd0f88a5db4bc) |

Nonce continuity from v2's 0/1 to v3's 2/3 confirms storage continuity through
the proxy upgrade.

No `SignedForUserRequest` or `CollectedSignatures` follows either v3 source
request, and no corresponding `RelayedMessage` exists on Sepolia as of
2026-09-18. The repository's `BlobLayout::Len104` assignment for Chiado v3 is
therefore a compatibility inference: Sepolia v2 accepts only the legacy
104-byte layout, but no successful v3→v2 message has demonstrated what the
oracle actually encodes.

### 5. Mainnet correspondence

The version number itself is not the mapping key: every proxy starts its own
counter. Correspondence below is based on verified ABI/source surfaces and,
for Chiado v3, runtime selectors and event topics.

| Testnet implementation | Closest mainnet implementation | Relationship |
| --- | --- | --- |
| Sepolia Foreign v1 | Ethereum Foreign v7 (`0xEeE4f8dB…`) | Same event surface and Compound/cDAI generation; test-only migration helper differs |
| Sepolia Foreign v2 | Ethereum Foreign v9 (`0xb54042F5…`) | Identical ABI signature set; combines effects of mainnet v8 and v9 relative to testnet v1 |
| Chiado Home v1 | Gnosis Home v5 (`0x3b388724…`) | Identical ABI signature set |
| Chiado Home v2 | Gnosis Home v6 (`0xB740472C…`) | Identical ABI signature set |
| Chiado Home v3 | Gnosis Home v7 (`0xe6998b0C…`) | Same token-bearing source event and all selectors except the two USDS-deposit accessors |

Relevant mainnet sequence:

| Side/version | Block | UTC | Concrete change | Evidence |
| --- | ---: | --- | --- | --- |
| Ethereum Foreign v7 | 13367127 | 2021-10-06 | Compound/cDAI-era `XDaiForeignBridge` | [implementation](https://eth.blockscout.com/address/0xeee4f8db4410bebd74a76cb711d096c5e66d0473) |
| Ethereum Foreign v8 | 18175639 | 2023-09-20 | sDAI yield surface; no source-event change | [implementation](https://eth.blockscout.com/address/0x166124b75c798cedf1b43655e9b5284ebd5203db) |
| Ethereum Foreign v9 | 22273407 | 2025-04-15 09:12:11 | nonce-bearing source event and Hashi | [upgrade tx](https://eth.blockscout.com/tx/0xc4db8a77365d4870af65f44232ca728e5e0fd583cde3ba83ba81cac3d77ff89d) |
| Gnosis Home v6 | 39569937 | 2025-04-15 09:17:20 | nonce-bearing source event and Hashi | [upgrade tx](https://gnosisscan.io/tx/0x976d3973604588794b771377e356ae1e53a231eca133e003214722e27e639090) |
| Ethereum Foreign v10 | 23748179 | 2025-11-07 15:06:59 | DAI/sDAI → USDS/sUSDS reserve migration; event unchanged | [upgrade tx](https://eth.blockscout.com/tx/0x05db4562ed98cb55938bb541e030222c9630e6ff8e224abc3fc0aefd5aba1202) |
| Gnosis Home v7 | 43027713 | 2025-11-07 18:07:25 | token-bearing source event and 104/124-byte messages | [upgrade tx](https://gnosisscan.io/tx/0x451ac0bbb8c27dfd673be06de989e39cf2437dbfda894f10957f635b5f580baa) |

The mainnet nonce pair landed about five minutes apart. The testnet copied that
generation 17 days later and only 44 seconds apart. Mainnet v10/v7 landed about
three hours apart; testnet later copied only a reduced Home-v7 side and left
Foreign on the v9 generation.

### 6. Oracle identity is independent of implementation version

`AffirmationCompleted(address,uint256,bytes32)` retains the same topic through
all Chiado implementations. Its third value is whatever the validator/oracle
passes to `executeAffirmation`; the contract does not derive whether it is a
source transaction hash or a nonce.

All four modern Sepolia v2 deposits demonstrate the mixed convention:

| Source nonce | Source tx / block | Value | Chiado completion(s) |
| ---: | --- | ---: | --- |
| 0 | `0xd30a84a8…`, 8261069 | `0.1e18` | nonce 0 at 15612527; source tx hash at 16803580 |
| 1 | `0xb0ca9731…`, 8261098 | `0.02e18` | nonce 1 at 15612593; source tx hash at 16803580 |
| 2 | `0xa5559dce…`, 9311522 | `1e18` | source tx hash at 18042501 only |
| 3 | `0x24a6e680…`, 10573684 | `11e18` | source tx hash at 20553477; nonce 3 at 20706963 |

For nonces 0, 1, and 3, the two completions have the same recipient and value
but different `bytes32` keys. The nonce-3 pair is particularly diagnostic:

- source request: [Sepolia transaction](https://eth-sepolia.blockscout.com/tx/0x24a6e680de4e33a2528e7142ce102c0281a1a97fe9f1c1c3ed14725a3d25a8c4);
- hash-keyed completion, 33 minutes before the Chiado v3 upgrade:
  [Chiado transaction](https://gnosis-chiado.blockscout.com/tx/0xb3af155c992ea6335191f0865fe38a6b256142c19770f8a575296e7143ed14b4);
- nonce-keyed completion, eleven days after the upgrade:
  [Chiado transaction](https://gnosis-chiado.blockscout.com/tx/0xda69bb6fe2fdbf89de9feae08266ec8f2a967e010ead4799fc3f4d8fce10d9f2).

This proves that no implementation block can by itself classify every Chiado
destination `bytes32`. The v3 boundary happens to exclude all preceding
hash-keyed completions; it did not create a reliable destination identity
epoch.

### 7. Current indexer boundary

`config/xdai/bridges-testnet.json` configures:

| Side | Version | `started_at_block` | Effect |
| --- | ---: | ---: | --- |
| Sepolia Foreign | 2 | 8239484 | Starts exactly at the two-argument→three-argument source-event change |
| Chiado Home | 3 | 20553827 | Starts exactly at the three-argument→four-argument source-event change and after every observed hash-keyed completion |

This boundary avoids the mixed-identity destination history, but it means the
Chiado completions for Sepolia nonces 0, 1, and 2 are outside the scan window.
Those messages can remain `Initiated`. Nonce 3 has a nonce-keyed completion
above the Chiado floor; its earlier hash-keyed completion remains excluded.

Two repository comments currently state that `20553827` is *not* the block at
which the Home event shape changed:

- `interchain-indexer-logic/src/indexer/xdai/version.rs` near
  `CHIADO_EPOCH_FLOOR_BLOCK`;
- `config/full-testnet/ENVs.md` in the bridge `1003` description.

That literal statement conflicts with the upgrade log, v2/v3 runtime topics,
and the first post-upgrade source event. The accurate distinction is:

- `20553827` **is** the last Home source-event-shape change;
- it is **not** a clean boundary at which the meaning of the unchanged
  destination `AffirmationCompleted.bytes32` changed.

## Invariants

- A proxy `version()` is meaningful only with its deployment chain and side.
  Testnet versions 2/3 are not mainnet versions 2/3.
- Sepolia v2 source requests always carry a nonce; Chiado v3 source requests
  always carry both nonce and token.
- The Sepolia bridged asset remains the single mock DAI across both testnet
  Foreign implementations; no testnet USDS reserve flip was observed.
- Proxy upgrades preserve storage. Chiado's source nonce continued 0, 1, 2, 3
  across v2→v3.
- Source-event topic changes are implementation facts. Destination `bytes32`
  semantics are oracle-input facts and may vary without an upgrade.
- A token-bearing Chiado event does not by itself prove a 124-byte message was
  produced or accepted on Sepolia.

## Failure Modes / Observability

- Using the wrong source-event ABI silently misses an entire version window
  because every relevant upgrade changes `topic0`.
- Treating all post-v2 Chiado completions as nonce-keyed either loses the
  hash-keyed executions or creates additional message identities for deposits
  that were also completed under a nonce.
- Lowering the current Chiado floor admits mixed identity history and can
  surface duplicate logical deposits under different native IDs.
- Treating Chiado v3 as full mainnet Home v7 invents a testnet USDS deposit
  route that its runtime does not expose.
- Treating the four-argument v3 event as proof of 124-byte support can make the
  indexer claim a compatibility path never demonstrated on chain.

Inspect:

- proxy `Upgraded` and `ProxyOwnershipTransferred` logs;
- source event `topic0` and arity;
- `AffirmationCompleted` recipient/value/third-field triples;
- `SignedForUserRequest`, `CollectedSignatures`, and Sepolia `RelayedMessage`
  for the first future completed Chiado-v3 transfer;
- startup ABI/floor validation in `xdai/abi.rs`.

## Edge Cases / Gotchas

- The initial Chiado and Sepolia installations were asynchronous, but neither
  was an upgrade from a functioning paired testnet implementation.
- Sepolia v1 is only *v7-era*, not byte-for-byte/mainnet-ABI identical, because
  its test-specific migration helper differs.
- Sepolia v1→v2 bundles two generations of mainnet Foreign changes; describing
  it only as the nonce upgrade omits the Compound/cDAI→sDAI surface migration.
- Chiado v3's source is unverified. Selector equality cannot prove internal
  control flow, storage-key usage, or message serialization.
- Explorer ABI parameter names such as `transactionHash` are not semantic
  evidence for the `bytes32` value actually supplied by the oracle.
- A post-v3 `AffirmationCompleted` on Chiado is an Ethereum→Gnosis destination
  event; it does not demonstrate that a Chiado-v3 source request can complete
  in the opposite direction on Sepolia.

## Change Triggers

Update this note when any of the following occurs:

- Sepolia upgrades beyond Foreign v2;
- Chiado upgrades beyond Home v3 or the v3 source becomes verified;
- the first Chiado-v3 source request reaches `CollectedSignatures` and a
  Sepolia `RelayedMessage`;
- a testnet USDS or second-asset route appears;
- the oracle emits another destination completion whose `bytes32` convention
  changes;
- the configured scan floors or testnet grammar entries change;
- the inaccurate `20553827` comments in `version.rs` / `ENVs.md` are corrected.

## Open Questions

1. Which exact source commit/build produced unverified Chiado v3?
2. Does Chiado v3 serialize a 104-byte mock-DAI message, a 124-byte
   token-bearing message, or choose dynamically?
3. Why were both proxy owners transferred together if only Chiado was
   upgraded?
4. Why did the oracle re-execute modern Sepolia deposits using alternate
   identity keys, including eleven days after the v3 upgrade?
5. Did each paired `AffirmationCompleted` cause a second native-value release,
   or was any compensating state/action applied outside the bridge logs?
