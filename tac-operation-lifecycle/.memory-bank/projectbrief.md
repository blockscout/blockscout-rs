# Project Brief

**tac-operation-lifecycle** indexes TAC "Operations" (cross-chain user actions between TON and the TAC EVM L2) and serves them via gRPC/REST for Blockscout UI. It does **not** read blockchains directly — it polls a remote TAC data API (`RPC__URL`, e.g. `https://data.turin.tac.build/`) and mirrors operation state into Postgres.

## Workspace layout

| Crate | Role |
|---|---|
| `tac-operation-lifecycle-logic` | Core: `Indexer` (indexer.rs), `TacDatabase` (database.rs), HTTP `Client` (client/), settings |
| `tac-operation-lifecycle-entity` | SeaORM entities: `operation`, `operation_stage`, `transaction`, `interval`, `watermark`, `operation_meta_info`, `stage_type` |
| `tac-operation-lifecycle-migration` | SeaORM migrations |
| `tac-operation-lifecycle-proto` | Proto/OpenAPI definitions (`proto/v1/`, `proto/v2/`, one shared `api_config_http.yaml`); both packages compile in one pass into a single Swagger artifact |
| `tac-operation-lifecycle-server` | gRPC+HTTP server, services (operations, statistic, health) |

## Data model (Postgres)

- **operation** — `id` (TEXT PK, the operationId from TAC API), `timestamp`, `sender_address/_blockchain`, `status` (`status_enum`: `pending|processing|completed|failed`, indexer bookkeeping only), `next_retry`, `retry_count`, plus the lifecycle columns:
  - `op_type` (TEXT, nullable) — **meaning depends on `profiling_version`**: the cross-chain route for version 2, the old overloaded value for version 1, `NULL` until first profiling.
  - `profiling_version` (SMALLINT, nullable) — `NULL` never profiled, `1` Stage Profiler v1, `2` Stage Profiler v2.
  - `op_status` (TEXT, nullable) — business outcome `success`/`failed`, version-2 only, may be absent even then.
  - `finalized`, `rollback` (BOOLEAN, nullable) — version-2 only; `finalized` is the polling-terminal signal, `rollback` is independent and not terminal.
  - `error_reason` (TEXT, nullable) — locally derived short failure label for failed version-2 rows.
  - Partial indexes `idx_operation_v2_pending` (version-2 re-polling) and `idx_operation_v1_backfill` (legacy convergence).
- **operation_stage** — per-operation lifecycle stages (6 types: CollectedInTAC, IncludedInTACConsensus, ExecutedInTAC, CollectedInTON, IncludedInTONConsensus, ExecutedInTON), each with `success` (bool), `timestamp`, `note`. Deleted and fully re-inserted on every profiling refresh. Indexed on `operation_id`.
- **transaction** — tx hashes attached to stages, with blockchain type (Tac/Ton).
- **interval** — time-window work units for discovery, same `status_enum` + retry fields.
- **watermark** — single row: latest timestamp covered by intervals.
- **operation_meta_info** — fees and valid executors per chain (upserted).

## Two-phase sync

1. **Discovery**: fetch operation IDs per time interval (`GET /operation-ids?from=&till=`) → insert `operation` rows with `status=pending`, `op_type=NULL`, `profiling_version=NULL`.
2. **Profiling**: fetch per-operation stage data (`POST /v2/stage-profiling`, falling back to `POST /stage-profiling`) → write the source-appropriate lifecycle facts, stages and meta; decide whether the operation is terminal (see [operation-lifecycle.md](operation-lifecycle.md)).

Plus a third, background phase: a **v2 re-profiling worker** converts legacy `profiling_version=1` rows whenever v2 is available, so `op_type`'s version-dependent meaning is transitional (see [sync-architecture.md](sync-architecture.md)).

Two API versions are served side by side: v1 keeps its original overloaded `type` (computed at read time), v2 exposes the reshaped model. Both are described by one Swagger artifact (see [api-surface.md](api-surface.md)).
