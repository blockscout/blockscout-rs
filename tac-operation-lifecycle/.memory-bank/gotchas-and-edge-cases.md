# Gotchas & Edge Cases

Originally observed 2026-07-15; revised after the Stage Profiler v2 adoption on branch `evgenkor/tac/staging-v2`. Verify before relying on them.

## 1. `op_type` has a version-dependent meaning — never parse it alone
For `profiling_version=2` it is the **route only**; for `profiling_version=1` it is the old overloaded value (`PENDING`, `ROLLBACK`, `INSUFFICIENT-FEE`, `ERROR_TYPE`, …); `NULL` means never profiled. Every read, claim predicate and migration must branch on `profiling_version` first. This is transitional — once backfill converges every profiled row is version 2 — but "transitional" lasts as long as v1 fallback can still happen.

## 2. A v1 fallback can move a row *backwards* from version 2 to version 1
Intentional: a successful v1 refresh clears `op_status`/`finalized`/`rollback`/`error_reason` so stale canonical facts are never presented as current. All readers and claim predicates must tolerate both directions. The backfill worker upgrades the row again after v2 recovers.

## 3. `UNKNOWN` op_type limbo (narrowed, not gone)
A version-1 row with `op_type='UNKNOWN'` gets `status=pending` (`is_finalized()==false`), but no live stream selects it: `query_new_operations` wants `profiling_version IS NULL`, `query_pending_operations` wants `PENDING`/`INSUFFICIENT-FEE`. In `prefer_v2`/`v2_only` the backfill worker now rescues such rows; in `v1_only` they are still permanently stuck. Pre-existing behaviour, unchanged by the v2 work.

## 4. `failed` ≠ business failure (now three different "failures")
- DB `status=failed` — the indexer's own fetch failed; will retry.
- `op_status='failed'` — the business outcome reported by upstream v2.
- stage `success=false` — a single lifecycle stage failed.
Legacy version-1 rows have no `op_status` at all; business failure there is only `ROLLBACK` or a failed stage.

## 5. `rollback=true` is not terminal
A rolled-back operation may still resume (e.g. after an insufficient executor fee is supplied). Only `finalized` stops polling. Equally, `op_status='failed'` on a non-final operation is temporary from the indexer's point of view — even though the public v2 API deliberately shows it as `failed` right away (see `api-surface.md`).

## 6. The forever-pending cap must never rewrite `finalized`
It only flips technical `status` to `completed`. If someone "simplifies" it into setting `finalized=true`, age-capped operations would start rendering as final routes in v1 and as success/failed in v2. There are tests pinning the current behaviour; keep them.

## 7. Unbounded retries, linear backoff
`retry_count` has no maximum; `next_retry = now + 5s × (retry_count+1)` (base delay hardcoded, not in settings). A permanently-broken op/interval retries forever at slowly increasing spacing, floored by the 60s retry-stream scan period. Consequence for convergence: such rows are version-1 forever, which is why the backfill counter excludes `status=failed` rows.

## 8. `ERROR_TYPE` is silently terminal — for v1 data only
An unrecognized `operationType` from **v1** → `#[serde(other)]` → `ErrorType` → `is_finalized()==true` → `completed`. For **v2** an unrecognized route is stored verbatim (`OperationRoute::Unrecognized`) and cannot affect terminality, which depends on `finalized` alone. Note the v2 read API collapses any unrecognized route to `UNKNOWN`, so a new upstream route is invisible in the API until it is added to `V2OperationType`.

## 9. The down migration writes `'ERROR'`, the service writes `'ERROR_TYPE'`
`LegacyOperationType::ErrorType` serialises as `ERROR_TYPE` (serde `SCREAMING_SNAKE_CASE`); the down migration's fallback branch writes `'ERROR'`. Both parse back to `ErrorType` via `FromStr`, so there is no functional difference — just don't be surprised by two spellings in historical data.

## 10. Down migration requires the new binary to be stopped
It reconstructs the overloaded `op_type` for every version-2 row (including the insufficient-fee stage-note rule) before dropping the columns. A running new binary would concurrently write route-only values back.

## 11. README/env-docs defaults drift from code
`settings.rs` defaults vs README table: `polling_interval` 1s (README: 2), `retry_interval` 60s (README: 120), `start_timestamp` 1740787200 = 2025-03-01 (README: 0). Trust `settings.rs`. Note the deployed TOMLs now set `request_per_second = 5`, far below the code default of 100.

## 12. Realtime thread starts from watermark, not from `Indexer::realtime_boundary`
`start()` passes `db.get_watermark()` as the realtime thread's initial boundary, while the historical/realtime *stream split* uses the constructor-computed `realtime_boundary` (`max(now-10min, latest op ts)`). Usually close, but different values; the realtime fetch window can initially overlap already-covered historical intervals (harmless: op insert is conflict-do-nothing).

## 13. Realtime boundary only advances on non-empty responses
By design (guards against upstream lag), but means an idle chain re-fetches the same growing `[boundary..now]` window every `polling_interval` (1s) indefinitely.

## 14. Stage rewrite is destructive
Every re-poll — including every backfill upgrade — deletes and re-inserts all `operation_stage` rows and their `transaction` rows inside one transaction. `stage.id` values are therefore not stable identifiers across refreshes.

## 15. Raw-SQL claim queries interpolate strings
`build_interval_query`/`build_operation_query` format conditions (incl. timestamps and the version predicates) directly into SQL text — safe today (all inputs internal), but not parameterized; keep in mind if adding user-controlled filters.

## 16. `error_reason` in the DB and in the API can legitimately differ
The column stores the full derived reason; the v2 API withholds anything longer than 16 characters rather than truncating. A failed operation with no `error_reason` in the response is normal, not a bug. See `api-surface.md`.

## Fixed by the v2 work (kept for history)
- *Ops missing from a profiling response stayed `processing` forever* — `persist_operation_results` now schedules them for technical retry, after attempting a per-id v1 recovery in `prefer_v2`.
- *`INSUFFICIENT-FEE` was materialized during indexing* — for version-2 data it is now derived at read time (batched, indexed), so indexing no longer bakes a compatibility rule into storage.
