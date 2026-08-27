# Chart Update-Range Anchoring, Backfill Detection, and Indexer-Sync Gating

> Labelling convention: **[F]** = fact verified in the current source; **[I]** =
> inference / reading of the code not itself written down anywhere; **[Q]** = open
> question needing a human decision or a runtime experiment. Paths are
> repo-relative to `blockscout-rs/`.

> **Status — read this first.** §"Design Options" was **resolved** on 2026-08-26 and
> implemented on branch `evgenkor/stats/interchain-historical-backfill`. Option
> **A** was selected: the sync verdict is taken from the indexer's
> `GET /api/v1/status/indexing`, whose `catchup_complete` field
> (`progress_percent == 100.0 && failed_blocks == 0`) landed in `2d2ce3e6`, and it
> is scoped by bridge **and** chain. **There is no indexer-DB fallback** — an
> earlier draft had one and it was dropped deliberately: an unreachable indexer
> API means the indexer is not writing new rows either, so a DB fallback adds a
> second code path that can only ever agree with "keep recomputing", which is
> already what an unreachable API produces. Two factual corrections found while
> selecting are marked **[CORRECTION]** below: the catch-up-complete predicate
> **is** derivable from the indexer DB after all (so the fallback was dropped for
> the reason above, not for being impossible), and Options B′/C′ are
> **unsound** since per-chain indexing was decoupled inside a bridge. Everything
> else in this note stands.

## Scope

Covered:

- how a chart's update range start is chosen on each cycle, and what "anchors" it;
- the `chart_data.min_blockscout_block` channel as a **backfill detector**, and
  why it works in `Blockscout` / `MultichainAggregator` mode but not in `Interchain`;
- the concrete consequence: whether stats picks up indexer data that appears
  *behind* the anchor (historical backfill), per chart kind;
- the recovery paths that already exist;
- how "wait for the indexer" (`conditional_start` / `IndexingStatus`) is built for
  the other modes, and why it is structurally absent in `Interchain` mode;
- what the interchain-indexer's own indexing-progress handle offers, and which
  parts of it can and cannot be replicated from its database;
- **how a sync signal would be scoped** across many bridges, many chains, and
  chains shared between bridges, given that stats' filtering is optional;
- design options for closing the gap, and their trade-offs.

Intentionally out of scope:

- the interchain read-filter semantics themselves — see
  `interchain-mode-and-filtering.md`, which this note deliberately does **not**
  duplicate;
- the indexer's internal catch-up algorithm (cursors, range driver, failure
  ledger) beyond what stats would need to read;
- restating the decision inline: §"Design Options" below is the original,
  pre-decision analysis of the options that were evaluated. The outcome of
  that evaluation (Option A, resolved 2026-08-26) is recorded once, in the
  Status banner above — the section itself is left as it was written, not
  rewritten to read as a decision it wasn't at the time.

## Short Answer

**[F]** A chart's next update starts at `last_accurate_point + 1 timespan`, and
`last_accurate_point` is derived from `charts.last_updated_at`, not from the data.
The *earliest* date in the indexer (`get_min_date`) is consulted **only** when
`last_accurate_point` is `None` — i.e. on the very first update or under
`force_full`. So the first successful update permanently anchors the chart's
history floor.

**[F]** The one signal that unfreezes that floor is `chart_data.min_blockscout_block`:
when the recorded value differs from the observed one, `last_accurate_point` returns
`None` and the chart recomputes in full. In `Blockscout` and `MultichainAggregator`
mode that column is a min-block indicator that *decreases as indexation continues*,
so it is a genuine backfill detector. In `Interchain` mode the same column is
reused as the read-filter **fingerprint**, which hashes only operator-configured
values — nothing about how much the indexer has indexed.

**[I]** Consequence for the scenario "index a bridge from an empty DB, start stats
shortly after, let the indexer backfill the full history": interchain **counters**
self-heal (they re-`COUNT(*)` the whole table every cycle and ignore
`last_accurate_point`), while **every line chart** — base daily, all lower
resolutions, and the cumulative `*_growth_*` families — never revisits the
backfilled range. The cumulative charts are worst affected: a missing prefix
offsets the entire series. Nothing detects this, and nothing logs it.

**[F]** The interchain indexer's catch-up runs **downwards** — realtime starts at
the current latest block `N`, catch-up starts at `N - 1` and walks down to the
configured start block. So `MIN(init_timestamp)` over the indexed data falls
monotonically while catch-up progresses, which makes it a well-matched signal
rather than a lucky heuristic.

## Why This Matters

**[I]** The failure is silent and permanent. The charts keep updating, keep
reporting fresh `last_updated_at`, and keep serving. The only visible symptom is a
cross-check that nothing performs automatically: the counter
`totalInterchainMessagesSent` diverges from the last point of
`messagesGrowthSentInterchain`, because one recounts everything and the other does
not.

**[I]** It is also mode-specific in a way that is easy to miss. Someone who knows
that stats "handles reindexing" from Blockscout mode will reasonably assume the
same holds in interchain mode. It does not, and the reason is one repurposed column.

