# Stage Profiler v2 Lifecycle Model

## Scope

This note records the upstream Stage Profiler v2 lifecycle model consumed by `tac-operation-lifecycle`. It covers the semantics of the new `POST /v2/stage-profiling` response and the boundaries it changes: Stage Profiler, the indexer, and the Read API.

It does not choose database schema, migrations, Read API versioning, or implementation strategy.

## Contract Facts

- The request accepts one or more operation identifiers and returns one lifecycle record per operation.
- An operation exposes four independent concepts:
  - `operationType` — the cross-chain route, such as `TAC-TON`, `TON-TAC`, or `TON-TAC-TON`.
  - `finalized` — whether lifecycle polling must stop.
  - `status` — the business outcome, observed as `success` or `failed`; it may be absent.
  - `rollback` — whether the operation is currently rolled back.
- Stage-level data remains available independently, including existence, success, timestamps, transactions, and diagnostic notes.

## Confirmed Interpretation Rules

- `finalized=true` is the sole criterion for stopping Stage Profiler polling. `status` and `rollback` do not change that decision.
- `finalized=false` means the operation remains in public `PENDING` state and continues to be polled, even when `status=failed` or `rollback=true`.
- A finalized operation with `status=success` is a successful final outcome.
- A finalized operation with `status=failed` is a failed final outcome; `rollback` may be either `true` or `false`.
- `rollback=true` by itself is not terminal. A rolled-back operation may become executable later, for example after an insufficient executor fee is supplied.
- The route is now known before finalization. A non-final operation may therefore be publicly `PENDING` while already carrying a concrete route type.

## Observed Examples

| Lifecycle facts | Interpretation |
| --- | --- |
| `finalized=true`, `status=success`, `rollback=false` | Successful final operation; stop polling. |
| `finalized=true`, `status=failed`, `rollback=true` | Failed final operation with rollback; stop polling. |
| `finalized=true`, `status=failed`, `rollback=false` | Failed final operation without rollback; stop polling. |
| `finalized=false`, `status=failed`, `rollback=true`, insufficient executor fee | Non-final pending operation; continue polling because it may resume after the fee is supplied. |

## Relationship To The Previous Model

### Previous Stage Profiler Model

The earlier `POST /stage-profiling` response exposed a single `operationType` field. The field mixed several different meanings:

- the cross-chain route: `TAC-TON`, `TON-TAC`, or `TON-TAC-TON`;
- the non-final lifecycle state: `PENDING`;
- rollback as a final shape: `ROLLBACK`;
- a local service-specific interpretation of insufficient-fee failures.

The service could not reliably know a concrete route while an operation remained `PENDING`. It repeatedly queried the operation until the type changed to a final route or rollback. The old integration also derived `INSUFFICIENT_FEE` locally from a failed stage note containing both “insufficient” and “fee”.

The old polling decision was inferred from that overloaded type:

- terminal: route types, `ROLLBACK`, and unparseable types handled as an error fallback;
- non-terminal: `PENDING` and locally-derived `INSUFFICIENT_FEE`;
- safety cap: non-terminal pending operations older than one week stopped being polled.

### V2 Difference

The earlier response overloaded `operationType`: it represented the route, pending state, rollback, and part of the final-outcome logic. The v2 contract separates those concerns.

## Codebase Behavior Before Adoption (historical)

> **Status: superseded.** The v2 model has since been adopted on branch `evgenkor/tac/staging-v2`. The section below describes the *pre-adoption* implementation and is kept only to explain why the change touched so many boundaries. For the current behaviour see `.memory-bank/operation-lifecycle.md` and `.memory-bank/api-surface.md`.

The implementation mirrored the old overloaded model end to end:

- The Stage Profiler client deserializes only `operation_type` plus stages. Its `OperationType` enum includes route types, `Pending`, `Rollback`, the locally derived `InsufficientFee`, and an error fallback.
- The indexer decides whether to stop polling through `OperationType::is_finalized()`. Routes and rollback are terminal; pending and insufficient-fee are not.
- `Indexer::process_operation_with_retries` applies the one-week cap only to old pending/insufficient-fee types, then writes the resulting technical work status as either `completed` or `pending`.
- The database persists the derived old type into `operation.op_type`. Pending-operation queries select that same column for `PENDING` or `INSUFFICIENT-FEE`, so the route/business outcome/polling decision remain coupled in storage.
- `TacDatabase::derive_operation_type` inspects failed stage notes and converts old `PENDING` into local `INSUFFICIENT-FEE` when the note contains both “insufficient” and “fee”.
- The v1 Read API reads `operation.op_type` and returns it as the sole public `type`; it does not expose the indexer work status. It therefore inherits the overloaded semantics.

These are source-of-truth observations, not an implementation prescription. They explain why adopting the v2 Stage Profiler model affects the client, polling loop, storage boundary, and public read boundary together.

The service currently treats the old type as both business meaning and a polling decision. Adapting to v2 must preserve the newly independent meanings rather than infer finality from the route or rollback.

## Affected Boundaries

- **Stage Profiler client:** consumes the v2 endpoint and its separate lifecycle fields.
- **Indexer:** bases repeat polling exclusively on `finalized`, while retaining the agreed protection for operations that remain non-final for more than one week.
- **Read API:** must be able to represent route, public outcome, finality, and rollback as separate concepts. The concrete contract design is an implementation-analysis decision.

## Open Question

The prior integration stopped polling a `PENDING` operation older than one week. It is not yet confirmed whether Stage Profiler v2 automatically marks such operations as finalized. Until that is established, retain the one-week safeguard as the agreed operational behavior. **Still open** after adoption — the safeguard is retained and only changes the technical work status.

## Adoption Note

One interpretation rule was deliberately *not* carried into the public API: "`finalized=false` means the operation remains in public `PENDING` state". That still holds for Read API v1, but Read API v2 reports `failed` as soon as the outcome is failed, regardless of finality, and does not expose `finalized` at all. `finalized` is treated as an indexer-only signal. Rationale and the full projection are recorded in `.memory-bank/api-surface.md`; the contract facts above remain an accurate description of *upstream*.

## Source Anchors

- Stage Profiler Swagger: `POST /v2/stage-profiling`.
- `tac-operation-lifecycle-logic/src/client/models/profiling.rs`.
- `tac-operation-lifecycle-logic/src/indexer.rs`.
- `tac-operation-lifecycle-logic/src/database.rs`.
- `tac-operation-lifecycle-server/src/services/operations.rs`.
- `tac-operation-lifecycle-proto/proto/v1/tac-operation-lifecycle.proto`.
