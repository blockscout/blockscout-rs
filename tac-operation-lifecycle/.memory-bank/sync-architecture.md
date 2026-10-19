# Sync Architecture

Source: `tac-operation-lifecycle-logic/src/indexer.rs`, `database.rs`.

## Timeline dissection: intervals + watermark

- The historical timeline `[start_timestamp .. realtime_boundary]` is chopped into fixed intervals of `catchup_interval` seconds (default 5s) by `generate_historical_intervals()` → `TacDatabase::generate_pending_intervals()`. Each interval is a DB row with `status=pending`.
- The **watermark** (single DB row) = end of the last generated/completed interval; it advances inside the same DB transaction whenever intervals are inserted.
- **realtime_boundary** (in-memory, computed at startup in `Indexer::new`): `max(now - 10 min, latest operation timestamp in DB)`. It splits interval processing into "historical" (below) and "realtime" (above) regimes.

## Realtime thread (`create_realtime_thread`)

Separate tokio task, loops every `polling_interval` (default 1s):
1. `GET /operation-ids` from `realtime_bnd` to `now` (initial `realtime_bnd` = **DB watermark**, not the constructor's realtime_boundary).
2. Insert found operations as `pending` (`insert_pending_operations`, conflict-do-nothing on id).
3. On success, insert an already-`completed` interval `[realtime_bnd, max_op_timestamp]` (marks range as covered, advances watermark) and move `realtime_bnd = max_op_timestamp + 1`.
- Note: boundary only advances when operations were found; empty ranges are re-requested from the same start — intentional protection against the remote API lagging ("falsely empty response").

## Job streams (all infinite async streams polling the DB)

Claim semantics: each `query_*` uses raw SQL `SELECT ... FOR UPDATE SKIP LOCKED` + `UPDATE ... SET status='processing' RETURNING ...` — atomically claims a batch, safe for concurrent consumers.

| Stream | DB selection | Batch / delay |
|---|---|---|
| `new_operations_stream` | `operation`: `status=pending AND profiling_version IS NULL` (never profiled) | `operations_query_batch`=10 / 200ms |
| `pending_operations_stream` | `operation`: `status=pending AND ((profiling_version=2 AND finalized=FALSE) OR (profiling_version=1 AND op_type IN ('PENDING','INSUFFICIENT-FEE')))` | 10 / 200ms |
| `interval_stream` (×3 instances) | `interval`: `status=pending` within `[from..to]`, ASC or DESC | `intervals_query_batch`=10 / 100ms |
| `retry_intervals_stream` | `interval`: `status=failed AND next_retry < now` | `intervals_retry_batch`=10 / `retry_interval`=60s |
| `retry_operations_stream` | `operation`: `status=failed AND next_retry < now` | 10 / 60s |

## Stream priority (`select_with_strategy`, left-biased)

```
1. pending_operations           (re-polling live PENDING/INSUFFICIENT-FEE ops)
2. new_operations               (first profiling of just-discovered ops)
3. realtime intervals           (start >= realtime_boundary, ASC)
4. historical intervals DESC    (newest-first, high prio)
5. historical intervals ASC     (oldest-first, low prio)
6. retry streams                (failed intervals + failed operations, unbiased select)
```

The combined stream is consumed with `for_each_concurrent(concurrency)` (default = CPU cores). Interval jobs → `process_interval_with_retries` (fetch op IDs, insert pending ops, mark interval `completed`); operation jobs → `process_operation_with_retries` (fetch stage profiling, see [operation-lifecycle.md](operation-lifecycle.md)).

## V2 re-profiling worker (`create_v2_backfill_thread`)

A separate tokio task, deliberately **outside** the prioritized stream so legacy convergence never competes with live traffic.

- Disabled entirely in `stage_profiling_mode = v1_only` (it could never advance the version).
- Claims `op_type IS NOT NULL AND profiling_version=1 AND status IN ('pending','completed')`, newest-first, `operations_query_batch` at a time, with `FOR UPDATE SKIP LOCKED` — effective concurrency one, reusing the shared HTTP rate limiter.
- Calls `get_operations_stages_v2` **directly**; it must never fall back to v1, which could not raise the version.
- Pauses while the prefer-v2 circuit is open, and may consume the single post-cooldown probe so recovery does not depend on new live operations arriving. `release_v2_probe()` hands an unused reservation back.
- Stays alive after the queue drains (a later v1 fallback can create version-1 rows again), sleeping `max(retry_interval, 60s)`.
- Convergence is reported at INFO only on the 0-transition, and is measured with `count_v1_operations_for_backfill` — the *claimable* predicate. Rows parked in the retry stream are counted separately (`count_v1_operations_awaiting_retry`) and logged alongside, so one permanently failing legacy row cannot suppress the report forever.
- Failures and omitted ids go to the ordinary technical retry path; rows abandoned in `processing` are recovered by the startup reset.

## Startup sequence (`Indexer::start`)

0. Bail out immediately if `indexer.enabled = false` (`INDEXER__ENABLED`) — the API keeps serving, nothing is fetched.
1. `ensure_stages_types_exist` — upsert the 6 stage types.
2. `generate_historical_intervals(realtime_boundary)` — fill gap between watermark and realtime boundary.
3. `reset_processing_operations/intervals` — anything stuck in `processing` (crash mid-flight) is reset to `pending`.
4. Spawn the v2 re-profiling worker — **after** the reset, so it cannot race rows being recovered.
5. Spawn realtime thread; build and consume prioritized streams (runs forever).

## Failure handling & retry

- Interval fetch failure → `set_interval_retry`: `status=failed`, `retry_count+=1`, `next_retry = now + 5s * (retry_count+1)` (linear backoff, no cap on attempts).
- Operation profiling failure (whole batch HTTP/parse error) → same via `set_operation_retry` for each op in batch.
- **Ids omitted from an otherwise successful response** also go to retry (`persist_operation_results`), so they are no longer stranded in `processing`. In `prefer_v2` the omitted subset is first re-requested from v1, and only ids missing from the final permitted source are retried. A v1 recovery result is accepted **only** for ids that v2 actually omitted — anything else is logged and discarded so it cannot overwrite authoritative v2 data.
- Retry streams re-claim them once `next_retry` passes, flipping back to `processing`.
- HTTP client (`client/mod.rs`): rate-limited (governor, `request_per_second`, now **5** in the deployed configs), `num_of_retries` attempts waiting on the limiter; note the retry loop only retries limiter-timeout, an actual HTTP error is returned immediately.

## Stage Profiler source selection & circuit breaker

`stage_profiling_mode`: `prefer_v2` (default) / `v2_only` / `v1_only`; `stage_profiling_v2_probe_interval` (default 60s).

In `prefer_v2`, an *availability or contract* failure of v2 (connect/timeout/request transport errors, HTTP `404|405|410|500|501|502|503|504`, undeserializable or empty body) falls the same batch back to v1 and opens a process-wide circuit for the probe interval. While open, live traffic goes straight to v1 without re-hitting a known-broken v2. After cooldown exactly one caller reserves a probe (`Mutex`-guarded, shared across `Client` clones — verified under 16 concurrent callers); success closes the circuit, an eligible failure reopens it.

`400|401|403|422|429`, other auth/validation failures and local rate-limiter exhaustion **never** fall back — they surface to the technical retry path so configuration and contract errors stay visible.

Responses are source-tagged (`ProfilingResponse::V1`/`V2`); the version is never inferred from field values. A v1 refresh may legitimately downgrade a version-2 row back to version 1 — intentional, so stale canonical facts are never served as current.
