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
- The **DB `status` column is not exposed** through the public API at all — consumers only see op_type and per-stage success/notes. Stage `timestamp` is only serialized when the stage has transactions.
- Stage type ids: DB stores 1-based (`StageType::to_id` 1..6), proto enum is 0-based → served as `stage_type_id - 1`.

## Ordering quirk

`get_full_operations_with_sql` sorts stages by `(timestamp, stage_type_id, id)`, but if any stage has a zero timestamp (known upstream API glitch) it falls back to `(stage_type_id, id)` ordering for the whole operation.
