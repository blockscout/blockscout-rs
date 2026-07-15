# Memory Bank — tac-operation-lifecycle

Knowledge base for the TAC Operation Lifecycle microservice, collected from source-code research.

| File | Contents |
|---|---|
| [projectbrief.md](projectbrief.md) | What the service is, workspace layout, data model |
| [sync-architecture.md](sync-architecture.md) | Interval/watermark mechanism, job streams, priorities, realtime vs historical |
| [operation-lifecycle.md](operation-lifecycle.md) | **Operation status machine, terminal-state detection, interpretation of PENDING / ROLLBACK / failed states** |
| [api-surface.md](api-surface.md) | Upstream TAC API (client side) and the served gRPC/REST API |
| [gotchas-and-edge-cases.md](gotchas-and-edge-cases.md) | Known gaps, limbo states, doc/code discrepancies |

Last updated: 2026-07-15 (branch `evgenkor/tac/staging-v2`).
