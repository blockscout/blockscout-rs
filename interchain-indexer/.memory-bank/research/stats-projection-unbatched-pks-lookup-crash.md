# Stats Projection: Unbatched `pks` Lookup Crashes Maintenance

## Scope

This note documents a production incident (2026-09-16) where buffer
maintenance failed twice within minutes, with two different-looking Postgres
errors, and traces both to the same unbatched query in
`project_messages_batch()`. It covers the query itself, why it produces two
distinct failure modes depending on cohort size, why it was missed despite an
adjacent, correctly-chunked sibling query, and the intended fix. It does not
cover the rest of stats projection's eligibility/aggregation logic — see
`stats-projection.md` for that.

## Short Answer

`project_messages_batch()` (`interchain-indexer-logic/src/stats/projection.rs`,
initial `find()` around lines 95-127) reloads canonical message rows with a
composite-key `IN` filter —
`Expr::tuple([Id, BridgeId]).in_tuples(pks.iter().copied())` — built from the
**entire, unchunked** `pks` slice for one maintenance cycle's flushed cohort.
Unlike every other bulk query in the same transaction, this one does not go
through the repo's shared `batched_upsert`/`run_in_batches` chunking
(`interchain-indexer-logic/src/bulk.rs`), so its size is bounded only by how
large the flushed cohort happens to be. During catch-up/backfill bursts that
cohort has no cap, and the query fails once it crosses either of two
independent Postgres/sqlx ceilings:

- **`stack depth limit exceeded`** — Postgres expands a composite/tuple `IN`
  list into an `OR`-tree of row-equality comparisons (not a flat
  `ScalarArrayOpExpr` the way a single-column `IN`/`is_in()` would be), and a
  cohort of a few thousand tuples can overflow the parser/planner's recursion
  stack (`max_stack_depth`, default 2MB) well before any bind-parameter limit
  is reached.
- **`too many arguments for query: N`** — if the cohort grows large enough
  (observed: 81264, i.e. ~40632 tuples × 2 binds/tuple), it exceeds sqlx's
  hard bind-parameter ceiling (`u16::MAX = 65535`, the same
  `PG_BIND_PARAM_LIMIT` constant the rest of the codebase respects), and the
  query is rejected outright before ever reaching Postgres's stack.

Both are the same underlying bug (one unbounded query) surfacing differently
depending on exactly how large that cycle's cohort was.

## Why This Matters

This query runs inside the shared buffer-maintenance transaction
(`commit_maintenance`, `message_buffer/maintenance.rs`), which also carries
canonical message/transfer writes and cursor persistence for the cycle. Per
`.memory-bank/rules/error-handling.md` ("Expected Skips Inside a Shared
Transaction"), any `DbErr` escaping that transaction rolls back **everything**
in it, not just stats projection. Because the cohort that triggered the crash
is re-read from the same unadvanced cursor on the next cycle, this is a
**poison cycle**: the service will keep re-hitting the same oversized cohort
and re-failing maintenance indefinitely, stalling all buffer flushing (not
just stats), until the underlying query is fixed or the cohort shrinks on its
own (unlikely, since the hot buffer has no capacity cap — see
`message-lifecycle.md`).

## Source-of-Truth Files

- `interchain-indexer-logic/src/stats/projection.rs` — `project_messages_batch()`,
  the vulnerable `find()` call
- `interchain-indexer-logic/src/stats/service.rs` — `apply_stats_for_flushed_batch()`
  builds `msg_pks` (lines ~109-120, no size cap) and calls
  `project_messages_batch`; the **correctly chunked** sibling transfer lookup
  (lines ~128-148) with its own explanatory comment
- `interchain-indexer-logic/src/message_buffer/maintenance.rs` —
  `commit_maintenance` (~lines 287-346), the shared transaction; calls into
  stats service at ~line 316
- `interchain-indexer-logic/src/message_buffer/buffer.rs` — logs
  `buffer maintenance failed` (~line 163) when `commit_maintenance` returns
  `Err`
- `interchain-indexer-logic/src/bulk.rs` — the shared, correctly-used batching
  utility this query bypasses: `PG_BIND_PARAM_LIMIT = u16::MAX` (line 16),
  `batch_size_for_width()` (lines 20-36), `run_in_batches()` (lines 39-53),
  `batched_upsert()` (lines 56-75)
- `interchain-indexer-logic/src/message_buffer/persistence.rs` — every other
  bulk write in the same maintenance transaction (`crosschain_messages`,
  `crosschain_transfers`, `pending_messages`, `amb_messages_confirmations`,
  `amb_message_anomalies`) goes through `batched_upsert` and is unaffected

## Key Types / Tables / Contracts

- `PG_BIND_PARAM_LIMIT` (`bulk.rs:16`) — `u16::MAX = 65535`, the sqlx/Postgres
  bind-parameter ceiling the codebase is built around
- `batch_size_for_width()` — `PG_BIND_PARAM_LIMIT / width`, where `width` is
  an entity's total column count; used everywhere **except** the query this
  note documents
- `Expr::tuple([...]).in_tuples(pks)` — SeaORM's composite/row-valued `IN`
  builder; binds 2 params per tuple here (`Id`, `BridgeId`), but the risk is
  not bind-count alone — Postgres's own row-IN-to-OR-tree expansion adds a
  second, lower, unbatched ceiling
- `msg_pks: Vec<(i64, i32)>` (`stats/service.rs`) — the full, unchunked
  flushed-cohort primary-key list passed into `project_messages_batch`

## Step-by-Step Flow

1. Buffer maintenance flushes a cycle's consolidatable cohort (see
   `message-lifecycle.md`); `stats/service.rs` builds `msg_pks` from **every**
   entry in `flushed`, with no cap on cohort size.
