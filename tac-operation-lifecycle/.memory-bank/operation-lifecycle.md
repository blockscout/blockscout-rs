# Operation Lifecycle: Status Machine & Terminal-State Detection

Source of truth: `Indexer::process_operation_with_retries` (indexer.rs:521), `OperationType::is_finalized` (indexer.rs:64), `TacDatabase::set_operation_data` / `derive_operation_type` (database.rs:772, 867).

## Two orthogonal state dimensions

1. **DB `status`** (`status_enum`: `pending | processing | completed | failed`) — *indexer bookkeeping*: does this row still need work?
   - `pending` — needs (re-)profiling; picked up by new/pending operation streams.
   - `processing` — claimed by a worker right now (reset to `pending` on service restart).
   - `failed` — the *fetch attempt* failed (HTTP/parse error), scheduled for retry via `next_retry`. **NOT** a business failure of the operation itself.
   - `completed` — **terminal**. No query ever selects `completed` operations for refetching; the row is frozen except for the sweep migration below.
2. **`op_type`** (TEXT, from TAC API `stage-profiling` response) — *business state*: `PENDING`, `TON-TAC-TON`, `TAC-TON`, `TON-TAC`, `ROLLBACK`, `INSUFFICIENT-FEE` (locally derived), `UNKNOWN`, `ERROR` (serde fallback for unrecognized strings). `NULL` = never profiled yet.

## The terminal-state decision (indexer.rs:535-557)

After each successful `POST /stage-profiling` response, per operation:

```
new_status =
  if op_type.is_finalized()                      -> completed   (terminal)
  else if op_type in {PENDING, INSUFFICIENT-FEE}
       and op.timestamp < now - forever_pending_operations_age_sec
                                                 -> completed   (terminal, "forever pending")
  else                                           -> pending     (will be re-polled)
```

`is_finalized()` (indexer.rs:64-75):

| op_type | finalized? |
|---|---|
| TON-TAC-TON, TAC-TON, TON-TAC | ✅ terminal — bridging flow reached its end shape |
| ROLLBACK | ✅ terminal — the protocol rolled the operation back; that *is* its final outcome |
| ERROR (unparseable type) | ✅ terminal — defensive: don't re-poll garbage forever |
| PENDING | ❌ keep polling |
| INSUFFICIENT-FEE | ❌ keep polling (fee could still be topped up / executed) |
| UNKNOWN | ❌ not finalized — but see limbo gotcha below |

**Forever-pending cap**: `forever_pending_operations_age_sec` (default **1 week**, `TAC_OPERATION_LIFECYCLE__INDEXER__FOREVER_PENDING_OPERATIONS_AGE_SEC`). An operation still `PENDING`/`INSUFFICIENT-FEE` whose *operation timestamp* is older than a week is force-marked `completed` and never rechecked ("Forever pending operation has been found" warn log). The op_type stays `PENDING`/`INSUFFICIENT-FEE` — the UI shows it as such; only the indexer stops caring. Rationale (comment in README): the 1-week bound is hardcoded by the protocol itself.

So: **"stop refetching" ⇔ DB `status=completed`**, reached either by a finalized op_type or by the 1-week pending age cap.

## Full status flow

```
discovered (interval / realtime fetch)
   INSERT status=pending, op_type=NULL
        │  claimed by new_operations_stream (op_type IS NULL)
        ▼
   status=processing ── POST /stage-profiling ──┐
        │                                       │
        │ batch fetch error                     │ per-op response
        ▼                                       ▼
   status=failed                 ┌─ finalized type ────────────► status=completed  [TERMINAL]
   next_retry=now+5s*attempts    ├─ PENDING/INSUFF-FEE > 1wk ──► status=completed  [TERMINAL]
   (retry_operations_stream      ├─ PENDING/INSUFF-FEE ≤ 1wk ──► status=pending ──► re-claimed by
    re-claims after next_retry)  │                                 pending_operations_stream (loop)
                                 └─ UNKNOWN ───────────────────► status=pending    [LIMBO: no stream selects it]
```

Every profiling write (`set_operation_data`) is one transaction: delete all existing stages for the op → update op row (op_type, sender, status) → re-insert stages + their transactions → upsert meta_info. Re-polls therefore fully replace stage history each time.

## Interpretation of specific states

### "Failed"
- The upstream TAC API **has no FAILED operation type**. On-chain failure surfaces as either op_type `ROLLBACK`, or per-stage `success=false` with a human `note`.
- The service stores stage-level `success`/`note` verbatim (`operation_stage` table) and exposes them via `status_history` in the API. It draws **no operation-level "failed" conclusion** from stage failures — except the insufficient-fee derivation below.
- DB `status=failed` strictly means "the indexer's HTTP fetch failed; will retry" (linear backoff `5s × retry_count`, retried every `retry_interval`=60s scan, unbounded attempts).

### "Rollbacked"
- `ROLLBACK` arrives as an ordinary op_type from stage-profiling; `is_finalized()==true` → immediately `completed`, never re-polled. Semantics: the operation's effects were reverted (e.g. assets returned on origin chain) — a *final outcome*, not an error to retry.

### "Pending"
- `PENDING` op_type means the cross-chain flow hasn't reached a final shape yet. The row keeps DB `status=pending` and cycles through `pending_operations_stream` → `processing` → back, at the **highest** stream priority, until the type flips to a finalized one or the 1-week age cap fires.

### "Insufficient fee" (locally derived)
- The API reports these as `PENDING`. `derive_operation_type` (database.rs:867) rewrites the type to `INSUFFICIENT-FEE` when op_type==PENDING **and** any stage has `success==false` and note containing (case-insensitive) both `"insufficient"` and `"fee"`.
- Treated exactly like PENDING for polling/terminality (the explicit `InsufficientFee` check in the age-cap condition is documented as redundancy/future-proofing, since the API keeps returning them as PENDING).
- Migration `m20260304_204118_mark_insufficient_fee_operations` retroactively applied the same rule (SQL `ILIKE '%insufficient%' AND '%fee%'`, only `success=FALSE` stages) to pre-existing rows.