**[F]** The same *shape* of failure is already documented in the codebase for a
neighbouring cause. `resolve_only_indexed_by_bridge`'s comment
(`read/interchain.rs:202-208`) explains that a cycle reading a transient horizon
"writes under-counted points, and nothing later repairs them: the fingerprint
deliberately excludes the horizon's contents, so the next cycle sees no mismatch
and only updates forward from `last_accurate_point`." That is this note's failure
mode, reached through the horizon instead of through the backfill. The general
statement — repurposing the column removed the backfill detector — is written
down nowhere.

## Source-of-Truth Files

### Anchoring and backfill detection

- `stats/src/data_source/kinds/local_db/mod.rs` — `update_itself_inner` (L164-273):
  reads the observed min-block/fingerprint per mode, the interchain
  clear-on-mismatch gate (L225-251), then `last_accurate_point` → `Update::update_values`.
- `stats/src/charts/db_interaction/read/local_db.rs` — `recorded_min_indexer_block`
  (L435), `last_accurate_point` (L455; the equality compare is L479).
- `stats/src/data_source/kinds/local_db/parameters/update/batching/mod.rs` —
  `BatchUpdate::update_values` (L60-119): where `update_range_start` is computed.
- `stats/src/data_source/kinds/local_db/parameters/update/point.rs` — `PassPoint`:
  the counter path that ignores `last_accurate_point`.
- `stats/src/charts/db_interaction/read/mod.rs` — `get_min_date` (L59) and its
  per-mode dispatch.
- `stats/src/charts/db_interaction/read/{blockscout,multichain,interchain}.rs` —
  `get_min_block_*` / `get_min_date_*`; `resolve_only_indexed_by_bridge` (L159).
- `stats/src/charts/db_interaction/filters/interchain.rs` — `InterchainFilter`,
  `filter_fingerprint` (L243) and its bit-layout guarantees.
- `stats/src/charts/db_interaction/write.rs` — `insert_data_many` (L38), the
  upsert; `update_column(MinBlockscoutBlock)` at L52.
- `stats/entity/src/chart_data.rs:16` + `stats/migration/src/m20220101_000001_init.rs:30`
  — the column: nullable `bigint` → `Option<i64>`.
- `stats/src/charts/chart.rs` — `approximate_trailing_points` (L234).
- `stats-server/src/settings.rs` — `InterchainFilterSettings` (L149): the six
  filter dimensions.

### Indexer-sync gating

- `stats-server/src/blockscout_waiter.rs` — `IndexingStatusAggregator`,
  `IndexingStatusListener`, `run` (L243), `init` (L331),
  `init_blockscout_api_client` (L374).
- `stats/src/charts/indexing_status.rs` — the three-axis `IndexingStatus`.
- `stats-server/src/update_service.rs` — `wait_for_start_condition` (L151),
  `run_cron` (L394), `update` (L406; the per-cycle interchain horizon probe is L437).
- `stats-server/src/settings.rs` — `apply_interchain_mode_settings` (L494),
  `StartConditionSettings` (L531) and its `*_checks_enabled` helpers (L553-561).
- `stats/src/charts/db_interaction/read/zetachain_cctx.rs` —
  `query_zetachain_cctx_indexed_until` (L22): the DB-derived-status precedent.

### interchain-indexer (sibling service; read-only reference)

- `interchain-indexer-proto/proto/v1/status.proto` — `GetIndexingProgress`;
  REST as `GET /api/v1/status/indexing`
  (`interchain-indexer-proto/swagger/v1/interchain-indexer.swagger.yaml:1255`).
- `interchain-indexer-server/src/services/status.rs` — `collect_indexing_progress` (L92).
- `interchain-indexer-server/src/indexers.rs` — `IndexingTarget` (L565),
  `enumerate_indexing_targets` (L585).
- `interchain-indexer-logic/src/indexer/progress.rs` — `CatchupProgress::compute`
  and the module doc on what the percentage is *not*.
- `interchain-indexer-entity/src/codegen/{indexer_checkpoints,indexer_failures}.rs`.
- `.memory-bank/research/indexing-gaps-retries-and-checkpoint-safety.md` §1 and §7
  — scan boundaries (catch-up direction) and catch-up completion.
- `.memory-bank/research/message-lifecycle.md` — which side of a message creates
  the row; `.memory-bank/research/avalanche-bridge-filtering.md:171` — where
  `init_timestamp` comes from and how it degrades.

## Key Types / Tables / Contracts

| name | role |
|---|---|
| `charts.last_updated_at` | the anchor. Set to `cx.time` (or the batch range end) after each successful update |
| `chart_data.min_blockscout_block` | the "recompute everything" channel. Nullable `bigint`. **Mode-dependent meaning** |
| `last_accurate_point()` | turns the anchor + the min-block comparison into "where does the next update start" |
| `approximate_trailing_points()` | 1 for lines, 0 for counters — how far back from the anchor is considered unfinished |
| `BatchUpdate` / `Batch30Days` | the batching that walks `update_range_start → now` |
| `PassPoint` | the counter update behaviour — ignores `last_accurate_point` entirely |
| `filter_fingerprint()` | FNV-1a over the 5 configured filter dimensions + `horizon_enabled`, masked to 63 bits, `0`/`i64::MAX` remapped to `1` |
| `IndexingStatus{blockscout,user_ops,zetachain_cctx}` | the wait-for-indexer requirement model |
| `indexer_checkpoints`, `indexer_failures` (indexer DB) | the catch-up cursors and the unresolved-hole ledger |

