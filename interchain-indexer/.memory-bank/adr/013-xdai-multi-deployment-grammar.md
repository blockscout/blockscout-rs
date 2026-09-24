# ADR-013: xDai Deployment Constants Stay In Code, Keyed By Chain Id

**Date:** 2026-09-18

**Authors:** @EvgenKor

## Context

`indexer/xdai/version.rs` held four Ethereum/Gnosis **mainnet** facts as
module-level constants, alongside the protocol grammar they belong to:

- `FOREIGN_EPOCH_FLOOR_BLOCK` (22273407) and `HOME_EPOCH_FLOOR_BLOCK`
  (39569937) — the first block of the current identity epoch on each side.
  Below them the same `topic0`s carry a source transaction hash in the
  `bytes32` field rather than a nonce, with no on-chain signal, so
  `AbiRegistry::from_chains` refuses any `started_at_block` beneath them.
- `XDaiGrammar::source_asset` — the ERC-20 the Foreign bridge held during a
  version window (DAI for v9, USDS for v10).
- `legacy_ethereum_asset` with `LEGACY_DAI_EPOCH_START_BLOCK` (9161003) and
  `USDS_EPOCH_START_BLOCK` (23748179) — the same fact for the legacy
  reconstruction path, which resolves an asset at an arbitrary Ethereum block
  referenced by a destination affirmation, including blocks far below the
  configured window.

Chain ids had already been made config-driven (`Direction` names a *side*,
`AbiRegistry::chain_ids()` resolves it), but that is only half of what a second
deployment needs. `.memory-bank/research/xdai-bridge-testnet-deployment-fit.md`
established that a real classic xDai bridge exists on Sepolia ↔ Chiado, that
its event grammar is byte-for-byte the mainnet grammar, and that the *only*
obstacles were those four constants: the floor `ensure!` rejects testnet blocks
an order of magnitude lower, and the asset table names tokens that do not exist
on Sepolia.

There is a second, subtler problem. `bridge_contracts.version` for xDai mirrors
`EternalStorageProxy.version()`, an upgrade counter that **restarts at 1 for
each deployment**. Ethereum reports 9/10 and Gnosis 6/7; Sepolia reports 2 and
Chiado 3. `getBridgeInterfacesVersion()` cannot discriminate either — it returns
`6.1.0` on all four proxies.

Related prior art: ADR-006 established that a contract version is resolved by
`(address, block)` at decode time. This decision is the layer above — which
*deployment's* protocol constants a resolved version belongs to.

## Decision

**The constants stay in code and become per deployment, selected by chain id.
`bridges.json` does not change.**

1. **Epoch floors hang off the grammar.** `XDaiGrammar` gains
   `epoch_floor_block`, so each version window declares the floor of the
   deployment it belongs to. `AbiRegistry::from_chains` reads the floor off the
   grammar it just selected instead of a module constant; the `ensure!` itself
   is unchanged.

2. **`grammar_for` is keyed on `(chain_id, side, version)`.** `XDaiGrammar`
   gains `chain_id` as a deployment discriminator. Config writes the proxies'
   real `version()` counters — Sepolia Foreign 2, Chiado Home 3 — so the config
   never contradicts the chain.

3. **`legacy_ethereum_asset` takes the Foreign chain id** as its first
   parameter and branches per deployment before branching on direction. The
   Ethereum arm is unchanged, including its `bail!` below 9161003.

4. **The Home v6 fallback follows the same key.** `consolidation.rs`'s
   `source.event.token.unwrap_or(DAI)` becomes
   `legacy_home_ethereum_asset(chain_ids.foreign)`, resolved from the chain ids
   already stamped on the buffered message.

5. **`assert_epoch_boundaries_agree` stays**, now comparing a window's
   `source_asset` against the *same deployment's* legacy table.

Adding a third deployment is a `version.rs` change: grammar windows, floor
constants, asset constants. It is not a schema change and not a migration.

## Alternatives Considered

### Alternative 1: Move the constants into `bridges.json`

A per-bridge `xdai` object carrying `foreign_epoch_floor_block`,
`home_epoch_floor_block` and an ordered `(from_block, asset)` list, optional so
that AMB and Avalanche entries keep parsing — the pattern `home_chain_id` and
`reconstruct_incoming_ictt_transfers` already set.

**Pros:**
- A new deployment is a config change, deployable without a release.
- Collapsing `source_asset` and `legacy_ethereum_asset` into one ordered list
  would remove their dual source of truth outright and make
  `assert_epoch_boundaries_agree` a well-formedness check on that list.

**Cons:**
- Every block boundary involved is **already in `contracts[]`** as a
  `started_at_block`. Declaring the floors and the DAI→USDS boundary again in a
  sibling field states the same number twice, which is precisely the divergence
  `assert_epoch_boundaries_agree` had to be written to catch — the alternative
  would have removed one instance of that class of bug and created another.
- These are not operator preferences. They are protocol facts about a fixed,
  already-deployed contract set, established by on-chain investigation and
  meaningless to change. Config is for what a deployment may legitimately vary.
- It widens a schema shared by three bridge types, and the "must be present iff
  `bridge_type == Xdai`" validation is a new failure mode for the other two.

