# API Surface

## Upstream (consumed): TAC data API (`client/mod.rs`)

Base URL: `TAC_OPERATION_LIFECYCLE__RPC__URL` (e.g. `https://data.turin.tac.build/`). Rate-limited client (governor), `request_per_second`=100 default.

1. `GET /operation-ids?from=&till=&offset=` → `{ response: { total, operations: [{operationId, timestamp}] } }`
   - Used by interval jobs and the realtime thread for discovery. Client auto-paginates via `offset` until `total` reached or empty page.
2. `POST /stage-profiling` body `{"operationIds": [...]}` → `{ response: { <opId>: OperationData } }`
   - `OperationData`: `operationType` (SCREAMING_SNAKE / hyphenated, e.g. `TON-TAC-TON`, `PENDING`), `metaInfo` (initialCaller, validExecutors, feeInfo per chain), plus flattened map of 6 stage keys → `{ exists, stageData: { success, timestamp, transactions[], note } }`.
   - `note` may be a JSON object — coerced to string by `deserialize_note_to_string`.
   - Unknown `operationType` strings deserialize to `ErrorType` via `#[serde(other)]`.

## Served API (proto v1, `tac-operation-lifecycle.proto`)

- `GET /api/v1/tac/operations` (`GetOperations`) — list / multi-search (`q` = operation id | tx hash | TON/TAC sender address), timestamp-based pagination (PAGE_SIZE=50).
- `GET /api/v1/tac/operations/{operation_id}` (`GetOperationDetails`) — full details incl. `status_history` (stages + transactions).
- `GET /api/v1/tac/operations:byTransaction/{tx_hash}` style (`GetOperationsByTransaction`) — full operations touching a tx.
- statistic.proto — interval/operation counters from `get_intervals_statistic` / `get_operations_statistic`.

### Type mapping (server/src/services/operations.rs)

DB `op_type` TEXT → parsed back into `OperationType` → numeric proto enum via `to_id()`:
`ERROR=0, PENDING=1, TON_TAC_TON=2, TAC_TON=3, TON_TAC=4, ROLLBACK=5, UNKNOWN=6, INSUFFICIENT_FEE=7`.
- `op_type=NULL` (not yet profiled) → served as `UNKNOWN`.
- Unparseable stored string → `ERROR`.
- The **DB `status` column is not exposed** through either API version and is not part of any projection — consumers only see op_type/status and per-stage success/notes. Stage `timestamp` is only serialized when the stage has transactions.
- Stage type ids: DB stores 1-based (`StageType::to_id` 1..6), proto enum is 0-based → served as `stage_type_id - 1`.

## Served API (proto v2, `proto/v2/tac-operation-lifecycle.proto`)

- `GET /api/v2/tac/operations`, `GET /api/v2/tac/operations/{operation_id}`, `GET /api/v2/tac/operations:byTx/{tx_hash}` — same query/pagination/stage shape as v1.
- Fields: required `type` (`UNKNOWN|TON_TAC_TON|TAC_TON|TON_TAC`), required `status` (`pending|success|failed`), required `rollback`, optional `error_reason`, plus timestamp/sender/`status_history`.
- One Swagger artifact covers both versions and is served from `/api/v1/docs/swagger.yaml` and `/api/v2/docs/swagger.yaml`.

### v2 `status` is a product projection — accepted design, do not "fix"

`OperationsService::v2_lifecycle` deliberately does not mirror the upstream fields. A reviewer will notice the asymmetry; it is intentional:

- **Upstream `finalized` is not exposed** (`reserved "finalized"` in both v2 messages). It is an indexer-only signal for whether to keep re-requesting an operation.
- **`failed` ignores finality** — `op_status=failed` reads as `failed` even while the indexer still polls. The user cannot influence such an operation, and "failed and pending" is not a state worth surfacing. Detail lives in `rollback` / `error_reason`.
- **`success` requires finality** (`op_status=success` **and** `finalized=true`). A non-final success reads as `pending`, as does a row whose profiling has not been requested yet (`profiling_version IS NULL`). A version-2 row never has `finalized=true` with a NULL `op_status` — both columns are written together from one successful v2 response.
- **Legacy version-1 rows are mapped, not hidden**: `ROLLBACK`/`INSUFFICIENT-FEE` → `failed`, concrete route → `success`, anything else → `pending`; `rollback` is `true` only for stored `ROLLBACK` or a confirmed version-2 `rollback`. The v1 upstream model could not express a final failure without rollback, so a legacy route may read as `success` for an operation that actually failed — this matches what consumers already see today, and the v2 re-profiling worker converts these rows over time.
- `error_reason` is only exposed for version-2 rows whose projected status is `failed`; it is derived in `lifecycle::derive_operation_error_reason` from the latest failed stage note, with `Insufficient Fee` taking priority.
- **The length cap is a presentation rule, not a storage rule.** `operation.error_reason` keeps the derived value whatever its length; `v2_lifecycle` withholds it when it exceeds `MAX_ERROR_REASON_LEN` (16 characters, `server/src/services/operations.rs`), because longer values are raw upstream payloads (serialized revert data, whole message bodies) rather than labels. Consequences: a failed operation may come back with no `error_reason` at all, the DB column and the API field can legitimately disagree, and the untruncated text stays readable per stage via `status_history`. Never "fix" this by truncating — a cut-off payload is worse than none. Raising the cap is an API-only change; no backfill is needed.

Note: the original `task.md` success criterion "consumers can obtain route, business outcome, finality and rollback as separate concepts" was superseded by this projection during implementation (commits `50e436fe`, `aed50985`). Storage still keeps the four facts separate; the *public* surface intentionally does not.

## Ordering quirk

`get_full_operations_with_sql` sorts stages by `(timestamp, stage_type_id, id)`, but if any stage has a zero timestamp (known upstream API glitch) it falls back to `(stage_type_id, id)` ordering for the whole operation.