## Step-by-Step Flow

**[F]** Per chart, per update cycle (`update_itself_inner`):

1. If `charts.last_updated_at == cx.time`, return — the chart was already handled
   in this group update.
2. Read the observed min-indexer-block for the mode:
   - `Blockscout`/`Zetachain` → `MIN(blocks.number) WHERE consensus`
   - `MultichainAggregator` → `SUM(block_ranges.min_block_number)`
   - `Interchain` → `filter.fingerprint` (**not a DB read at all**)
3. Read `recorded_min_indexer_block` — the value stamped on the chart's newest
   stored point (`ORDER BY date DESC LIMIT 1`). Skipped only when
   `force_full && mode != Interchain`.
4. **Interchain only:** if a recorded value exists and differs from the observed
   one, `clear_chart_data_and_updated_at` (rows + `last_updated_at`, one
   transaction). This exists because `insert_data_many` is an upsert with no
   delete, so a *narrowed* filter would leave stale rows.
5. `last_accurate_point(observed, recorded, force_full, approximate_trailing_points, policy)`:
   - `force_full` → `None`
   - `recorded != observed`, or `recorded` absent, or `last_updated_at` absent → `None`
   - otherwise → read the stored series around `last_updated_at` and return the
     newest **non-approximate** point.
6. `Update::update_values(...)`:
   - **`BatchUpdate` (all line charts):**
     `update_range_start = last_accurate_point + 1`, or `get_min_date(cx)` when it
     is `None`. Then `generate_batch_ranges(update_range_start, now, BatchSize)`,
     and `update_metadata` after **every** batch.
   - **`PassPoint` (all counters):** `last_accurate_point` is `_`-ignored; the
     dependency is queried over `UniversalRange::full()` and one point is upserted.
7. `Update::update_metadata(stats_db, chart_id, cx.time)`.

**[F]** The interchain history floor, when step 6 needs it:
`get_min_date_interchain` = `MIN(crosschain_messages.init_timestamp)` over the
filter's messages query and the joined transfers query, `unwrap_or(now)` on an
empty result. It is `UpdateCache`-keyed on the statement text, so it costs one
query per group update at most.

## Invariants

**[F]** Established:

- A chart's stored history below `last_accurate_point` is never recomputed unless
  the min-block/fingerprint comparison fails or `force_full` is set.
- `min_blockscout_block` in `Blockscout` mode can only decrease as indexation
  proceeds; in `MultichainAggregator` mode the *sum* of per-chain minima has the
  same property (stated verbatim in `multichain.rs:18-20`). Both therefore fire
  on backfill and on a new chain appearing.
- In `Interchain` mode the fingerprint is a pure function of operator-configured
  values plus a boolean. It is invariant under any change in indexer content.
- `filter_fingerprint`'s output is always in `[1, 2^63 - 2]`: masked to 63 bits,
  with `0` and `i64::MAX` remapped to `1`. **The sign bit is therefore always
  zero, i.e. unused.**
- Nothing performs arithmetic, sign checks, or unsigned casts on
  `min_blockscout_block`. Every access is `Set(...)` on write, `update_column`
  in the upsert, or an equality comparison on read.
- `insert_data_many` is `ON CONFLICT (chart_id, date) DO UPDATE`. **Widening is
  self-correcting; narrowing is not.** This is why `force_full` alone is a valid
  fix for backfill but not for a narrowed filter.
- A chart that has written **zero** rows is not anchored: `recorded_min_indexer_block`
  returns `None`, so every subsequent cycle re-derives the floor from `get_min_date`.
  Anchoring begins with the first written row.
- Indexer-side: catch-up walks **down** from `latest - 1` to the configured start
  block, and `mark_catchup_complete` then lowers `catchup_max_cursor` to
  `start_block - 1`.

**[I]** Derived:

- The anchor is a function of *when stats first ran*, not of what the data is.
  Two stats deployments started a week apart against the same indexer can hold
  permanently different histories, with no field distinguishing them.
- Because catch-up descends, `MIN(init_timestamp)` is monotonically
  non-increasing during catch-up and constant afterwards. A forward-scanning
  indexer would break this and would leave the *middle* of history permanently
  empty while the minimum never moved — that hazard does not apply here.

## Failure Modes / Observability

**[I]** Per chart kind, for "indexer backfills a range older than the anchor":

| kind | interchain families | outcome |
|---|---|---|
| counter (`DirectPointLocalDbChartSource` + `PassPoint`) | 7 `total_interchain_*` | ✅ correct — recounted in full every cycle, no time bound in the statement |
| base daily line (`DirectVecLocalDbChartSource`) | 6 `new_*` | ❌ backfilled days never queried again |
| weekly / monthly / yearly (`SumLowerResolution`) | same 6 | ❌ inherits the gap — they aggregate the **local** daily/monthly chart, not the indexer, so they stay *consistent with* the wrong daily series |
| cumulative (`DailyCumulativeLocalDbChartSource`) | 2 `messages_growth_*` | ❌❌ the missing prefix offsets every later point |