2. `apply_stats_for_flushed_batch` calls `project_messages_batch(tx, &msg_pks, ...)`.
3. `project_messages_batch`'s first statement re-reads the canonical rows via
   `crosschain_messages::Entity::find()` joined to `Bridges`, filtered by
   `Expr::tuple([Id, BridgeId]).in_tuples(pks.iter().copied())` — the entire
   `pks` slice in one query, no chunking.
4. For a moderate-but-unbounded cohort, Postgres's parser/planner expands the
   composite `IN` into a deep `OR`-tree and overflows `max_stack_depth` →
   `stack depth limit exceeded`.
5. For a much larger cohort (a heavier catch-up burst), the query never even
   reaches Postgres: sqlx refuses to send it once total bind params exceed
   `65535` → `too many arguments for query: N`.
6. Either error is a `DbErr` that propagates out of `commit_maintenance`,
   rolling back the whole maintenance transaction (canonical writes, cursor
   advancement, stats projection all together) for that cycle.
7. Because the cursor never advanced, the next maintenance cycle re-reads the
   same backlog and is likely to reconstruct a similarly oversized `pks`,
   repeating the failure.

## Invariants

**Corrected 2026-09-16, after the fix landed (`stats-projection-unbatched-cohort-queries`
task) — the three bullets originally here endorsed a fix that does not work.
See `.memory-bank/rules/database.md` §Batching for the corrected rule.**

- ~~every other bulk query in the same maintenance transaction is chunked
  through `bulk.rs`'s shared `batch_size_for_width`/`run_in_batches`
  machinery and stays under `PG_BIND_PARAM_LIMIT` by construction~~ — **false**.
  That machinery bounds *bind count*, not *planner stack depth*, and for a
  row-valued `IN` those are two different ceilings. Several "correctly
  chunked" sites (`stats/service.rs:134`'s sibling lookup and four
  `load_*_map`/`load_stats_asset_edges_for_keys` helpers in `projection.rs`)
  were latent instances of the same bug at `PG_BIND_PARAM_LIMIT / width`
  tuple counts, and `projection.rs`'s own mark-update
  (formerly `run_in_batches(&mark, 2, …)`) and its four flat-`is_in` update
  sites were not even bind-safe by construction.
- ~~`project_messages_batch`'s initial `find()` is the **one** exception: it
  builds its own composite-`IN` query directly, outside that shared
  utility~~ — **false**. There were **three** unchunked cohort-sized queries:
  `projection.rs`'s message load (formerly `:112`), the transfer load
  (formerly `:1594`), and the transfer deferral load (formerly `:1614`).
- ~~the sibling transfer lookup five lines later in `stats/service.rs`
  (~lines 134-148) already hand-rolls the correct chunking pattern
  (`batch_size = PG_BIND_PARAM_LIMIT / 2`) for the analogous problem~~ — it is
  **bind-safe but not stack-safe**, and it was the template that propagated
  the latent bug to every other row-valued `IN` site touched by this fix. The
  actual fix is `bulk::ROW_IN_KEY_CHUNK` (see `bulk.rs`), a fixed,
  not-bind-derived constant, applied to every row-valued `IN` load in
  `stats/projection.rs` and `stats/service.rs`.
