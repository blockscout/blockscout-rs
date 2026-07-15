# Project Brief

**tac-operation-lifecycle** indexes TAC "Operations" (cross-chain user actions between TON and the TAC EVM L2) and serves them via gRPC/REST for Blockscout UI. It does **not** read blockchains directly — it polls a remote TAC data API (`RPC__URL`, e.g. `https://data.turin.tac.build/`) and mirrors operation state into Postgres.

## Workspace layout

| Crate | Role |
|---|---|
| `tac-operation-lifecycle-logic` | Core: `Indexer` (indexer.rs), `TacDatabase` (database.rs), HTTP `Client` (client/), settings |
| `tac-operation-lifecycle-entity` | SeaORM entities: `operation`, `operation_stage`, `transaction`, `interval`, `watermark`, `operation_meta_info`, `stage_type` |
| `tac-operation-lifecycle-migration` | SeaORM migrations |
| `tac-operation-lifecycle-proto` | Proto/OpenAPI definitions (v1) |
| `tac-operation-lifecycle-server` | gRPC+HTTP server, services (operations, statistic, health) |

## Data model (Postgres)

- **operation** — `id` (TEXT PK, the operationId from TAC API), `op_type` (TEXT, nullable; NULL until first profiling fetch), `timestamp`, `sender_address/_blockchain`, `status` (`status_enum`: `pending|processing|completed|failed`), `next_retry`, `retry_count`.
- **operation_stage** — per-operation lifecycle stages (6 types: CollectedInTAC, IncludedInTACConsensus, ExecutedInTAC, CollectedInTON, IncludedInTONConsensus, ExecutedInTON), each with `success` (bool), `timestamp`, `note`. Deleted and fully re-inserted on every profiling refresh.
- **transaction** — tx hashes attached to stages, with blockchain type (Tac/Ton).
- **interval** — time-window work units for discovery, same `status_enum` + retry fields.
- **watermark** — single row: latest timestamp covered by intervals.
- **operation_meta_info** — fees and valid executors per chain (upserted).

## Two-phase sync

1. **Discovery**: fetch operation IDs per time interval (`GET /operation-ids?from=&till=`) → insert `operation` rows with `status=pending`, `op_type=NULL`.
2. **Profiling**: fetch per-operation stage data (`POST /stage-profiling`) → write op_type, stages, meta; decide whether the operation is terminal (see [operation-lifecycle.md](operation-lifecycle.md)).