### Alternative 2: Derive the floors from `contracts[]`

Take each side's floor to be the earliest configured `started_at_block` for
that chain.

**Pros:**
- No duplication at all, and no new fields.

**Cons:**
- It makes the `ensure!` a tautology that can never fail, so the guard is
  **deleted rather than relocated**. Its job is to stop an operator configuring
  a window below the nonce epoch, where the identity semantics silently change;
  derived from the config, a too-low `started_at_block` produces no error.
- The analogous derivation for the asset table ("a block below every window
  takes the earliest window's asset") would also drop
  `legacy_ethereum_asset`'s `bail!` below 9161003, turning a loud failure into
  a guess on a live path — a mainnet behaviour change smuggled in alongside
  testnet support.

### Alternative 3: Key `grammar_for` on `(side, version)` and write grammar labels in config

Write Foreign `9` and Home `6`/`7` in the testnet config even though the
proxies report 2 and 3, treating `version` as a grammar label.

**Pros:**
- No code change to the lookup; AMB already writes protocol version numbers
  rather than proxy counters.

**Cons:**
- The config contradicts `EternalStorageProxy.version()`, which is a trap for
  whoever next debugs the deployment.
- Worse, it leaves the selection ambiguous in the other direction: a *mainnet*
  config could select the Sepolia grammar — an epoch floor 14M blocks too low
  and a `source_asset` that does not exist on Ethereum — merely by writing
  `version: 2`. Nothing would reject it, and
  `check_source_asset_matches_latest` is non-fatal by design.

## Consequences

### Positive

- A second deployment works end to end, and the mainnet resolution is provably
  unchanged: the shipped `config/xdai` and `config/full-mainnet` files are
  loaded in tests and asserted to produce the same floors, the same DAI→USDS
  boundary at 23748179 and the same pre-9161003 `bail!`.
- Deployment selection is unambiguous. A version number under the wrong chain
  id is a hard startup error naming every registered deployment, rather than a
  silent mis-selection.
- `check_source_asset_matches_latest` compares against the configured
  deployment's newest asset, so it stops being guaranteed-wrong on testnet and
  becomes a real signal there. It remains non-fatal.
- `bridges.json` keeps one shape for every bridge type; AMB and Avalanche
  entries are untouched.

### Negative

- Adding a deployment requires a release. Accepted: these are protocol
  constants derived from on-chain investigation, not operational knobs, and
  each addition needs a research note anyway.
- The grammar table now names chain ids, which reads like a partial reversal of
  making chain ids config-driven. It is not: the ids select *which deployment's
  constants apply* and are validated against the config. Every chain id an xDai
  message writes still comes from `bridges.json` through
  `AbiRegistry::chain_ids()`.
- `source_asset` and `legacy_ethereum_asset` remain two statements of the same
  fact, still reconciled by `assert_epoch_boundaries_agree` rather than made
  structurally impossible to disagree. Collapsing them is a worthwhile
  follow-up and is independent of multi-deployment support.

### Neutral

- `legacy_home_ethereum_asset` is deliberately **total** rather than fallible.
  It is read from `Consolidate::consolidate`, which runs inside the maintenance
  plan, where an `Err` aborts the whole bridge's cycle (see
  `.memory-bank/rules/error-handling.md`, "Expected Skips Inside a Shared
  Transaction"). An unrecognised chain id falls back to DAI, exactly as the
  pre-multi-deployment code did unconditionally.
- No Chiado Home v2 grammar window exists: Home v2 lives entirely below the
  Chiado floor, so no config can select it, and a grammar the config cannot
  reference is a claim the code cannot back.
- **The testnet bridge indexes its two sides over ranges eleven months apart**
  (Sepolia from 2025-05-02, Chiado from 2026-04-02). This is deliberate. The
  two floors answer different questions — Foreign: from where the source event
  is decodable; Home: from where the destination identity is unambiguous — and
  nothing makes them coincide. Lowering the Chiado floor to the Home v2 upgrade
  was investigated and rejected on evidence: four of the six hash-keyed
  affirmations it would admit resolve to Sepolia transactions emitting the
  *modern* three-argument source event, which `decode_legacy_source_event`
  rejects outright, so those blocks would fail and retry forever rather than
  merely duplicating rows. Re-keying such an affirmation to its source event's
  nonce would fix that, but for three of the four it then collides with
  `ensure_completion_compatible`, because the Chiado oracle genuinely affirmed
  and paid those deposits twice in two distinct transactions. Closing the gap
  therefore means choosing which of two real payouts is *the* completion and
  relaxing an invariant shared with mainnet, to gain three messages on a bridge
  with four deposits. The per-affirmation evidence is in the research note under
  *Why the two sides' ranges differ by eleven months*.

## References

- `.memory-bank/research/xdai-bridge-testnet-deployment-fit.md` — the on-chain
  investigation behind every testnet value, and the Chiado floor's
  no-duplicate-rows argument.
- `.memory-bank/research/xdai-bridge-protocol-and-indexing-fit.md` — the
  mainnet protocol primer.
- `.memory-bank/gotchas.md`, "xDai Deployments Are Keyed On
  `(chain_id, side, version)`".
- ADR-006 — contract versioning resolved by block, the layer this sits above.
