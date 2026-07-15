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
| `new_operations_stream` | `operation`: `status=pending AND op_type IS NULL` (never profiled) | `operations_query_batch`=10 / 200ms |
| `pending_operations_stream` | `operation`: `status=pending AND op_type IN ('PENDING','INSUFFICIENT-FEE')` (non-terminal, needs re-poll) | 10 / 200ms |
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

## Startup sequence (`Indexer::start`)

1. `ensure_stages_types_exist` — upsert the 6 stage types.
2. `generate_historical_intervals(realtime_boundary)` — fill gap between watermark and realtime boundary.
3. `reset_processing_operations/intervals` — anything stuck in `processing` (crash mid-flight) is reset to `pending`.
4. Spawn realtime thread; build and consume prioritized streams (runs forever).

## Failure handling & retry

- Interval fetch failure → `set_interval_retry`: `status=failed`, `retry_count+=1`, `next_retry = now + 5s * (retry_count+1)` (linear backoff, no cap on attempts).
- Operation profiling failure (whole batch HTTP/parse error) → same via `set_operation_retry` for each op in batch.
- Retry streams re-claim them once `next_retry` passes, flipping back to `processing`.
- HTTP client (`client/mod.rs`): rate-limited (governor, `request_per_second`=100), `num_of_retries` attempts waiting on the limiter; note the retry loop only retries limiter-timeout, an actual HTTP error is returned immediately.
