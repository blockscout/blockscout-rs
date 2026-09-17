# xDai Bridge: Protocol Model, Contrast With Omnibridge, and Indexing Fit

## Scope

The xDai bridge (DAI/USDS on Ethereum ↔ native xDAI on Gnosis Chain) as an
*indexing target*: its event grammar, how a message is identified, what a
transfer can and cannot carry, the full implementation-upgrade history of both
proxies, and which parts of the existing indexer architecture it fits.

Out of scope: a concrete implementation plan, the sDAI/DSR yield mechanics, the
Gnosis consensus-layer minting path beyond what is observable from logs, and
the AMB/Omnibridge message lifecycle itself (see
`amb-omnibridge-token-reconstruction.md` and `message-lifecycle.md`).

Status: **the xDai indexer now exists** —
`interchain-indexer-logic/src/indexer/xdai/` and `config/xdai/`, running as
`bridge_id = 3`. The config floors both sides at the current epoch
(Foreign v9 / Home v6), so only the nonce-identified era is indexed today.
Sections below marked as proposals were written before implementation; where
they disagree with the code, the code wins.

The proxy upgrade history of both sides — every implementation, its block, and
what it changed in the log surface — is in *Implementation Upgrade History*
below; it is the source of the `version` / `started_at_block` pairs in
`config/xdai/bridges.json`. Validator-set facts are in *Validator set* in that
same section.

## Short Answer

The xDai bridge is **not** an application on top of AMB. It is a standalone
`ERC20-to-Native` TokenBridge with its own validator set, sharing only ancestry
(and therefore event *names*) with AMB. There is no message header, no
`messageId`, no arbitrary payload: a message is exactly
`(recipient, value, nonce)` plus, on one direction, a destination-token
selector. Identity is a **per-contract monotonic counter**, so it is unique only
within a direction — and before April 2025 the same field held the source
transaction hash instead, under an unchanged `topic0`.

Consequences: one message always carries exactly one transfer; the buffer key
must be `f(direction, nonce)`; and historical indexing needs version-aware
*identity derivation*, not just version-aware ABI.

The official bridge explorer keys a transfer on
`initiator_chain_id (4 bytes) ‖ nonce (28 bytes)`. Storing exactly that blob in
`native_id` makes the identity globally unique *and* makes the existing
`{{message_id}}` `ui_url` template produce a correct link with no code change.

Neither direction's *asset* is fully derivable from logs: the Ethereum→Gnosis
source token is not in any event, and it silently changed from DAI to USDS at
Ethereum block 23748179.

## Why This Matters

Three traps make a naive reuse of the AMB indexer actively wrong rather than
merely incomplete:

1. **Name collision with different signatures.** Seven event names are shared
   with AMB. `dispatch_transaction` in
   `interchain-indexer-logic/src/indexer/amb/events.rs` dispatches on
   `event.name`, and `amb_side_for_abi` in
   `interchain-indexer-logic/src/indexer/amb/abi.rs` infers Home/Foreign from
   the *set of names*. An xDai ABI passes both checks and then fails at decode
   (`parse_amb_header`, `expect_b256`).
2. **A silent identity break.** `AffirmationCompleted`, `RelayedMessage`,
   `SignedForAffirmation`, `SignedForUserRequest` have carried the same
   `topic0` since 2018-10-08, but their `bytes32` changed meaning from *source
   transaction hash* to *nonce* on 2025-04-15. Nothing on chain signals this;
   only the block number does.
3. **A silent asset break.** `UserRequestForAffirmation` carries no token field,
   so the Ethereum→Gnosis source asset comes from contract state
   (`erc20token()`), and that flipped DAI → USDS at block 23748179 with no
   change to the event, the signature, or the `topic0`. Getting this wrong
   mislabels *every* deposit in that direction and corrupts stats volume.

## Source-of-Truth Files

### On chain (all verified sources; read via Blockscout)

| Role | Chain | Address |
|---|---|---|
| Foreign proxy (`EternalStorageProxy`) | Ethereum (1) | `0x4aa42145Aa6Ebf72e164C9bBC74fbD3788045016` |
| Foreign impl v10 (`XDaiForeignBridge`) | Ethereum (1) | `0x257bDD093Cab1Bd39eBF837dCB60f33d031d7d49` |
| Home proxy (`EternalStorageProxy`) | Gnosis (100) | `0x7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6` |
| Home impl v7 (`HomeBridgeErcToNative`) | Gnosis (100) | `0xe6998b0C03D3cb9ee8C04f266e573c7Fa8782846` |
| Block reward (mints xDAI) | Gnosis (100) | `0x481c034c6d9441db23Ea48De68BCAe812C5d39bA` |
| Validator management | Ethereum (1) | `0xe1579dEbdD2DF16Ebdb9db8694391fa74EeA201E` |
| Validator management | Gnosis (100) | `0xB289f0e6fBDFf8EEE340498a56e1787B303F1B6D` |
| USDS deposit contract | Gnosis (100) | `0x5C183C8A49aBA6e31049997a56D75600E27FF8c9` |
| `BridgeRouter` proxy (shared entry point) | Ethereum (1) | `0x9a873656c19Efecbfb4f9FAb5B7acdeAb466a0B0` (impl `0x74899961…`) |
| `XDaiBridgePeripheral` (DAI→USDS adapter) | Ethereum (1) | `0x3b6669727927b934753B018EB421a84Ed4eb0a43` |
| DAI | Ethereum (1) | `0x6B175474E89094C44Da98b954EedeAC495271d0F` |
| USDS | Ethereum (1) | `0xdC035D45d973E3EC169d2276DDab16f1e407384F` |
| `DaiUsds` 1:1 converter | Ethereum (1) | `0x3225737a9Bbb6473CB4a45b7244ACa2BeFdB276A` |
| sDAI vault (pre-v10 yield) | Ethereum (1) | `0x83F20F44975D03b1b09e64809B757c47f942BEeA` |
| sUSDS vault (post-v10 yield) | Ethereum (1) | `0xa3931d71877C0E7a3148CB7Eb4463524FEc27fbD` |
| Hashi manager | Ethereum (1) | `0x9acCFAD714A1e670CD1f6dc666FE892d1d5547BD` |
| Hashi manager | Gnosis (100) | `0x60Aa15198a3AdfC86FF15B941549A6447B2dDB49` |

Solidity units that define the behaviour (tokenbridge-contracts layout, as
verified behind the two proxies): `BasicHomeBridge.sol`,
`BasicForeignBridge.sol`, `BasicTokenBridge.sol`, `libraries/Message.sol`,
`BaseOverdrawManagement.sol` / `HomeOverdrawManagement.sol`,
`RewardableBridge.sol`.

Docs: <https://docs.gnosischain.com/bridges/About%20Token%20Bridges/xdai-bridge>

### In this repo (the code an xDai indexer would sit beside or reuse)

- `interchain-indexer-logic/src/indexer/amb/` — closest existing analogue;
  `abi.rs` (registry, `version_at`), `events.rs` (dispatch), `version.rs`
  (grammar), `consolidation.rs` (status/finality), `types.rs`.
- `interchain-indexer-logic/src/indexer/evm/` — `receipt_fetch.rs`,
  `transaction_grouping.rs`, `log_stream_builder.rs`.
- `interchain-indexer-server/src/config.rs` — `IndexerType`, `BridgeConfig`,
  `BridgeContractConfig` (`version`, `started_at_block`, `kind`, `abi`).
- `interchain-indexer-server/src/indexers.rs` — `spawn_configured_indexers`
  dispatch on `BridgeType` × `IndexerType`.
- `interchain-indexer-entity/src/codegen/sea_orm_active_enums.rs` — `BridgeType`
  (`lockmint | avalanche_native | amb`), `MessageStatus`, `TransferType`.