**[I]** Detection: the only automatic cross-check available is
counter-vs-cumulative divergence (e.g. `totalInterchainMessagesSent` vs the last
point of `messagesGrowthSentInterchain`). Nothing in stats performs it or logs it.
`last_accurate_point` does log `"running partial update"` with the point, so the
frozen floor is visible in logs if someone looks.

**[F]** Recovery paths that already exist:

1. `STATS__FORCE_UPDATE_ON_START=true` + restart → `force_full` → floor re-derived
   from `get_min_date` against the now-backfilled DB. Note the default is
   `Some(false)`, i.e. a plain restart runs an initial update but **not** a full one
   (`None` means no initial update at all).
2. `BatchUpdateCharts` RPC / REST with `from` (`read_service.rs:925`, authorized) →
   `set_next_update_from` rewinds `charts.last_updated_at` → partial recompute from
   that date.

**[F]** Already documented in `gotchas.md`: "In `Interchain` Mode,
`chart_data.min_blockscout_block` Is Not a Block Number" and "`insert_data_many` Is
an Upsert With No Delete". This note is the missing third piece — that repurposing
the column also **removed the backfill detector**, which neither gotcha states.

## How "Wait For The Indexer" Is Built (and why interchain has none)

**[F]** Three axes, two transports:

| axis | source | check |
|---|---|---|
| `blockscout` | Blockscout API indexing-status | `blocks_ratio` / `internal_transactions_ratio` thresholds |
| `user_ops` | Blockscout API AA indexer status | `finished_past_indexing` |
| `zetachain_cctx` | **the CCTX indexer DB directly** — `watermark` row with `kind=Historical` | `indexed_until < today_start` |

**[F]** `IndexingStatusAggregator::run` polls in a loop and publishes to a
`tokio::watch`; `IndexingStatusListener::wait_until_status_at_least` is the other end.

**[F]** Two structural properties that constrain any reuse:

1. **The aggregator does not exist in interchain mode.** It is created only when
   `init_blockscout_api_client` returns `Some` — i.e. only when a *Blockscout API
   URL* is configured. `apply_interchain_mode_settings` sets
   `blockscout_api_url = None` and `ignore_blockscout_api_absence = true`, so
   `status_listener` is `None` and `wait_for_start_condition` is a no-op. The
   coupling is to the Blockscout HTTP client, even though the zetachain axis uses
   no HTTP at all.
2. **The existing gate blocks, and it is one-shot.** `wait_for_start_condition`
   runs once, before `run_initial_update`; `run_cron` then loops forever with a
   hardcoded `force_full = false` and never re-reads the status. So the mechanism
   available today expresses *"do not update until X"*, never *"update differently
   while not X"*, and it cannot react to a status change after startup.

**[I]** Therefore adding a fourth `InterchainIndexingStatus` axis in the obvious way
would mean **serving no interchain chart at all until the bridge finishes catching
up** — potentially days on a from-scratch index. "Compute fully while catching up"
is a different mechanism that does not exist in the codebase yet.

**[F]** Latent bug worth knowing before touching this: `IndexingStatusAggregator::run`
returns early when `!blockscout_checks_enabled() && !user_ops_checks_enabled()` —
`zetachain_checks_enabled()` is not consulted. `init` seeds the zetachain axis to
`CatchingUp` when its check is enabled. So a `Zetachain`-mode config with both
blockscout ratios and the user-ops check disabled leaves the axis at `CatchingUp`
forever and every zetachain group blocks indefinitely. It does not fire on defaults
(zetachain mode keeps the Blockscout API), but a new interchain axis added the same
way would inherit the same shape.

## The Indexer's Progress Handle, and What Is Derivable From Its DB

**[F]** `GetIndexingProgress` — gRPC, and REST as
`GET /api/v1/status/indexing?bridge_id=&chain_id=` — returns one
`ChainIndexingProgress` per `(bridge_id, chain_id)`. The two fields that matter here:

- `catchup_complete: bool` — **[F]** since `2d2ce3e6` (#1733). Field 7, renamed
  from `catchup_scan_complete`, and widened:
  `CatchupProgress::catchup_complete(failed_blocks)` =
  `progress_percent == 100.0 && failed_blocks == 0`. The old flag mirrored
  `scan_complete` ("the two catch-up cursors met") and so could report a completed
  catch-up for a pair with unresolved holes. Deliberately phrased over the
  *percentage* rather than over `scan_complete`, for two reasons given in its doc
  comment: a client can reproduce it exactly from two fields the same response
  carries, and it reads `realtime_cursor < start_block` (`total = 0`, where
  `scan_complete` is vacuously `true`) as "not done".
- `failed_blocks: u64` — blocks inside unresolved holes, still reported
  separately. `progress.rs`'s module doc is explicit that
  `catchup_progress_percent` is the *scanned* share and **not** a completeness
  measure; `100%` with `failed_blocks > 0` is a normal reading of *that* field,
  and is exactly the case `catchup_complete` now folds in.
- `scan_complete` survives as the indexer's internal scan-level notion and is no
  longer exposed on the API.

**[F]** The endpoint's enumeration is **config-driven**: `collect_indexing_progress`
joins `targets` (from the in-memory bridges config, via `enumerate_indexing_targets`)
against `indexer_checkpoints` and `indexer_failure_totals` **in Rust**, with the
stated reason that a SQL join *cannot* produce the config-only rows — a pair whose
indexer failed to start, or that has no checkpoint yet — which is where the
endpoint's main value is.

**[F]** `start_block` lives only in `IndexingTarget` (config); the
`indexer_checkpoints` table has no such column.

> **[CORRECTION] The predicate is DB-derivable anyway — `catchup_min_cursor` *is*
> the configured floor.** `interchain-indexer-server/src/indexers.rs:627` states
> it outright: *"the stored value **is** the previous run's configured floor, and
> comparing configuration against it directly is exact rather than a proxy."*
> `catchup_min_cursor` never advances with progress; exactly two writers move it —
> `seed_catchup_floor` (raise-only, called from
> `interchain-indexer-logic/src/indexer/evm/log_stream_builder.rs:56` at every
> stream-builder startup) and `lower_catchup_floor` (lower-only, from
> `indexers.rs:695`). And `persist_catchup_complete`
> (`interchain-indexer-logic/src/log_stream.rs:335-368`) calls
> `mark_catchup_complete(..., genesis_block.saturating_sub(1), ...)`, which writes
> `catchup_max_cursor = LEAST(existing, start_block - 1)`.
>
> **[I]** So with `S = M = catchup_min_cursor`, a reader outside the indexer can
> compute all of it:
> `blocks_remaining = if X < M {0} else {X - M + 1}`,
> `total = if R < M {0} else {R - M + 1}`,
> `complete = blocks_remaining == 0 && total > 0 && no indexer_failures rows`.
> Two residual blind spots, **both failing toward "not complete"**: an un-seeded
> floor (`M = 0, X = S - 1`; the seed write is a `warn`, not a startup blocker),
> and — the one genuine gap — a configured pair with no checkpoint row, which the
> DB cannot see at all. That last one is why the *API* was chosen as primary
> rather than the DB. `scan_complete` is computed as
`catchup_max_cursor < max(catchup_min_cursor, start_block)`, and the guarded form is
load-bearing: `mark_catchup_complete` writes `catchup_max_cursor = start_block - 1`,
so an un-healed row is `(min = 0, max = S - 1)` and the naive `max < min` predicate
reports a finished catch-up as unfinished.

**[F]** `bridge_contracts` is not a substitute for the config enumeration —
the indexer's own gotcha calls it a diagnostic proxy only: under-populated during
startup backfill, and permanently over-populated after a chain is dropped from a
bridge.

**[I]** So, unlike the zetachain watermark (one row, complete on its own), what
is **not** derivable from the indexer DB is the *pair enumeration*, not the
predicate (see the correction above). `indexer_checkpoints` and `indexer_failures`
are readable and sufficient to decide completeness for any pair that has a row;
the set of pairs that *should* exist is config-driven and invisible. The
undetectable case — a configured pair with no checkpoint row — fails in the
dangerous direction: a DB-only reader sees "nothing outstanding" and concludes
"synced".

**[F]** One trap for anyone who ever derives the sync verdict from the indexer DB
instead of the API (the shipped design does not — see Status above, but the fact
is durable and the DB *is* capable of it): `enumerate_indexing_targets`
**skips disabled bridges**, and `upsert_bridges` sets `enabled = false` on every
row before re-upserting the configured ones — so `bridges.enabled = true` is
exactly "in config and enabled", i.e. exactly the API's bridge set. A DB-derived
*sync* pair set must therefore filter on `bridges.enabled`. That is the **opposite**
of the rule `resolve_only_indexed_by_bridge` follows for the *horizon*, which must
never filter on it. Same table, two opposite rules, two different purposes.

**[I]** Tension worth recording: stats **already** derives the observability horizon
from `bridges ⋈ bridge_contracts` (see `interchain-mode-and-filtering.md` §6.6),
i.e. it already depends on the proxy the indexer considers unauthoritative.

## Scoping a Sync Signal Across Many Bridges and Chains

The indexer indexes many bridges × many chains, and one chain can be indexed by
several bridges. Stats' filter is optional and has six dimensions
(`home_chain_id`, `counterparty_chain_ids`, `src_chain_ids`, `dst_chain_ids`,
`bridge_ids`, `include_unindexed_chains`). "Which chain do we orient on?" therefore
looks like a required choice. It is not — the answer differs by signal component,
and for the cheapest component the choice does not exist at all.

### Level 1 — `MIN(init_timestamp)` needs no selection

**[F]** `get_min_date_interchain` computes the minimum *through the filter*
(`filter.messages_query()` and `filter.transfers_joined_query()`). It is a scalar
aggregate over the **result set**, not over a chosen partition.

**[I]** So it is correctly scoped by construction for every configuration: many
bridges, a chain shared between bridges (both of its `(bridge, chain)`
incarnations contribute rows), or no filter at all (the default is
`InterchainFilter::unfiltered()`). No per-chain decision is involved.

### Level 2 — the cursor/failure component scopes by **bridges**, not chains

**[I]** `indexer_checkpoints` and `indexer_failures` are keyed by
`(bridge_id, chain_id)`, so anything built on them does need a set of pairs.
`bridge_ids` is the only filter dimension that maps 1:1 onto that partitioning.
The five chain-id dimensions do **not** reduce the relevant pair set:

- **[F]** `init_timestamp` normally comes from the **source-side** block timestamp
  (`SourceData::from_send()`), and degrades to the *destination-side* block
  timestamp when the source chain is unconfigured
  (`avalanche-bridge-filtering.md:171`). Which day a message lands in can
  therefore depend on the opposite side's indexing.
- **[I]** `new_messages_received_interchain` filters on `dst_chain_id = home`, but
  the row originates from events on the *source* chain. The completeness of the
  received direction is governed by the counterparties' catch-up, not the home
  chain's.
- **[F]** The counterparty set is open unless `counterparty_chain_ids` /
  `dst_chain_ids` is explicitly configured, and both are `Option`.

**[I]** Consequently a per-chain narrowing risks a false "synced", which is the
dangerous direction. The safe reading is a conjunction over **all**
`(bridge, chain)` pairs of the bridges the filter admits; `bridge_ids = None`
means all bridges. Stats already resolves exactly that pair set in
`resolve_only_indexed_by_bridge` (with its documented `bridge_contracts` caveat).

### Level 3 — a fingerprint is a change-detector, not a completeness predicate

**[I]** A hash never has to decide whether a given chain "matters". It only has to
guarantee: *if anything that could add history behind the anchor moved, the value
moves.* So the whole scoped state can be hashed — e.g. sorted
`(bridge_id, chain_id, catchup_min_cursor, catchup_max_cursor)` rows for the
filter's bridges — with no selection logic at all.

**[I]** The cost asymmetry points the same way: an extra rebuild costs queries, a
missed one costs silently wrong data. Hash more, select less. A *choice* becomes
unavoidable only for a boolean `is_fully_synced`, because a conjunction needs a
set — and there the answer is Level 2's conservative reading.

## Design Options (original analysis — see the Status banner above for the resolution)

### Option A — force a full recompute while the indexer is not fully synced

**[I]** Wire point: `UpdateService::update` already performs a per-cycle interchain
probe (`resolve_only_indexed_by_bridge`) with a "on error skip the group rather than
compute something wrong" policy; a sync probe fits the same shape, and `force_full`
would come from it instead of `run_cron`'s hardcoded `false`.

- Correct by construction: `force_full` → `last_accurate_point = None` →
  `get_min_date_interchain` re-read each cycle; the upsert covers the widening.
- Cost: `Batch30Days` means a full rebuild is `days/30` batches per daily chart,
  plus the lower resolutions, **every cycle** (2h for the interchain groups) for the
  whole catch-up. **[Q]** Needs a real measurement against a multi-year bridge.
- Requires solving the aggregator's Blockscout-client coupling, or bypassing it.

### Option B — put a sync signal in the fingerprint slot: **has a real defect**

**[I]** Superficially attractive, because the clear-and-rebuild path already exists,
is interchain-gated, and is tested. It does not work as-is, for a reason that is not
about scoping:

| signal | reaction it needs | why |
|---|---|---|
| filter changed | **clear + recompute** | narrowing leaves stale rows (upsert has no delete) |
| history extended backwards | **recompute only** | widening is self-correcting |

**[I]** With one scalar carrying both, the two are indistinguishable and **every
catch-up cycle would also wipe the chart**. It also contradicts
`filter_fingerprint`'s stated design: the horizon's contents are excluded precisely
because DB-derived, self-growing values would rebuild all 39 charts on every
upstream bridge addition. Cursors are that, only worse — `realtime_cursor` advances
continuously and would produce a permanent rebuild loop, so it must be excluded
from any such scheme.

> **[CORRECTION] Options B′ and C′ below are unsound, and the reason postdates
> this note.** `05bd53f1` (*"decouple per-chain indexing within a bridge"*) means
> chains inside one bridge descend independently: chain A can be caught up to 2022
> while chain B has only just started walking down from the tip, filling days
> **interior** to the already-established global minimum. A falling
> `MIN(init_timestamp)` cannot see an interior fill — which is stated below as a
> minor miss ("interior holes healed later") but is in fact the *normal* case in
> any multi-chain configuration, not an edge case. Any design that detects
> backfill purely by watching the floor is therefore ruled out here. The
> §Invariants claim that descending catch-up makes the minimum "a well-matched
> signal rather than a lucky heuristic" holds **per pair** and does not lift to the
> aggregate.

### Option B′ — compare the indexer floor against the stored floor, rewind

**[I]** Compare `MIN(init_timestamp)` through the filter against
`MIN(chart_data.date)` for that chart; if the indexer floor is lower, rewind
`charts.last_updated_at` to it via the existing `set_next_update_from`.

- Correctly scoped by construction (Level 1) — no chain selection.
- Per-chart rather than per-deployment; deletes nothing; leaves the fingerprint
  unambiguous as a filter-change detector.
- Needs no API, no fourth axis, no decoupling of the aggregator.
- A "minimum stored date" read helper does not exist yet; it is a one-query
  sibling of `recorded_min_indexer_block`.
- Cost: while the floor keeps falling, this rebuilds every cycle — the same
  thrash as Option A, reached differently.
- Misses interior holes healed later (they do not move the minimum) and a
  configured pair that never started.

### Option C — split the word: fingerprint bits + a "backfill in progress" bit

**[F]** The bit is genuinely free: the column is a nullable `bigint` → `Option<i64>`,
`filter_fingerprint` masks to 63 bits so the sign bit is always zero, and no code
path does arithmetic, sign checks, or unsigned casts on the value.

**[I]** Two comparison widths give the two different reactions Option B could not
separate:

| comparison | reads | reaction |
|---|---|---|
| the interchain gate in `update_itself_inner` | fingerprint bits only | `clear_chart_data_and_updated_at` + recompute |
| `last_accurate_point` | the whole word | `None` → recompute, no delete |

Both call sites already exist; only the gate's predicate changes. If filter and
flag change in the same cycle the fingerprint bits differ and the clear still
fires, which is correct.

**[I]** The decisive property is the cost profile, which comes from the flag being
a *boolean* — it changes at most twice:

- cycle 1 of catch-up: nothing recorded → full compute from the current floor,
  rows stamped with flag = 1;
- cycles 2..N: recorded flag = 1 equals observed flag = 1 → **equal** → ordinary
  incremental updates, no rebuilds;
- catch-up completes: 1 → 0 → **one** full recompute from the true floor.

| option | rebuilds during catch-up |
|---|---|
| A | one per cycle (2h) for the whole catch-up |
| B′ | one per cycle while the floor keeps falling |
| **C** | **one, at completion** |

**[I]** Implementation notes:

- Prefer **bit 62** over the sign bit: mask the fingerprint to
  `0x3fff_ffff_ffff_ffff` and keep the stored value non-negative. The existing
  `0 | i64::MAX => 1` remap keeps working, collision probability moves from 2⁻⁶³ to
  2⁻⁶² (irrelevant), and the column stays readable as a number in three of four
  modes. Negative values there are a needless trap for anyone inspecting it in SQL.
- **Restart-safe by construction.** `recorded_min_indexer_block` reads the stamp on
  the *newest* row while `BatchUpdate` walks upward from the floor, so the newest
  row receives the new value last. An interrupted rebuild leaves the newest row
  still carrying the old stamp, so the next cycle sees a mismatch and restarts from
  `get_min_date`. The rebuild does not "count as done" until it finishes. This
  works precisely because the flag lives in the row stamp rather than in a separate
  table.
- Changing the encoding requires bumping `VERSION_TAG`, which changes every
  fingerprint — one global rebuild on deploy, and in interchain mode that mismatch
  also *clears* all 39 charts. The version tag exists for exactly this, but it must
  be a conscious one-off.
- The flag is transport + reaction, **not** a source. It still needs Level 2's
  conjunction, and it should include `failed_blocks`, not just
  `catchup_scan_complete` — the ledger draining then produces its own 1 → 0 and its
  own rebuild.
- Debounce: a flaky source turns every flip into a full rebuild. The established
  policy fits — `resolve_only_indexed_by_bridge` failure *skips the group* rather
  than computing without the horizon; likewise, "could not determine sync state"
  should skip the cycle rather than guess in either direction. A 0 → 1 → 0 sequence
  caused by adding a bridge is correct behaviour, not flapping.

### Option C′ — same bits, a coarse floor instead of a flag

**[I]** Store `MIN(init_timestamp)` bucketed to a month or quarter in those bits
(~10 bits) instead of one boolean. Rebuilds are then bounded by the number of
bucket boundaries the floor crosses — more than C's single rebuild, far fewer than
A/B′ — and correctness improves progressively instead of only at the end. It needs
**no API at all**, because the minimum is already filter-scoped (Level 1). Costs:
fewer fingerprint bits, and it cannot express interior holes, which the boolean
flag accommodates naturally via `failed_blocks`.

### Complementarity of the underlying signals

| signal | catches | misses |
|---|---|---|
| `MIN(init_timestamp)` falling | history extending **backwards** — the scenario this note is about | interior holes healed later (they do not move the minimum) |
| `indexer_failures` / catch-up cursors | interior holes closing | a pair that never started |
| `catchup_complete` (API only — the *enumeration* is what is API-only, not the predicate; see the correction above) | a configured pair that never started | — |

**[Q]** Open design questions: is a heuristic acceptable, or is the API guarantee
required? Is "wrong during catch-up, right at completion" (C) acceptable, or is
progressive correctness (C′) worth its extra rebuilds? Should this be
interchain-only, or is `MultichainAggregator` exposed to a comparable hole?

## Edge Cases / Gotchas

- **[F]** Zero rows written ⇒ not anchored. A first update against a truly empty
  indexer DB writes nothing (`get_min_date_interchain` returns `now`), so the floor
  is re-derived on the next cycle. The trap only closes once the first row lands.
- **[F]** `update_metadata` is called after **every** batch, not once at the end. An
  interrupted rebuild resumes from where it stopped and the chart serves a partially
  rebuilt series meanwhile — by design.
- **[F]** Lower resolutions have coarser trailing-approximation: with
  `approximate_trailing_points = 1`, a yearly chart treats the *previous year* as
  the last accurate point and recomputes the whole current year each cycle. This
  does **not** make it self-heal from the indexer — it re-derives from the (already
  incomplete) stored daily/monthly chart, so resolutions stay mutually consistent
  and jointly wrong.
- **[F]** `force_full` skips `last_accurate_point`'s read but not the
  upsert-has-no-delete problem. It fixes backfill (widening) and does **not** fix a
  narrowed filter.
- **[I]** `STATS__FORCE_UPDATE_ON_START` has three states, not two: `None` = no
  initial update, `Some(false)` = normal initial update, `Some(true)` = full.
- **[F]** The sign bit of `min_blockscout_block` is unused today only because
  `filter_fingerprint` masks to 63 bits *and* remaps `0`/`i64::MAX`. Any change to
  that masking has to preserve whatever the bit layout comes to mean.

## Change Triggers

Revisit this note when:

- the selected design lands (see the Status block): `run_cron`'s `force_full` stops
  being effectively hardcoded, `catchup_complete` changes meaning on the indexer
  side, or stats stops reading `GET /api/v1/status/indexing`;

- `get_min_block_interchain` stops returning the filter fingerprint, or the
  fingerprint's bit layout changes (including a `VERSION_TAG` bump), or a separate
  column/table is added for indexer state;
- the interchain clear gate in `update_itself_inner` stops comparing the full
  recorded value;
- `run_cron`'s hardcoded `force_full = false` becomes conditional;
- `IndexingStatus` gains a fourth axis, or `IndexingStatusAggregator` is decoupled
  from `blockscout_client::Configuration`;
- `wait_for_start_condition` stops being one-shot;
- the indexer's catch-up stops descending from the tip (§Invariants depends on it),
  or `enumerate_indexing_targets` stops being config-driven, or
  `indexer_checkpoints` gains a `start_block` column (either of the last two would
  make the completeness signal DB-derivable);
- `approximate_trailing_points` or the `BatchUpdate` range derivation changes;
- `InterchainFilterSettings` gains a dimension that maps onto the indexer's
  `(bridge_id, chain_id)` partitioning (that would change Level 2's scoping);
- interchain charts start reading the indexer's projected `stats_*` tables instead
  of re-aggregating the canonical ones.

## Open Questions

1. **Answered.** Catch-up descends: without a checkpoint both implementations start
   realtime at the latest block `N` and catch-up at `N - 1`, down to the configured
   start block (`indexing-gaps-retries-and-checkpoint-safety.md` §1). So
   `MIN(init_timestamp)` falls monotonically during catch-up, and the
   forward-scanning hazard — a permanently empty *middle* of history that the
   minimum cannot see — does not arise. Kept rather than deleted because it is what
   makes Level 1 and Option C′ viable.
2. **[Q]** What is the real per-cycle cost of Option A on a multi-year bridge
   (batch count, indexer query load)? Needs a measurement.
3. **[Q]** Should the counter-vs-cumulative divergence be turned into an explicit
   consistency check or metric, independently of whichever option is chosen?
4. **[Q]** Is `MultichainAggregator`'s `SUM(min_block_number)` detector robust
   against a compensating change (one chain's minimum falling while another chain is
   removed)? The sum is a hash-like proxy, not an order relation.
5. **[Q]** For Option C, where should the flag be computed — a per-cycle probe in
   `UpdateService::update` alongside the horizon resolution, or a revived
   aggregator publishing to a `watch` channel that `update` reads with `borrow()`?
   The latter reuses more, but requires the Blockscout-client decoupling.

## Related Notes

- `.memory-bank/research/interchain-mode-and-filtering.md` — the filter itself, the
  chart inventory, and the fingerprint's *filter* role. **Discrepancy found while
  writing this note:** its §10 bullet "No index supports `src_chain_id` /
  `dst_chain_id` predicates" contradicts its own §4, which correctly documents
  `crosschain_messages_src_chain_ts_idx` and `crosschain_messages_dst_chain_ts_idx`.
  The indexes do exist (`m20251030_000001_initial_up.sql:111-117`); the §10 bullet
  should be corrected.
- `.memory-bank/gotchas.md` — "In `Interchain` Mode, `chart_data.min_blockscout_block`
  Is Not a Block Number", "`insert_data_many` Is an Upsert With No Delete", "Trailing
  Line-Chart Points Are Marked Approximate By Design".
- `interchain-indexer/.memory-bank/research/indexing-gaps-retries-and-checkpoint-safety.md`
  — scan boundaries, catch-up completion, and the failure ledger.
- `interchain-indexer/.memory-bank/research/message-lifecycle.md` — which side of a
  message creates the row and when it is flushed.
- `interchain-indexer/.memory-bank/gotchas.md` — "`bridge_contracts` Is Only A
  Diagnostic Proxy For Runtime Membership".
