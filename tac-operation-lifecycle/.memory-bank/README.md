# Memory Bank — tac-operation-lifecycle

Knowledge base for the TAC Operation Lifecycle microservice, collected from source-code research.

| File | Contents |
|---|---|
| [projectbrief.md](projectbrief.md) | What the service is, workspace layout, data model |
| [sync-architecture.md](sync-architecture.md) | Interval/watermark mechanism, job streams, priorities, realtime vs historical |
| [operation-lifecycle.md](operation-lifecycle.md) | **Operation status machine, versioned lifecycle facts, terminal-state detection, interpretation of pending / rollback / failed states** |
| [api-surface.md](api-surface.md) | Upstream Stage Profiler v1+v2 (client side), the served v1/v2 gRPC/REST APIs, and the accepted v2 `status` projection |
| [gotchas-and-edge-cases.md](gotchas-and-edge-cases.md) | Known gaps, limbo states, doc/code discrepancies |
| [research/stage-profiler-v2-lifecycle-model.md](research/stage-profiler-v2-lifecycle-model.md) | Upstream Stage Profiler v2 lifecycle model (adopted; its "before" section is historical) |
| [workflows/](workflows/) | Reusable research, analysis, planning, implementation, review, and PR-description workflows |

Start with `operation-lifecycle.md` for anything about operation state, and `api-surface.md` before changing either API version — it records which projections are deliberate and must not be "fixed".

Last updated: 2026-08-07 (branch `evgenkor/tac/staging-v2`, after the Stage Profiler v2 adoption).