## Key Types / Tables / Contracts

### Event grammar (current implementations)

| Event | Side | `topic0` | Params |
|---|---|---|---|
| `UserRequestForAffirmation` | Foreign | `0xf6968e68…` | `(address recipient, uint256 value, bytes32 nonce)` — none indexed |
| `RelayedMessage` | Foreign | `0x4ab7d581…` | `(address recipient, uint256 value, bytes32 transactionHash)` — none indexed; the value is the **home** nonce despite the name |
| `UserRequestForSignature` | Home | `0xe1e0bc4a…` | `(address recipient, uint256 value, bytes32 nonce, address token)` — none indexed |
| `SignedForAffirmation` | Home | `0x5df9cc3e…` | `(address indexed signer, bytes32 nonce)` |
| `SignedForUserRequest` | Home | `0xbf06885f…` | `(address indexed signer, bytes32 messageHash)` |
| `CollectedSignatures` | Home | `0x41555740…` | `(address authorityResponsibleForRelay, bytes32 messageHash, uint256 NumberOfCollectedSignatures)` |
| `AffirmationCompleted` | Home | `0x6fc115a8…` | `(address recipient, uint256 value, bytes32 nonce)` — none indexed |

Auxiliary, keyed by nonce or `hashMsg` and therefore attachable to a message:
`FeeDistributedFromAffirmation(uint256,bytes32 indexed)`,
`FeeDistributedFromSignatures(uint256,bytes32 indexed)`,
`AmountLimitExceeded(address,uint256,bytes32 indexed,bytes32)` `0x26c0a209…`,
`AssetAboveLimitsFixed(bytes32 indexed,uint256,uint256)` `0x5bcec656…`,
`MediatorAmountLimitExceeded(address,uint256,bytes32 indexed)` `0x3344bbb9…`
(present in the deployed Home v7 ABI, inherited from `BaseOverdrawManagement`;
listed in *Breaking changes* below but previously missing here — whether this
contract ever actually emits it is unverified).

Not attachable: `AddedReceiver(uint256 amount, address indexed receiver,
address indexed bridge)` on the block reward contract carries no message key.

### Name collision with AMB — the decisive table

| Event | AMB `topic0` | xDai `topic0` |
|---|---|---|
| `UserRequestForAffirmation` | `0x482515ce…` *(bytes32,bytes)* | `0xf6968e68…` |
| `UserRequestForSignature` | `0x520d2afd…` *(bytes32,bytes)* | `0xe1e0bc4a…` |
| `RelayedMessage` | `0x27333edb…` *(address,address,bytes32,bool)* | `0x4ab7d581…` |
| `AffirmationCompleted` | `0xe194ef61…` *(address,address,bytes32,bool)* | `0x6fc115a8…` |
| `SignedForAffirmation` | `0x5df9cc3e…` | **identical** |
| `SignedForUserRequest` | `0xbf06885f…` | **identical** |
| `CollectedSignatures` | `0x41555740…` | **identical** |

Matching is safe — `AbiRegistry::resolve_log` keys on
`(chain_id, address, topic0, block)`. *Dispatch* is not: it keys on
`event.name`.

### Message identity

Both sides inherit one counter from `BasicTokenBridge`:

```solidity
function nonce() public view returns (uint256) { return uintStorage[TOKEN_BRIDGE_NONCE]; }
```

and emit it the same way (`BasicForeignBridge`, `BasicHomeBridge`):

```solidity
uint256 currentNonce = nonce();
setNonce(currentNonce + 1);
emit UserRequestForAffirmation(_receiver, _amount, bytes32(currentNonce));
```

Two contracts on two chains ⇒ **two independent counters**, so nonce ranges
overlap across directions (observed simultaneously: `≈0x1ae0` for
Ethereum→Gnosis, `≈0x140c` for Gnosis→Ethereum).

The protocol's own globally-unique hashes:

| Direction | Hash | Preimage | Defined in |
|---|---|---|---|
| Ethereum→Gnosis | `hashMsg` | `recipient ‖ value ‖ nonce` (84 bytes) | `BasicHomeBridge.executeAffirmation` |
| Gnosis→Ethereum | `messageHash` | `recipient ‖ value ‖ nonce ‖ foreignBridgeAddr [‖ token]` (exactly 104 or 124 bytes) | `BasicHomeBridge.submitSignature` |

Different preimage lengths ⇒ the two families never collide. These are also
what the contracts use for their own dedup (`affirmationsSigned`,
`messagesSigned`, plus `relayedMessages(nonce)` on the foreign side).

### `native_id` encoding (matches the official explorer)

`bridge.gnosischain.com/bridge-explorer/transaction/<id>` keys on a 32-byte blob:

```
bytes  0..4  : initiator chain id, big-endian   (1 = Ethereum, 100 = Gnosis)
bytes  4..32 : nonce, big-endian, zero-padded
```

Examples (the first is a confirmed live explorer URL; both correspond to
messages traced in *Worked log traces* below):

```
Ethereum(1) nonce 0x1adf -> 0x0000000100000000000000000000000000000000000000000000000000001adf
Gnosis(100) nonce 0x140a -> 0x000000640000000000000000000000000000000000000000000000000000140a
```

This is the recommended value for `crosschain_messages.native_id`: the chain-id
prefix *is* the direction discriminator the bare nonce lacks, so the blob is
globally unique and the buffer key can be derived straight from it. It also
needs no serving-layer work — `get_message_id_from_message`
(`interchain-indexer-server/src/services/interchain_service.rs`) already returns
`to_hex_prefixed(native_id)` as the API's `message_id`, and the AMB-style
`ui_url` template `…/bridge-explorer/transaction/{{message_id}}` then resolves
correctly.

#### Legacy (tx-hash) era: store the transaction hash verbatim

28 bytes cannot hold a 32-byte transaction hash, so the encoding above has no
form for the pre-2025-04-15 era. **The decision is to store the raw source
transaction hash as `native_id` for those messages.** Nothing in the stack
objects:

- `native_id` is `Option<Vec<u8>>` → `bytea`, no length constraint;
- `get_crosschain_message` (`interchain-indexer-logic/src/database.rs`)
  dispatches purely on length — `> 8 bytes` → look up by `native_id`, `<= 8` →
  interpret as the `id` PK. A 32-byte hash lands in the `native_id` branch.
  Variable length is already exercised: a test there uses a **16-byte**
  `native_id`;
- `get_message_id_from_message` just hex-encodes the blob;
- `IdentityStrategy` in `indexer/xdai/version.rs` exists for exactly this — its
  doc comment says the enum is there so a transaction-hash-keyed epoch becomes a
  new arm rather than a redesign.

The only blocker is the `ensure!` in `native_id_blob`
(`indexer/xdai/types.rs`), which rejects a value that does not fit in 28 bytes.
That guard was written on the premise that every message must yield a working
explorer link — and the official explorer cannot render legacy transfers at all,
so the premise does not hold. Today the guard makes the handler return `Err`, so
those messages are **not indexed at all** (this is the source of the
`failed to process xDai event` log lines). The `native_id = NULL` alternative is
also worse: `get_message_id_from_message` then falls back to
`format!("0x{:x}", message.id)`, leaking the internal buffer key into the public
API for an equally dead link.

The implemented change classifies the unchanged destination `bytes32` at event
level: `value <= u64::MAX` is a nonce, and any larger value is retained as the
raw source transaction hash. Source events remain nonce-only and reject larger
values. This deliberately uses an eight-byte numeric threshold (a uniform hash
is misclassified with probability 2⁻¹⁹²), so hashes with four or eight leading
zero bytes are still preserved when their remaining high bytes are non-zero.
Configured source grammar windows remain nonce-based; no unsupported old scan
window was added.

