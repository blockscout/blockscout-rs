# Operation Lifecycle: Status Machine & Terminal-State Detection

Source of truth: `Indexer::operation_work_status` and `Indexer::process_operation_with_retries` (indexer.rs), `lifecycle.rs` (`derive_v1_source_type`, `derive_operation_error_reason`, `project_v1_type`), `TacDatabase::set_operation_data` (database.rs).

Rewritten after the Stage Profiler v2 adoption. The single overloaded `op_type` no longer carries the whole lifecycle.

## Three orthogonal dimensions

1. **DB `status`** (`status_enum`: `pending | processing | completed | failed`) — *indexer bookkeeping only*: does this row still need work? Never exposed by either API version and never part of any public projection.
   - `pending` — needs (re-)profiling; picked up by the new/pending operation streams.
   - `processing` — claimed by a worker right now (reset to `pending` on service restart).
   - `failed` — the *fetch attempt* failed (HTTP/parse error), scheduled for retry via `next_retry`. **NOT** a business failure.
   - `completed` — the indexer stops polling. Not frozen anymore: the v2 backfill worker re-claims `completed` version-1 rows.
2. **`profiling_version`** (SMALLINT, nullable) — which upstream contract produced the row's data. `NULL` = never profiled, `1` = Stage Profiler v1, `2` = Stage Profiler v2. **Every reader must branch on it**, because it changes what `op_type` means.
3. **Canonical lifecycle facts** — for version-2 rows these are four independent columns:
   - `op_type` — the cross-chain **route only**: `TON-TAC-TON`, `TAC-TON`, `TON-TAC`, `UNKNOWN`, or an unrecognized upstream string stored verbatim;
   - `op_status` — business outcome, `success` / `failed`, **nullable** (upstream may omit it);
   - `finalized` — whether upstream considers the operation final; the sole normal polling-terminal signal;
   - `rollback` — whether the operation is currently rolled back; **not** terminal on its own;
   - plus `error_reason`, a locally derived short failure label (see below).

   For version-1 rows `op_type` keeps the **old overloaded** value (`PENDING`, a route, `ROLLBACK`, locally derived `INSUFFICIENT-FEE`, `UNKNOWN`, `ERROR_TYPE`) and `op_status` / `finalized` / `rollback` / `error_reason` are `NULL`.

## The terminal-state decision (`Indexer::operation_work_status`)

Version-tagged: the upstream response is wrapped in `SourceOperationData::V1` / `::V2`, so v2 semantics can never be applied to v1 data. Source is never inferred from field values.

```
V2 data:
  finalized == true                          -> completed   (normal terminal)
  finalized == false, op.timestamp older than
      forever_pending_operations_age_sec      -> completed   (local stop; warn)
  otherwise                                  -> pending      (re-polled)

V1 data (legacy contract):
  derive_v1_source_type(...).is_finalized()   -> completed
  op_type in {PENDING, INSUFFICIENT-FEE} and
      operation older than the age cap        -> completed   (local stop; warn)
  otherwise                                  -> pending
```

Route, `op_status`, `rollback`, stage failures and unparseable route strings **never** influence terminality for v2 data. `LegacyOperationType::is_finalized()` still applies to v1 data only (routes / `ROLLBACK` / `ERROR_TYPE` terminal; `PENDING` / `INSUFFICIENT-FEE` / `UNKNOWN` not).

### Forever-pending cap — a local stop, not a finality rewrite

`forever_pending_operations_age_sec` (default **1 week**). When it fires on a version-2 row:

- technical `status` becomes `completed`, so polling stops;
- canonical `finalized` **stays `false`** — the cap never rewrites upstream facts;
- v2 still reports the operation per its `op_status` projection, v1 still projects `PENDING` (or `INSUFFICIENT_FEE`).

So an age-capped operation can never turn into a final route just because the indexer gave up. Retained deliberately: it is **not** confirmed that Stage Profiler v2 eventually finalizes indefinitely-pending operations.

## Claim predicates (database.rs)

| Purpose | Predicate |
|---|---|
| `query_new_operations` | `status=pending AND profiling_version IS NULL` |
| `query_pending_operations` | `status=pending AND ((profiling_version=2 AND finalized=FALSE) OR (profiling_version=1 AND op_type IN ('PENDING','INSUFFICIENT-FEE')))` |
| `query_v1_operations_for_backfill` | `op_type IS NOT NULL AND profiling_version=1 AND status IN ('pending','completed')` |
| `query_failed_operations` | `status=failed AND next_retry < now` |

Version-2 re-polling depends on `finalized` alone. Backed by partial indexes `idx_operation_v2_pending` and `idx_operation_v1_backfill`.