- **Corrected bind arithmetic.** This note originally modeled the query #1
  cost as roughly `2·tuples`. The real cost is `2·T + c₁`, where `c₁` is
  **config-sized** — `StatsProcessed.eq(0i16)` plus
  `finalized_message_stats_condition` (`projection.rs:70-78`) plus
  `chain_unindexed_condition` (`stats/indexed_chains.rs:318-363`, which is
  `Σ_b (1 + |chains_b|)` over configured bridges) — and grows with every
  bridge or chain added. This is precisely why `PG_BIND_PARAM_LIMIT / 2 =
  32_767` tuples ⇒ `65_534` binds leaves room for exactly one more and still
  overflows; it is not a safety margin.

## Failure Modes / Observability

- `stack depth limit exceeded` — `Query Error: error returned from database:
  stack depth limit exceeded`, logged as `buffer maintenance failed
  error=maintenance transaction failed` by
  `interchain-indexer-logic::message_buffer::buffer` (observed
  2026-09-16T09:27:07Z)
- `too many arguments for query: N` — `Query Error: encountered unexpected or
  invalid data: PgConnection::run(): too many arguments for query: 81264
  (sqlx_postgres::connection::executor:220)`, same log site (observed
  2026-09-16T09:35:26Z, ~8 minutes after the first failure — consistent with
  a growing/ongoing catch-up burst producing progressively larger cohorts
  across consecutive maintenance cycles)
- both errors share the same log line and root cause; either should be
  treated as this bug, not as independent incidents
- **not** the cause: a separate, expected `WARN sqlx::query: slow statement`
  for the batched `crosschain_messages` upsert (`bulk.rs`/`persistence.rs`)
  was initially suspected but ruled out — that upsert is a flat multi-row
  `INSERT ... VALUES ... ON CONFLICT`, correctly chunked to exactly
  `65535 / 17 = 3855` rows by `batch_size_for_width`, and carries neither the
  stack-depth nor the bind-count risk (no composite `IN`, no OR-tree
  expansion); its slowness is an unrelated, orthogonal performance signal

## Edge Cases / Gotchas

- this is **not** the same category of problem as the documented
  "PostgreSQL bind limit" gotcha (`gotchas.md`) — that gotcha is about the
  65535 *bind-parameter count* ceiling, which `bulk.rs`'s batching already
  handles everywhere else. The stack-depth failure mode is a **different,
  lower, and less predictable** ceiling specific to composite/row-valued
  `IN` (`(col_a, col_b) IN ((...),(...))`) expanding into an `OR`-tree —
  ordinary single-column `IN`/`is_in()` does not have this problem, since
  Postgres optimizes that into a flat `ScalarArrayOpExpr`
- because the stack-depth ceiling is data/shape-dependent (planner stack
  usage, not a fixed row count), there is no chunk size for this query that
  is *provably* immune to it. **`PG_BIND_PARAM_LIMIT / 2` does *not* keep the
  `OR`-tree small enough** — it leaves a row-valued `IN` at up to 32 767
  tuples, the same order as the cohorts running when the planner stack
  overflowed in production, so that bound is bind-safe but not stack-safe.
  The shipped fix instead uses a fixed, margin-justified-not-derived
  constant, `bulk::ROW_IN_KEY_CHUNK = 2_000`, which uses ~6-15% of the bind
  budget. **The stack-depth threshold was never measured.** It is bounded
  only from *above*, at < ~40 600 tuples, and even that is an inference: the
  09:35 bind-count failure carried ~40 620 tuples, and the 09:27 stack
  failure came from an earlier, therefore smaller, cohort. So 2 000 sits an
  order of magnitude below an *upper bound*, not below an observed limit.
  This is a margin argument, not a proof — see `bulk::ROW_IN_KEY_CHUNK`'s doc
  comment, and do not raise the constant without measuring.
- the root cause is unbounded **input** (the hot message buffer has no
  capacity cap, only TTL/interval — see `message-lifecycle.md`), not
  unbounded **output**; fixing only the query's chunking treats the symptom,
  but the buffer's lack of a capacity cap is why any single flushed cohort
  can grow this large in the first place
- raising Postgres's `max_stack_depth` was considered and rejected as a
  workaround: it is bounded by the OS thread stack size
  (`ulimit -s`) and unsafe to raise past what the kernel actually enforces
  (risks a backend crash/segfault instead of a graceful error); it also does
  not address the separate, harder `65535` bind-parameter ceiling, and does
  nothing for the structural unbounded-cohort problem — it only moves the
  threshold at which the same class of failure recurs

## Change Triggers

Update this note when:

- ~~`project_messages_batch`'s initial `find()` is fixed to chunk `pks` (the
  intended fix: mirror `stats/service.rs`'s existing `crosschain_transfers`
  chunking pattern, `batch_size = PG_BIND_PARAM_LIMIT / 2`)~~ — **done, and
  that was not the fix that shipped.** The actual fix (task
  `stats-projection-unbatched-cohort-queries`, landed 2026-09-16): load-only
  chunking (never the aggregation) at a fixed constant,
  `bulk::ROW_IN_KEY_CHUNK = 2_000`, applied uniformly to every row-valued `IN`
  load in `stats/projection.rs` and `stats/service.rs` — not
  `PG_BIND_PARAM_LIMIT / 2`. A bind-derived size is wrong for this shape: the
  ceiling for a row-valued `IN` is Postgres's planner stack depth
  (`max_stack_depth`), which no bind-count arithmetic can express (see
  `bulk::ROW_IN_KEY_CHUNK`'s doc comment and
  `.memory-bank/rules/database.md` §Batching). `project_transfers_batch`'s
  aggregation body (union-find merge, contradiction de-dup,
  `stats_asset_edges` insert with no `on_conflict`) is unaffected — only its
  three loads (main load, deferral load, and the four `load_*` helpers) are
  chunked; the body still runs exactly once per cohort.
- the hot message buffer (`message_buffer/buffer.rs`) gains a capacity cap or
  other bound on cohort size, changing whether this failure mode is reachable
  at all (still open — see Deferred below)
- `bulk.rs`'s shared batching utility changes in a way that could be extended
  to cover composite-`IN` queries generically (not just `INSERT`/upsert) —
  `run_in_chunks` now exists as the row-valued-`IN` analogue of
  `run_in_batches`, added standalone (not a refactor of `run_in_batches`)
- `message_buffer/persistence.rs:268` (`delete_replaced_messages`) and `:415`
  (`remove_finalized_from_pending`) are fixed — see Deferred below; they carry
  the identical exposure and were deliberately left out of the 2026-09-16 fix

## Open Questions

- ~~whether other composite-key `IN` queries exist elsewhere in the codebase
  (outside `project_messages_batch`) that share this same unbatched-tuple-IN
  risk and haven't yet been large enough to surface it~~ — **resolved.** Yes:
  `message_buffer/persistence.rs:268` (`delete_replaced_messages`) and `:415`
  (`remove_finalized_from_pending`) are both 32 767-tuple row-valued `IN`
  DELETEs in the same maintenance transaction, fed by the same cohort —
  bind-safe but carrying the identical `OR`-tree stack-depth exposure.
  **Deliberately deferred, still unfixed** — see Deferred below.
  `persistence.rs:447` (`fetch_cursors`) is also an unchunked row-valued `IN`
  but is config-sized (bounded by `(bridge_id, chain_id)` pairs in config,
  not by cohort size), and is audited **safe** — no action needed.
- the exact tuple-count threshold at which `stack depth limit exceeded`
  becomes reachable in this repo's production Postgres configuration (depends
  on `max_stack_depth` and query shape/joins; not yet empirically pinned down
  beyond "well under 40632 tuples, since that cohort hit the bind-count
  ceiling instead"). Still open — the 2026-09-16 fix does not resolve this
  empirically, it sizes `ROW_IN_KEY_CHUNK` with an order-of-magnitude margin
  below the known upper bound instead.

## Deferred

Filed during the 2026-09-16 fix (`stats-projection-unbatched-cohort-queries`),
deliberately left out of that change to keep the hotfix diff tight:

- **`message_buffer/persistence.rs:268` (`delete_replaced_messages`) and
  `:415` (`remove_finalized_from_pending`)** — both `run_in_batches(&keys, 2,
  …)` wrapping a row-valued `IN` DELETE at 32 767 tuples per chunk. Bind-safe,
  but carrying the identical `OR`-tree stack-depth exposure, in the same
  maintenance transaction, fed by the same cohort. Two one-line changes to
  `run_in_chunks(&keys, bulk::ROW_IN_KEY_CHUNK, …)` once those existed in
  `bulk.rs` — which the 2026-09-16 fix created, specifically so this follow-up
  would not have to add them itself.
- a cohort capacity cap on the hot message buffer
  (`message_buffer/buffer.rs`, `plan_maintenance`) — the only fix that
  addresses transaction *duration* under a pathological cohort; chunking
  converts a hard failure into a slow success, it does not make a
  pathological cohort process quickly. Watch `BUFFER_MAINTENANCE_DURATION`
  (`message_buffer/maintenance.rs`) on the next heavy catch-up.
- the `unnest` array-join query shape (two binds, no `OR`-tree, immune to
  both ceilings) — the structurally superior long-term answer, held in
  reserve pending verification of `sea-query`'s placeholder behavior for
  `Expr::cust_with_values` and an `EXPLAIN ANALYZE` comparison for small
  cohorts.