Caveat, recorded honestly: in the legacy era the protocol's own identity is
`hashMsg = keccak(recipient ‖ value ‖ bytes32)`, not the bare transaction hash —
one transaction could in principle carry several `relayTokens` calls sharing a
hash, which is precisely the ambiguity the nonce upgrade removed. But `hashMsg`
is not usable as a buffer key in either era, because
`SignedForAffirmation(signer, bytes32)` carries no recipient or value; the
`bytes32` is the only key the event grammar offers. Keying on it is forced, not
chosen. Empirically the risk is nil: 1500 legacy `UserRequestForAffirmation`
events sampled across Ethereum blocks 21882741–22273111 gave 1500 distinct
transactions and **zero** carrying more than one deposit. Worth a counter (like
AMB's `AMB_IDENTITY_CONFLICTS_TOTAL`) if two different `(recipient, value)`
pairs ever arrive under one legacy `bytes32`, rather than silently merging.

Two consequences to document for consumers: the legacy `ui_url` will 404 (no
worse than the message being absent, but suppressing the link when the id is not
in encoded form is a reasonable product call), and a raw hash does not encode
its chain the way the 4-byte prefix does — `src_chain_id` on the row is what
tells you which explorer to open.

### Message body layout (`libraries/Message.sol`)

```
offset  32: 20 bytes  recipient
offset  52: 32 bytes  value
offset  84: 32 bytes  nonce            // comment still says "transaction hash"
offset 116: 20 bytes  contractAddress  // foreign bridge, anti-double-spend
offset 136: 20 bytes  tokenAddress     // 124-byte variant only
```

`isMessageValid` **requires** length ∈ {104, 124}; `parseMessage` hardcodes DAI
`0x6B175474E89094C44Da98b954EedeAC495271d0F` for the 104-byte legacy form.

### Asset model (DAI / USDS / native xDAI)

The Gnosis side is always the native coin (`TransferType::Native`, no address).
The Ethereum side is where it gets version-dependent.

**Ethereum→Gnosis source token is not in any event.** `relayTokens` pulls
`erc20token()`, and `UserRequestForAffirmation` has no token field, so the asset
is contract state. Bisected on an archive node against the `erc20token()`
getter:

| Ethereum blocks | source asset |
|---|---|
| 9161003 – 23748178 | DAI `0x6B175474…` |
| 23748179 – … | USDS `0xdC035D45…` |

23748179 is exactly the Foreign v10 upgrade block. An indexer must resolve this
from a version-keyed table, never from logs.

The bridge itself accepts **exactly one** token at a time — DAI is still
bridgeable only through an external adapter placed *around* the bridge, not by
the bridge. `BridgeRouter.relayTokens(_token, _receiver, _amount)` dispatches:

| `_token` | route |
|---|---|
| DAI | `XDaiBridgePeripheral` `0x3b6669727927b934753B018EB421a84Ed4eb0a43` → `daiToUsds` → xDai bridge |
| USDS | xDai bridge directly (`tokenRoutes[USDS]` = the bridge) |
| `address(0)` | `WETHOmnibridgeRouter` `0xa6439Ca0…` (`wrapAndRelayTokens`) |
| anything else | **Omnibridge** `0x88ad09518695c6c3712AC10a214bE5109a655671` |

`XDaiBridgePeripheral` is non-upgradeable, `onlyRouter`, and emits **no events
of its own**: it converts DAI→USDS 1:1 via `DaiUsds` and then calls
`relayTokens`. So a DAI deposit is indistinguishable from a USDS deposit at the
bridge's event level — the only trace is the ERC-20 `Transfer`/`Approval` logs
in the same transaction.

**Decision:** `token_src_address` for Ethereum→Gnosis records *what the bridge
saw* (USDS from 23748179, DAI before). That is the protocol truth, it matches
the reserve, and it keeps stats aggregates internally consistent. Surfacing
"the user actually paid DAI" is a UI/enrichment concern requiring transaction
token-transfer inspection, deliberately out of the indexer's scope.

**Gnosis→Ethereum destination token is explicit** in
`UserRequestForSignature.token` — but only since Home v7; before that it is
always DAI. The home side picks it by *caller identity*, not user input:
`tokenAddress = DAI; if (msg.sender == usdsDepositContract) tokenAddress = USDS;`.

The foreign bridge holds **USDS** as its reserve since v10 and converts on
payout, 1:1, inside `onExecuteMessage`:

```solidity
ERC20 token = ERC20(USDS);
if (_tokenAddress == DAI) {
    token.approve(DAI_USDS, _amount);
    IDaiUsds(DAI_USDS).usdsToDai(address(this), _amount);
    return ERC20(DAI).transfer(_recipient, _amount);
} else if (_tokenAddress == USDS) { /* direct USDS transfer */ }
```

The migration itself is the one-shot `swapSDAIToUSDS()` (withdraw sDAI → DAI,
disable DAI interest, `IDaiUsds.daiToUsds`, set
`boolStorage[keccak256("upgrade_DAI_to_USDS")]`); yield moved from the sDAI
vault to sUSDS.

DAI and USDS are both 18-decimal and convert 1:1, but they are distinct
addresses and must stay distinct in `token_*_address` and in stats asset
linking — collapsing them would hide the migration.

### Plain-transfer deposits (sending straight to the bridge address)

The two directions look superficially alike and are fundamentally different.

**Gnosis → Ethereum: supported and fully observable.** The home implementation
has a payable fallback:

```solidity
function() public payable {
    require(msg.data.length == 0);
    nativeTransfer(msg.sender);
}
function relayTokens(address _receiver) external payable { nativeTransfer(_receiver); }
```

Both paths reach `nativeTransfer`, which emits `UserRequestForSignature`. The
only difference is the recipient: the fallback uses `msg.sender`. So "just send
xDAI to the bridge" is a first-class path, it emits the event, and nothing
special is needed to index it. Group 2 above is exactly this.

**Ethereum → Gnosis: a gap, not a path.** The foreign implementation has **no
fallback at all** (no `function ()` anywhere in its sources) and no
`onTokenTransfer` — DAI and USDS are plain ERC-20s, whose `transfer` does not
notify the recipient. The bridge's code therefore never runs: no event, no
nonce, no storage write. Only the token's own `Transfer` log exists, and the
deposit can be honoured only by off-chain validator monitoring, keyed on the
transaction hash because nothing else exists.

Related asymmetry: `relayTokens` enforces `require(withinLimit(_amount))` at the
source, while a plain transfer bypasses it entirely — the limit is then hit on
the destination side in `withinExecutionLimit`, with the parking consequences
described under *Failure Modes*.

**This is the original mechanism, not a later addition.** Foreign v1 and v2
(2018-10-08 → 2019-12-24) declare no source event at all, so watching the
token's `Transfer` to the bridge address was the *only* way to deposit from
Ethereum. `relayTokens` / `UserRequestForAffirmation` arrived at v3
(block 9161003, 2019-12-25) and sat alongside it for years — on-chain for new
deposits, off-chain monitoring for the legacy path, with the docs warning
against `transfer`. It is now dead: the v10 `recoverUSDS` doc comment exists
precisely to retrieve tokens "mistakenly sent to this contract after the Hashi
integration, as the Transfer event will no longer be supported". Bracketed
empirically — still honoured on 2025-04-25 (the worked example below), and
absent across Gnosis blocks 45473867–48293152, where 2000 consecutive
`AffirmationCompleted` events contain zero legacy-format identifiers.

Worked example: Ethereum `0x37b3752a…` (block 22027902, 2025-03-12) is a plain
`transfer` of 1460.00000000000001019 DAI to the bridge, whose only log is the
DAI `Transfer` — no bridge event. Gnosis `0x66f605d4…` (block 39738154,
2025-04-25) then carries `SignedForAffirmation` + `AddedReceiver` +
`AffirmationCompleted` for `(0x2F137840…, same value, 0x37b3752a…)`, and
`numAffirmationsSigned` = `2**255 | 4` — 44 days later, four validators
honoured it and the xDAI was minted.

**How to index these — not by subscribing to `Transfer`.** That filter is cheap
(`address = DAI|USDS`, `topic0 = Transfer`, `topic2 = bridge`, all indexed) but
wrong, because it cannot tell a deposit from everything else that moves tokens
into the bridge:

- `relayTokens` itself does `erc20token().transferFrom(msg.sender, address(this), _amount)`,
  so **every ordinary deposit emits an identical `Transfer(user → bridge)`**;
- `XDaiBridgePeripheral` transfers USDS in before calling `relayTokens`;
- `refillBridge()` / `_withdraw` pull from the sUSDS/sDAI vault back into the
  bridge; the one-shot `swapSDAIToUSDS()` moved the entire reserve;
- ordinary mistaken transfers that validators never honour.

The first two are separable by "no `UserRequestForAffirmation` from the bridge
in the same transaction" — free, since receipts are fetched anyway
(`indexer/evm/receipt_fetch.rs`). The internal flows need an address allowlist,
which is fragile. The last class is *undecidable at source time*: a plain
transfer becomes a bridge message only retroactively, when it is affirmed.

Derive it from the destination instead. `AffirmationCompleted(recipient, value,
bytes32)` is self-sufficient, because in this era the `bytes32` *is* the source
transaction hash. Full reconstruction from that one log:

| column | source |
|---|---|
| `status` | `Completed` — the event only fires at threshold |
| `src_chain_id` / `dst_chain_id` | `1` / `100`, fixed: `AffirmationCompleted` exists only on the Home side, so the direction is Eth→Gno by construction |
| `src_tx_hash` | the `bytes32` itself |
| `dst_tx_hash` | the log's own transaction |
| `native_id` | the same `bytes32` (see *Legacy (tx-hash) era* above) |
| `recipient_address` | event `recipient` |
| `init_timestamp` | source block timestamp from the targeted receipt lookup |
| `last_update_timestamp` | destination block timestamp |
| `src_amount` / `dst_amount` | event `value` for both; this direction has no fee, and no source event exists to differ from it |
| `token_src_address` | epoch table — DAI below Ethereum block 23748179, USDS from it |
| `token_dst_address` | `NATIVE_SENTINEL` (the Gnosis leg is native xDAI) |
| `sender_address` | source receipt `from` (the receipt fetch helper exposes it without a separate transaction lookup) |

The source receipt and its block are fetched together. The lookup also proves
whether a legacy bridge source event exists and supplies a direction-safe asset:
Gno→Eth legacy messages always pay DAI, while only Eth→Gno compares its Ethereum
source block to the DAI/USDS cutover.

**What actually blocks this today** is the early return in
`indexer/xdai/consolidation.rs`:

```rust
(None, None) => return Ok(None),
```

Its comment gives three reasons — "no recipient, no timestamp to anchor
`init_timestamp` on, and no direction to derive `native_id` from" — and the
table above answers all three for this specific event (recipient is in the
event; the timestamp is the destination block's; the direction is implied by
which side emits `AffirmationCompleted`). Note the consequence of the gate as it
stands: a group-3 message currently produces **no row at all** — the destination
evidence sits in the buffer and never consolidates.

The cost of lifting the gate is that such a message appears already `Completed`,
with no `Initiated` phase — arguably correct, since an unhonoured plain transfer
is not a bridge message. Structurally this is the destination-only shape the
codebase already has (`build_destination_only` in AMB, `CompletionEvent` in
xDai). Lifting it would also need care so it does **not** apply in the nonce
era, where a destination-only observation means "source not yet indexed" and
should keep waiting rather than fabricating a completed row.

The minimal implementation now reconstructs this completed path without adding
a `Transfer` subscription: a missing bridge source event is accepted only for
Eth→Gno, and the completion amount is used for both sides in that single
plain-transfer fallback. A `Transfer` subscription would only be warranted
for a distinct feature — surfacing deposits the validators never honoured — and
should be scoped as such, not folded into message indexing.

## Step-by-Step Flow

Direction naming below follows the contracts' own vocabulary: **Home = Gnosis**,
**Foreign = Ethereum** (counter-intuitive, and the source of the `ForeignToHome`
/ `HomeToForeign` names in `amb/types.rs`).

### Ethereum → Gnosis (affirmation)

1. User calls `relayTokens(receiver, amount)` on the foreign bridge;
   `erc20token()` (USDS since block 23748179, DAI before) is pulled in and
   `UserRequestForAffirmation(recipient, value, nonce)` is emitted, incrementing
   the foreign counter. A plain ERC-20 `transfer` to the bridge address does
   **not** bridge anything — only `relayTokens` does.
2. Each validator calls `executeAffirmation(recipient, value, nonce)` on the
   home bridge **in its own transaction**, emitting
   `SignedForAffirmation(signer, nonce)`.
3. On reaching `requiredSignatures()` (4-of-7 per docs), the same transaction
   also emits `AffirmationCompleted(recipient, value, nonce)` and calls
   `blockReward.addExtraReceiver(...)`.
4. The actual xDAI credit happens **in a later block**, via the consensus
   engine and the block reward contract (`AddedReceiver`).

### Gnosis → Ethereum (signatures)

1. User sends native xDAI to the home bridge (payable fallback) or calls
   `relayTokens(receiver)`. The coins are burnt to `address(0)` and
   `UserRequestForSignature(recipient, value, nonce, token)` is emitted,
   incrementing the home counter. `token` is `DAI`, or `USDS` when the caller is
   the USDS deposit contract — it is *not* user-chosen.
2. Validators call `submitSignature(signature, message)` on the **home** chain →
   `SignedForUserRequest(signer, messageHash)`.
3. At threshold, `CollectedSignatures(relayer, messageHash, requiredSignatures)`
   — note the third field is `requiredSignatures()`, not the actual count.
4. Anyone calls `executeSignatures(message, signatures)` on Ethereum →
   `RelayedMessage(recipient, value, nonce)` and the ERC-20 payout.

### Worked log traces (real data, verified)

**Ethereum → Gnosis, nonce `0x…1ae0`**

| Chain / block | tx | log | event |
|---|---|---|---|
| ETH 25852059 | `0x0f4aa044…de40` | 237 | `UserRequestForAffirmation(0xc35B9b69…6687, 23673375455773347526, 0x…1ae0)` |
| GNO 47953922 | `0x28feed67…c16f` | 0 | `SignedForAffirmation(0xAeE7C90E…c656, 0x…1ae0)` |
| GNO 47954052 | `0xb9048640…4f2c` | 0 | `SignedForAffirmation(0x156c0DAA…Fade)` |
| GNO 47954055 | `0x3a016801…8bbe` | 16 | `SignedForAffirmation(0x1312E989…33dca)` |
| GNO 47954055 | `0x0ba868fb…61c7` | 18 / 20 | `SignedForAffirmation(0x82b00cA9…8DfC)` **+** `AffirmationCompleted(0xc35B9b69…6687, 23673375455773347526, 0x…1ae0)` |

Reads: confirmations arrive in separate transactions across ~2 minutes, so they
must be accumulated in the buffer; the completion shares a transaction with the
*last* confirmation; source and destination `value` match byte-for-byte (no fee
configured today).

**Gnosis → Ethereum, nonce `0x…140a`**

| Chain / block | tx | event |
|---|---|---|
| GNO 47945328 | `0x8df5b9c1…bbae` | `UserRequestForSignature(0xa1307726…e14A, 39239013587778384001516, 0x…140a, token=DAI)` |
| ETH 25848470 | `0x3a405d48…a4f4` | `RelayedMessage(0xa1307726…e14A, 39239013587778384001516, 0x…140a)` |

## Invariants

- **One message ⇒ exactly one transfer.** There is no payload field and the
  message blob length is constrained to {104, 124}; arbitrary data reverts.
  `crosschain_transfers.index` is always `0`; `crosschain_messages.payload` is
  always NULL.
- **One transaction ⇏ one message.** `relayTokens` is a plain external function;
  an aggregator can call it in a loop, producing N messages (N nonces) in one
  transaction. Relevant to `evm/transaction_grouping.rs`.
- `nonce` is monotonic and never reused **within a contract**; EternalStorage
  survives upgrades, so it never resets.
- `nonce` is **not** unique across directions.
- Direction is fully determined by `(event name, side)` — no lookup needed:
  `UserRequestForAffirmation`/`SignedForAffirmation`/`AffirmationCompleted` ⇒
  Ethereum→Gnosis; `UserRequestForSignature`/`SignedForUserRequest`/
  `CollectedSignatures`/`RelayedMessage` ⇒ Gnosis→Ethereum.
- **Only the signature (Gnosis→Ethereum) flow has a threshold event.** The
  affirmation (Ethereum→Gnosis) flow emits none: verified against the deployed
  Home v7 implementation `0xe6998b0C…`, whose sole threshold event is
  `CollectedSignatures`, belonging to `submitSignature`. So a "threshold
  reached, destination not yet executed" state — the one a `ReadyToClaim`
  status models — is observable **only** for Gnosis→Ethereum.

  The reason is sharper than "the home contract executes at threshold as a
  courtesy": in `executeAffirmation` the signature increment and the completion
  sit inside the **same** `withinExecutionLimit` branch. Hence
  `count == requiredSignatures()` implies `AffirmationCompleted` in that same
  transaction, and when the limit is exhausted the count does not advance
  either (the `else` branch emits no `SignedForAffirmation`).

  Consequence for a future implementation that split execution off the
  threshold: the state still would not be observable, because
  `requiredSignatures()` is not in any log — it is mutable validator-contract
  state, and `CollectedSignatures.NumberOfCollectedSignatures` carries that
  value rather than the actual count. "N signatures, no completion" is
  therefore indistinguishable from "still collecting". Such a change would have
  to add a new affirmation-threshold event, i.e. a new `topic0` and a new decode
  epoch — so it cannot arrive silently.
- Every event self-keys by nonce **except** `SignedForUserRequest` and
  `CollectedSignatures`, which carry `messageHash`. Both live on the *same*
  chain as their source event, so ordering within one chain stream is
  guaranteed; a `messageHash → key` map is still needed, but only as a
  same-chain lookup plus retry insurance — not the genuinely cross-chain
  problem AMB's `message_hash_lookup` solves.
- `messageHash` is computable by the indexer at `UserRequestForSignature` time:
  every component is either in the event or in config (the foreign bridge
  address), so the lookup can be populated proactively.
- The Gnosis side of a transfer is always native xDAI. The Ethereum side is
  DAI or USDS: for Gnosis→Ethereum it is named by
  `UserRequestForSignature.token` (Home v7+; DAI before); for Ethereum→Gnosis it
  is **not in the logs** and must be resolved from the block-keyed table in
  *Asset model*.
- `native_id` is `initiator_chain_id (4 B) ‖ nonce (28 B)` — globally unique,
  and identical to the key the official bridge explorer uses.

## Implementation Upgrade History

Reconstructed from `Upgraded(uint256,address)` (`topic0` `0x4289d619…`) on both
proxies; every implementation is verified and its ABI was compared.

### Foreign — Ethereum `0x4aa42145…`

| v | block | date | implementation | contract | log-surface change |
|---|---|---|---|---|---|
| 1 | 6478425 | 2018-10-08 | `0x710d6eC2…` | `ForeignBridgeErcToNative` | baseline: `RelayedMessage(address,uint256,bytes32)`; **no source event** |
| 2 | 6914961 | 2018-12-19 | `0x0D3726e5…` | `ForeignBridgeErcToNative` | +`ExecutionDailyLimitChanged`, +`OwnershipTransferred` |
| 3 | 9161003 | 2019-12-25 | `0x75Df5AF0…` | `ForeignBridgeErcToNative` | **+`UserRequestForAffirmation(address,uint256)`**; +`TokensSwapped` (SAI→DAI) |
| 5 | 9884448 | 2020-04-16 | `0x83c2E0E3…` | `ForeignBridgeErcToNative` | +`PaidInterest(address,uint256)` |
| 7 | 13367127 | 2021-10-06 | `0xEeE4f8dB…` | **`XDaiForeignBridge`** | −`TokensSwapped`; `PaidInterest` → `(address,address indexed,uint256)` |
| 8 | 18175639 | 2023-09-20 | `0x166124b7…` | `XDaiForeignBridge` | no event change |
| 9 | 22273407 | 2025-04-15 | `0xb54042F5…` | `XDaiForeignBridge` | **`UserRequestForAffirmation` gains `bytes32 nonce`** |
| 10 | 23748179 | 2025-11-07 | `0x257bDD09…` | `XDaiForeignBridge` | no event change — but `erc20token()` flips **DAI → USDS** at this exact block, and the reserve/yield move to USDS/sUSDS |

Versions 4 and 6 were skipped — `upgradeTo` only requires an increasing number.

### Home — Gnosis `0x7301CFA0…`

| v | block | date | implementation | log-surface change |
|---|---|---|---|---|
| 1 | 763 | 2018-10-08 | `0x37d5B903…` | baseline: `UserRequestForSignature(address,uint256)`, `AffirmationCompleted`, `SignedForAffirmation`, `SignedForUserRequest`, `CollectedSignatures` |
| 2 | 1211852 | 2018-12-19 | `0xCc74F1ff…` | +`AmountLimitExceeded(address,uint256,bytes32)` |
| 3 | 7500284 | 2019-12-24 | `0xC7b4618d…` | +`FeeDistributedFromAffirmation`, +`FeeDistributedFromSignatures`, +`AssetAboveLimitsFixed` |
| 4 | 9171005 | 2020-03-31 | `0x19dD7037…` | no event change |
| 5 | 18443654 | 2021-10-06 | `0x3b388724…` | `AmountLimitExceeded` → `(address,uint256,bytes32 indexed,bytes32)`; +`MediatorAmountLimitExceeded` |
| 6 | 39569937 | 2025-04-15 | `0xB740472C…` | **`UserRequestForSignature` gains `bytes32 nonce`** |
| 7 | 43027713 | 2025-11-07 | `0xe6998b0C…` | **`UserRequestForSignature` gains `address token`** (USDS) |

Foreign v9 / Home v6 landed the same day, as did Foreign v10 / Home v7 — the
sides are upgraded in coordinated pairs.

`config/xdai/bridges.json` encodes the two current windows per side (v9/v10 on
chain 1, v6/v7 on chain 100) with these exact `started_at_block` values.

### Validator set

Read on chain from the Home validator-management contract
`0xB289f0e6fBDFf8EEE340498a56e1787B303F1B6D` (the docs' "4-of-7" is confirmed,
not just asserted):

| | |
|---|---|
| `validatorCount()` | 7 |
| `requiredSignatures()` | 4 |

**The set rotates, and that is a trap for any historical analysis.** Every
address observed signing during the April–May 2025 drain —
`0x4D1c96B9…`, `0x587C0d02…`, `0xfA98B60E…`, `0xc073C8E5…` — returns
`isValidator == false` today, while addresses seen signing recently
(`0x82b00cA9…`, `0x1312E989…`) return `true`. Spot-checked with archive calls:
`isValidator(0x4D1c96B9…)` at Gnosis block 39931990 and
`isValidator(0x587C0d02…)` at 39738154 both return `true`. So membership must
always be read **at the block in question**, never at `latest` — otherwise a
legitimate historical affirmation looks like it came from a non-validator.

The indexer does not check validator membership (the contract already enforces
`_onlyValidator`), so this matters for investigation and for
`amb_messages_confirmations` interpretation, not for correctness of indexing.

### Breaking changes, category A — `topic0` changes (detectable)

| When | Event | Before | After |
|---|---|---|---|
| Foreign v3 @9161003 | `UserRequestForAffirmation` | *(absent)* | `0x1d491a42…` |
| Foreign v7 @13367127 | `PaidInterest` | `0x86dc5ede…` | `0x222348fe…` |
| Foreign v7 @13367127 | `TokensSwapped` | `0xc9eb2692…` | *(removed)* |
| Foreign v9 @22273407 | `UserRequestForAffirmation` | `0x1d491a42…` | `0xf6968e68…` |
| Home v5 @18443654 | `AmountLimitExceeded` | `0x159c0773…` | `0x26c0a209…` |
| Home v5 @18443654 | `MediatorAmountLimitExceeded` | — | `0x3344bbb9…` |
| Home v6 @39569937 | `UserRequestForSignature` | `0x127650bc…` | `0xbcb4ebd8…` |
| Home v7 @43027713 | `UserRequestForSignature` | `0xbcb4ebd8…` | `0xe1e0bc4a…` |

`ContractAbi::version_at` + `LogResolution::WrongVersion` handle this category
as designed.

### Breaking changes, category B — `topic0` unchanged (silent)

1. **`bytes32` changed meaning at Foreign v9 / Home v6 (2025-04-15).**
   `AffirmationCompleted`, `RelayedMessage`, `SignedForAffirmation`,
   `SignedForUserRequest` keep their 2018 signatures. Before the flip the field
   is the **source transaction hash**, which is absent from the source event
   entirely — correlation must be `source_log.transaction_hash ==
   destination_log.bytes32`. After it, correlation is by nonce carried in the
   source event. Verified: Gnosis block 39560137 emitted
   `AffirmationCompleted(0xD5b53759…89c6, 0x06c15c1056adf0c11000,
   0x4865ef26ef1be7b323325775fe55eea2afb03b80f62f7b74b0e714a64f2b8d9d)` and that
   `bytes32` resolves to a real Ethereum transaction in block 22269292.
2. **No source event at all before Foreign v3 (block 9161003, 2019-12-25).**
   For Ethereum→Gnosis below that block the deposit is only visible as an ERC-20
   `Transfer` to the bridge address; the direction's source is not
   reconstructible from bridge logs.
3. **Message blob layout changed at v7/v10** (104 → 104|124 bytes), so the
   `messageHash` preimage differs across the boundary while
   `SignedForUserRequest` / `CollectedSignatures` keep their `topic0`.
4. **The Ethereum→Gnosis source asset changed DAI → USDS at Foreign v10**
   (block 23748179), verified by bisecting `erc20token()` on an archive node:
   DAI at 23748178, USDS at 23748179. `UserRequestForAffirmation` is byte-identical
   across the boundary — the asset lives in contract state, not in the log.
5. `AssetAboveLimitsFixed` first parameter renamed `transactionHash` →
   `messageId` at Home v5; types and `topic0` unchanged.

### Resulting decode epochs

Foreign (Ethereum `0x4aa42145…`):

| Epoch | Blocks | Source event | `bytes32` means |
|---|---|---|---|
| A (v1–v2) | 6478425 – 9161002 | none | tx hash |
| B (v3–v8) | 9161003 – 22273406 | `UserRequestForAffirmation(address,uint256)` | tx hash |
| C (v9–v10) | 22273407 – … | `UserRequestForAffirmation(address,uint256,bytes32)` | nonce |

Home (Gnosis `0x7301CFA0…`):

| Epoch | Blocks | Source event | `bytes32` / blob |
|---|---|---|---|
| A (v1–v5) | 763 – 39569936 | `UserRequestForSignature(address,uint256)` | tx hash / 104 B |
| B (v6) | 39569937 – 43027712 | `UserRequestForSignature(address,uint256,bytes32)` | nonce / 104 B |
| C (v7) | 43027713 – … | `UserRequestForSignature(address,uint256,bytes32,address)` | nonce / 104 or 124 B |

Starting an indexer at `started_at_block` = 22273407 / 39569937 collapses the
*source* side to a single grammar. It does **not** do the same for the
destination side — see below.

### The epoch boundary is not clean on the destination side

A floored indexer still receives tx-hash-identified `bytes32` on destination
events long after the boundary, because identity is fixed when a message is
*initiated*, not when it is completed. Measured against a real incident log
(352 unique `failed to process xDai event` records, `bridge_id = 3`,
2025-04-15 → 2025-05-07), the arrivals fall into three groups:

**1. Orphaned affirmations from validators still on the old notation (348).**
A validator calls `executeAffirmation(recipient, value, bytes32)` passing the
*Ethereum transaction hash* for a message that does have a nonce. Verified
example: Gnosis tx `0x01ab7aa9…` affirms
`(0x9A760aa1…, 25000e18, 0x579c029a…)`, while the Ethereum source
`0x579c029a…` (block 22423136, after the v9 upgrade) emitted
`UserRequestForAffirmation(…, nonce = 0x151)`. The two `hashMsg` buckets are
disjoint, and the counters prove the outcome:

| bucket | signatures | processed |
|---|---|---|
| tx hash | 1 | no |
| nonce `0x151` | 4 | **yes** |

So the broken affirmation lands in its own bucket and never reaches threshold,
while the real message completes on the other six validators. 4-of-7 absorbs one
broken validator with no user impact. It was three validators initially
(`0xfA98B60E…`, `0xc073C8E5…` for ~1.5 h after the upgrade) and then
`0x4D1c96B9…` alone for three weeks, ~19/day.

**2. Legacy Gno→Eth claims (3).** A message initiated before Home v6 and
executed on Ethereum after it. `RelayedMessage` then carries the Gnosis source
transaction hash. Verified: `0x35323f04…` is a Gnosis transaction at block
39557691 (2025-04-14, i.e. below the floor) — a plain native send to the home
bridge. Entirely valid; the backlog drains within days.

**3. A legacy plain-transfer deposit (1).** See *Plain-transfer deposits* above.
The gap here is unbounded: this one was 44 days.

**Indexing rules that follow.** A `SignedForAffirmation` must **never create** a
buffer entry — only attach to one, or queue. Otherwise group 1 manufactures one
phantom `Initiated` message per orphaned signature (343 of them over three
weeks) alongside the correct row. The AMB indexer already has the right shape
(`handle_validator_confirmation` queues unknown keys into
`pending_message_hash_events`); the queue needs a bound, since here it would
accumulate for weeks with nothing ever draining it. And a floored indexer must
still be able to *decode* a tx-hash `bytes32` on destination events rather than
erroring out on it.

## Architecture Fit

**Reusable unchanged:** `LogStream`, `RangeDriver`, `MessageBuffer` +
`Consolidate`, `indexer/evm/*`, `failure_ledger`, `indexer_checkpoints`,
`crosschain_messages`, `crosschain_transfers`, and `amb_messages_confirmations`
(its columns — `message_id`, `bridge_id`, `validator_address`, `tx_hash`,
`block_number`, `block_timestamp` — are protocol-agnostic).

**New work required:**

- A `BridgeType` variant (DB enum has only `lockmint | avalanche_native | amb`)
  and an `IndexerType` variant, plus a branch in `spawn_configured_indexers`.
- A separate `types.rs` / `events.rs` / `consolidation.rs`. Reusing the AMB
  module is unsafe because of the name-based dispatch described above.
- A grammar registry that carries **identity derivation strategy** per version,
  not just an event-name set. `amb_grammar_for` returns event names only; that
  shape cannot express "this epoch keys on the source tx hash".

**Not needed:** `is_collision`, `amb_message_anomalies`, and the displaced-body
machinery — the nonce is contract-issued, so collisions are impossible by
construction (unlike AMB's `messageId`, assembled from caller-supplied fields).
`process_unknown_chains` / `home_chain_id` are also meaningless: the bridge is
fixed two-chain and carries no chain IDs in its events, so chain IDs come from
the config side mapping only.

**Status mapping.** `CollectedSignatures` → `ReadyToClaim` maps exactly as in
`amb/consolidation.rs`. `Completed` follows `AffirmationCompleted` /
`RelayedMessage`. `Failed` has **no source**: neither event carries a status
flag, and a failed execution reverts without emitting. `AmountLimitExceeded`
must **not** be treated as a status transition — see *Failure Modes*.

## Failure Modes / Observability

- **Over-limit parking is neither a failure nor a completion.** The home bridge
  gates incoming (Ethereum→Gnosis) execution on
  `withinExecutionLimit(v) = totalExecutedToday + v <= executionDailyLimit()
  && v <= executionMaxPerTx()` — currently 15 000 000 per UTC day and
  9 999 999 per transfer. On failure `executeAffirmation` **does not revert**:
  `onFailedAffirmation` records a `(recipient, value)` marker under `hashMsg`,
  adds the value to `outOfLimitAmount()`, and emits
  `AmountLimitExceeded(recipient, value, nonce, hashMsg)`. Critically it emits
  **no `SignedForAffirmation`** and does not advance the signature count — the
  message has not moved, while the funds are already locked on Ethereum. Only
  the *first* validator to hit the limit leaves the event; the rest revert on
  `require(recipient == address(0) && value == 0)`.

  Two exits: validators retry later (new UTC day, or raised limits), which runs
  the normal path and lets `_clearAboveLimitsMarker` erase the marker before the
  usual `AffirmationCompleted`; or governance calls `fixAssetsAboveLimits`,
  reported by `AssetAboveLimitsFixed` (partial release via its `remaining`
  field).

  That is the mechanism when parking really does precede execution. In practice
  it almost never does — see the next entry.

  **Parking is not a claim window.** It is a distinct third state: an execution
  attempt refused by the limit gate, with the signature count still *short* of
  threshold. Both genuinely undelivered messages below sit at "3 of 4". So it
  must never be mapped to a `ReadyToClaim`-style status, which means "threshold
  reached, execution pending" — a state the affirmation flow cannot reach at all
  (see *Invariants*).

- **Most `AmountLimitExceeded` events are phantoms on already-delivered
  messages.** `executeAffirmation` guards against reprocessing **only inside**
  the within-limit branch:

  ```solidity
  if (withinExecutionLimit(value)) {
      ...
      require(!isAlreadyProcessed(signed));    // guard is here
      ...
  } else {
      onFailedAffirmation(recipient, value, nonce, hashMsg);   // no guard here
  }
  ```

  The threshold is 4-of-7, so the remaining validators still submit their
  affirmations after a message is complete. Normally those revert on
  `require(!isAlreadyProcessed(...))`. But if the day's execution limit happens
  to be exhausted at that moment, control takes the `else` branch, which has no
  such guard, and an **already-paid** transfer is parked at full value and added
  to `outOfLimitAmount()`.

  This is near-deterministic for large transfers rather than a rare race:
  immediately after a 9.5M transfer executes, `totalExecutedPerDay` already
  includes those 9.5M, so a straggler carrying the same 9.5M evaluates
  `9.5M + 9.5M = 19M > 15M` and is guaranteed to take the `else` branch. Hence
  every large transfer collects a phantom marker while small ones almost never
  do.

  Ordering is provable from state alone: if parking came first and completion
  second, `_clearAboveLimitsMarker` would have zeroed the marker. A non-zero
  marker on a message whose `numAffirmationsSigned` carries the `2**255`
  processed bit therefore means **completion happened first, parking second**.

  Measured (full log sweep of both `AmountLimitExceeded` topic variants, plus a
  direct read of every marker from contract storage):

  | | value | entries |
  |---|---|---|
  | parked ever | 115 072 943.91 | 91 |
  | markers still non-zero | 110 030 538.23 | 23 |
  | `outOfLimitAmount()` on chain | 110 338 737.75 | — |
  | counter minus live markers | 308 199.52 | — |
  | └ of the 23: **phantom** (already delivered) | 110 030 456.49 | 21 |
  | └ of the 23: **genuinely undelivered** | **81.74** | **2** |

  The 308 199.52 gap is exactly the sum of all 61 pre-v5 events: those were
  added to the counter but their markers live under pre-v5 storage keys and can
  never be cleared. `AssetAboveLimitsFixed` has **never** been emitted — not
  because nobody bothered, but because there was almost nothing to rescue.

  So `outOfLimitAmount()` is **not** a "stuck funds" metric. Its 110M is
  ~99.9999 % phantom accounting plus an unclearable legacy tail.

  Phantom example — Gnosis block 45970539 (2026-05-02), tx
  `0x23487388d0596c4c321579432b75ba9036e7930c34061f09f38f21bc4c7c4279`, log
  index 14, submitted by validator `0x1312E989…33dca`:
  `AmountLimitExceeded(0xB4fb31E7…59c1, 9575375598895643874338853,
  nonce 0x…1539, hashMsg 0xbed4cc21…0c80)`. `numAffirmationsSigned(hashMsg)` =
  `2**255 | 4`, i.e. the transfer was already delivered — the official bridge
  explorer shows it as an ordinary completed transfer, and this log sits in a
  *different* transaction from the one that completed it.

  The only two genuinely undelivered messages, both from the tx-hash era (so not
  addressable by the `chain_id ‖ nonce` explorer key):

  | Gnosis block | recipient | value | sigs | tx |
  |---|---|---|---|---|
  | 28090839 | `0x5F41e307…f89B` | 41.9855 | 3 of 4 | `0xb66f69f7…0bbf7` |
  | 28093945 | `0xC923CeBC…9715` | 39.7557 | 3 of 4 | `0x6cfb81dd…d1815` |

  Reproducing the storage read: `uintStorage` is slot 0 (verified against
  `nonce()`), the marker is
  `uintStorage[keccak256(abi.encodePacked("txOutOfLimitValue", hashMsg))]`, and
  `hashMsg = keccak256(recipient ‖ value ‖ nonce)` (validated against the
  emitted `messageId`).

  **Indexing rules that follow.** `AmountLimitExceeded` is *not* by itself
  evidence of a stall, and it can arrive **after** the message completed — an
  indexer must never downgrade a `Completed` message on seeing it, or every
  large transfer will be misreported as stuck. A real stall is "marker set AND
  message not processed", and processedness is **not in the logs** (it lives in
  `numAffirmationsSigned`). From logs alone the closest available test is
  "`AmountLimitExceeded` for a nonce with no `AffirmationCompleted` for that
  nonce, before or after". Given that only 2 messages in the bridge's entire
  history are genuinely stuck, modelling this state is a low-priority nicety,
  not a correctness requirement.
- **Source-side daily limit** rejects at `relayTokens` with a revert, so nothing
  is indexed at all — the message simply never exists.
- **Block-reward lag.** `AffirmationCompleted` is a mint *instruction*; the xDAI
  credit lands in a later block via `AddedReceiver`, which carries no message
  key. "Bridge confirmed" and "funds arrived" are different blocks and cannot be
  linked deterministically from logs.
- **Hashi is live but does not affect indexing today.** Verified on chain:
  `HASHI_IS_ENABLED == true` and `HASHI_IS_MANDATORY == false` on both sides,
  with real managers set (`0x9acCFAD7…` on Ethereum, `0x60Aa1519…` on Gnosis),
  so messages *are* dispatched through Yaho. It is nonetheless invisible to a
  log-based indexer:
  - the outbound leg (`_maybeSendDataWithHashi` → `IYaho.dispatchMessage`) emits
    its logs from the **Yaho** contract, not from the bridge address;
  - the inbound leg `onMessage(...)` lives on the bridge but only writes
    storage — it emits nothing;
  - `require(isApprovedByHashi(hashMsg))` is gated on `HASHI_IS_MANDATORY`, so
    it is not enforced; execution still turns purely on validator signatures;
  - no event signature is affected.

  It would start to matter only if `HASHI_IS_MANDATORY` became `true`, adding a
  "signed but not Hashi-approved" state that is unobservable from bridge logs.
  Because that flag is a Solidity `constant`, not storage, flipping it requires
  a **new implementation** — so it necessarily shows up as a new version in the
  `Upgraded` history. Watching upgrades is sufficient; no runtime probe needed.
- **Fees** are configured but appear inactive: on both observed pairs the source
  and destination `value` match exactly. `FeeDistributedFrom*` events are the
  signal that this changed.

## Edge Cases / Gotchas

- Foreign v3's implementation address `0x75Df5AF045d91108662D8080fD1FEFAd6aA0bb59`
  is byte-identical to the AMB **home proxy on Gnosis** in
  `config/omnibridge/bridges.json`. Different chains, so the
  `(chain_id, address)` key in `abi.rs` disambiguates — but anything keyed on
  address alone breaks here.
- `RelayedMessage`'s parameter is still named `transactionHash` and
  `Message.sol`'s layout comment still says "transaction hash", although both
  now carry the nonce. Parameter names are not part of `topic0`, so this drift
  is invisible to ABI-based tooling.
- `CollectedSignatures.NumberOfCollectedSignatures` is `requiredSignatures()`,
  not the number actually collected.
- The destination token for Gnosis→Ethereum is selected by *caller identity*
  (`msg.sender == usdsDepositContract` ⇒ USDS, else DAI), not by user input.
- Legacy 104-byte messages implicitly mean DAI — hardcoded in `parseMessage`.
- A plain ERC-20 `transfer` to the bridge address no longer bridges anything
  (`recoverUSDS`'s own comment: *"the Transfer event will no longer be
  supported"*). Only `relayTokens` produces a message, so there is no need to
  subscribe to token `Transfer` events — except for Foreign epoch A, where that
  was the *only* signal.
- Setting `native_id` to the explorer's
  `initiator_chain_id ‖ nonce` blob makes `ui_url` work through the existing
  `{{message_id}}` template — but the encoding has no defined form for the
  pre-2025-04-15 tx-hash era.
- `sender_address` must come from the transaction origin; no event carries a
  sender. The same trap as the AMB gotcha *"AMB Header Sender Is Not The Source
  Transaction Initiator"* applies: an aggregator or Safe sending the transaction
  makes `receipt.from` the aggregator, not the user. The router and deposit
  wrappers do **not** add to this — see the `BridgeRouter` entry below.
- **`BridgeRouter` is a shared entry point for the xDai bridge *and*
  Omnibridge** (and the WETH omnibridge router), but it is **never an indexing
  anchor**. Three independent reasons:
  - it is not the only path — the router's own USDS route points straight at the
    bridge, and users can call `relayTokens` on the bridge, or on the Gnosis
    USDS deposit contract, directly. Anchoring on the router would silently drop
    all direct traffic;
  - it emits **no events** on the relaying path (nor does
    `XDaiBridgePeripheral`) — it only calls through, so there is nothing to
    subscribe to. The indexer is address-anchored anyway: `AbiRegistry` keys on
    `(chain_id, address)` and `filter_for_chain` builds `eth_getLogs` over the
    configured bridge addresses × topics;
  - it is a `TransparentUpgradeableProxy` with a mutable `setRoute(onlyOwner)`
    dispatch table, whereas the bridge proxies have been stable since 2018.

  What it *does* change is that `tx.to` carries no bridge attribution — that
  must come from the log's emitting address. **Checked:** neither
  `indexer/amb/` nor `indexer/evm/` uses `tx.to` in production code (all `to:`
  occurrences are block-range bounds or test mocks), and the source initiator is
  taken as `transaction_from = receipt.from`
  (`indexer/evm/receipt_fetch.rs`), so the shared entry point creates no bug in
  the existing AMB indexer.

  `receipt.from` is unaffected by the router in either direction: it is the
  transaction origin, so it stays the user whether they enter through the router
  or the bridge — and even on the DAI path, where the bridge sees the peripheral
  as `msg.sender`, the indexer reads `receipt.from`. The real `sender_address`
  trap is the pre-existing one (an aggregator or Safe sending the transaction),
  unrelated to the router.

  A single transaction could in principle carry events of both bridges — not
  from one router call, which is an if/else, but from a batching contract — so
  `evm/transaction_grouping.rs` is worth checking against that case. The router
  also proxies claims via `executeSignatures` and
  `safeExecuteSignaturesWithAutoGasLimit`.

## Change Triggers

Update this note when: either proxy is upgraded again — re-read
`Upgraded(uint256,address)`, re-diff the ABIs, **and re-check `erc20token()`**,
since the reserve asset can move without any ABI change; an xDai indexer is
actually implemented (the Architecture Fit section becomes a description rather
than a proposal); `MessageStatus` gains a state for parked/over-limit messages;
`HASHI_IS_MANDATORY` flips to `true` (only possible via a new implementation, so
it will appear in the upgrade history); or a fee manager is configured and
`FeeDistributedFrom*` starts firing.

## Open Questions

- **No verified `CollectedSignatures` log instance is recorded in this note.**
  The Gnosis→Ethereum worked trace (nonce `0x…140a`) contains only
  `UserRequestForSignature` and `RelayedMessage`, so the claim window is
  established from the contract flow and the deployed ABI rather than from a
  logged example. A sweep of `topic0 = 0x41555740…` on the Gnosis home proxy,
  paired with the matching `RelayedMessage`, would give a hard empirical anchor
  (and the real threshold→execution latency).
- Is `MediatorAmountLimitExceeded` ever actually emitted by the current Home
  implementation, or is it dead inherited surface?
- Does `submitSignature` bind the signed blob to a real home-originated
  request? Relevant because the blob layout accepts any valid 104/124-byte
  message, so an indexer that derived its key by parsing the nonce out of the
  submitted blob — rather than from the source event — could in principle have a
  validator-supplied signature land on a key from the opposite direction.
- Should the two genuinely stuck messages (81.74 xDAI total) get a dedicated
  `MessageStatus`, or is leaving them `Initiated` acceptable given the volume?
  Modelling them properly requires state the logs do not carry.
- Exactly when did validators stop honouring plain-transfer deposits? Bracketed
  to somewhere between 2025-04-25 and Gnosis block 45473867; narrowing it needs
  a sweep of `AffirmationCompleted` across the intervening range for
  legacy-format identifiers.
- Was the validator set rotated once or several times since May 2025, and when?
  Only spot-checked at two blocks; the full `ValidatorAdded`/`ValidatorRemoved`
  history on `0xB289f0e6…` was not swept.
- Surfacing unhonoured plain transfers (money sent to the bridge that never
  became a message) is a plausible separate feature. Does anyone want it, and is
  the internal-flow allowlist it requires maintainable?
