# Gotchas & Edge Cases

Observed during static code review (2026-07-15, branch `evgenkor/tac/staging-v2`). Verify before relying on them — some may be fixed later.

## 1. `UNKNOWN` op_type limbo (zombie rows)
If stage-profiling returns `UNKNOWN`, `is_finalized()==false` and the age-cap branch only covers `PENDING`/`INSUFFICIENT-FEE`, so the op is written back with `status=pending`, `op_type='UNKNOWN'`. But **no stream selects it**: `query_new_operations` wants `op_type IS NULL`, `query_pending_operations` wants `op_type IN ('PENDING','INSUFFICIENT-FEE')`. Such rows are never re-polled and never completed — permanently stuck as `pending`/UNKNOWN. (indexer.rs:535-557, database.rs:663-706)

## 2. Ops missing from a profiling response stay `processing`
`process_operation_with_retries` iterates the *response* map. If the API response omits a requested id (only logs "unknown operation" for the inverse case), the omitted op keeps `status=processing` forever; it's only rescued by the startup `reset_processing_operations()` (→ `pending`). No periodic in-process sweep exists.

## 3. `failed` ≠ business failure
DB `status=failed` only means an HTTP/parse failure of the indexer's own fetch. Business failure is `ROLLBACK` op_type or stage `success=false`. Don't mix them up when querying the DB.

## 4. Unbounded retries, linear backoff
`retry_count` has no maximum; `next_retry = now + 5s × (retry_count+1)` (base delay hardcoded, not in settings). A permanently-broken op/interval retries forever at slowly increasing spacing, floored by the 60s retry-stream scan period.

## 5. `ERROR` type is silently terminal
Any unrecognized `operationType` string from the API → serde `#[serde(other)]` → `ErrorType` → `is_finalized()==true` → `completed`. A new upstream op_type added to the API would freeze all such operations as ERROR(0) with no re-poll. Watch for this on upstream API upgrades.

## 6. README/env-docs defaults drift from code
`settings.rs` defaults vs README table: `polling_interval` 1s (README: 2), `retry_interval` 60s (README: 120), `start_timestamp` 1740787200 = 2025-03-01 (README: 0). Trust `settings.rs`.

## 7. Realtime thread starts from watermark, not from `Indexer::realtime_boundary`
`start()` passes `db.get_watermark()` as the realtime thread's initial boundary, while the historical/realtime *stream split* uses the constructor-computed `realtime_boundary` (`max(now-10min, latest op ts)`). Usually close, but they are different values; the realtime fetch window can initially overlap already-covered historical intervals (harmless: op insert is conflict-do-nothing).

## 8. Realtime boundary only advances on non-empty responses
By design (guards against upstream lag), but means an idle chain re-fetches the same growing `[boundary..now]` window every `polling_interval` (1s) indefinitely.

## 9. Stage rewrite is destructive
Every re-poll deletes and re-inserts all `operation_stage` rows (and their `transaction` rows via cascade? — no: transactions reference stage_id; they're re-inserted with new stage ids inside the same tx). `stage.id` values are therefore not stable identifiers across refreshes.

## 10. Raw-SQL claim queries interpolate strings
`build_interval_query`/`build_operation_query` format conditions (incl. timestamps) directly into SQL text — safe today (all inputs internal), but not parameterized; keep in mind if adding user-controlled filters.