## Full status flow

```
discovered (interval / realtime fetch)
   INSERT status=pending, op_type=NULL, profiling_version=NULL
        │  claimed by new_operations_stream (profiling_version IS NULL)
        ▼
   status=processing ── POST /v2/stage-profiling (or v1 fallback) ──┐
        │                                                          │
        │ batch fetch error / id omitted by the final source        │ per-op response
        ▼                                                          ▼
   status=failed                    ┌─ v2 finalized ──────────────────► completed  (finalized=true)
   next_retry=now+5s*attempts       ├─ v2 non-final, past age cap ────► completed  (finalized stays false)
   (retry_operations_stream         ├─ v2 non-final ──────────────────► pending ──► re-claimed by
    re-claims after next_retry)     │                                              pending_operations_stream
                                    ├─ v1 finalized type ─────────────► completed  (profiling_version=1)
                                    └─ v1 PENDING/INSUFF-FEE ─────────► pending

   any version-1 profiled row (pending or completed)
        │  claimed by the v2 backfill worker while v2 is available
        ▼
   status=processing ── POST /v2/stage-profiling ──► rewritten as profiling_version=2
```

`set_operation_data` is still one transaction per operation: delete all existing stages → update the operation row (route/version/outcome/finality/rollback/error_reason/sender/technical status) → re-insert stages and their transactions → upsert meta_info. Re-polls fully replace stage history, so `operation_stage.id` is not a stable identifier.

A successful **v1** write deliberately clears `op_status`, `finalized`, `rollback` and `error_reason` and sets `profiling_version=1`. Stale canonical facts must never survive a v1 refresh; the backfill worker upgrades the row again once v2 recovers. A *failed* request changes none of it — the last successful source version and its data stay intact.

## Interpretation of specific states

### "Failed"
- Upstream v2 reports it directly as `op_status=failed`, independently of finality and rollback. Upstream v1 had no failure field at all — failure surfaced only as `ROLLBACK` or per-stage `success=false`.
- DB `status=failed` still strictly means "the indexer's own fetch failed; will retry" (linear backoff `5s × retry_count`, unbounded attempts). Never conflate the two.
- Public presentation: v2 reports `failed` as soon as `op_status=failed`, without waiting for finality — an intentional product decision, see `api-surface.md`.

### "Rollbacked"
- Now an independent boolean. `rollback=true` is **not terminal**: a rolled-back operation may still resume (e.g. after an insufficient executor fee is supplied), so it keeps being polled while `finalized=false`.
- A final failure with `rollback=false` is a distinct, representable state — the old model could not express it.

### "Pending"
- For version-2 rows this means `finalized=false`, whatever the route, outcome or rollback flag says. The row cycles `pending → processing → pending` at the highest stream priority until upstream finalizes it or the age cap fires.

### "Insufficient fee"
- No longer a stored lifecycle value for version-2 rows. It is derived on demand from a failed stage note containing both `insufficient` and `fee` (case-insensitive):
  - `lifecycle::has_insufficient_fee_stages` for in-memory stage lists;
  - `TacDatabase::get_insufficient_fee_operation_ids` — one batched query for brief v1 list responses (never N+1), backed by the `operation_stage(operation_id)` index;
  - the same rule in SQL inside the down migration.
- For version-1 rows it is still *stored* as the overloaded `op_type='INSUFFICIENT-FEE'` (`derive_v1_source_type`), preserving the legacy behaviour, and migration `m20260304_204118_mark_insufficient_fee_operations` applied it retroactively to historical rows.

### `error_reason`
- Derived in `lifecycle::derive_operation_error_reason`, only for v2 data with `op_status=failed`: `Insufficient Fee` wins if the fee rule matches, otherwise the note of the **latest** failed stage (stage order `CollectedInTAC` → … → `ExecutedInTON`), unwrapped from JSON via the first non-empty of `internalMsg` / `content` / `errorName` / `internalBytesError`.
- Always stored in full; the API publishes it only when short enough to be a label (see `api-surface.md`).

## Public projections

Neither API version reads the technical `status`, and neither stores its projection — both are computed per request:

- **v1** `lifecycle::project_v1_type` reconstructs the old overloaded enum so existing consumers are unaffected. Version-1 rows return their stored value; version-2 rows are projected (`finalized=false` → `PENDING` / `INSUFFICIENT_FEE`; `finalized=true` + `failed` + `rollback` → `ROLLBACK`; otherwise the route).
- **v2** `OperationsService::v2_lifecycle` serves a deliberately reshaped product view. Details and the accepted design decisions live in `api-surface.md`.
