# Architectural Decision Records

This directory contains Architectural Decision Records (ADRs) documenting significant technical decisions made in this project.

## What is an ADR?

An ADR captures the context, decision, and consequences of an architectural choice. They help:

- Understand why things are the way they are
- Onboard new team members
- Learn from past choices

## Index

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [001](./001-message-buffer-tiered-storage.md) | Message Buffer Tiered Storage | Accepted | 2026-01 |
| [002](./002-primary-chain-filtering.md) | Primary Chain Filtering for Unknown Chains | Proposed | 2026-02 |
| [003](./003-amb-event-based-transfers.md) | AMB Transfers Reconstructed From Events; Nullable Transfer Sides | Accepted | 2026-06 |
| [004](./004-stats-observability-horizon-and-asset-union-find.md) | Stats Eligibility From An Observability Horizon; Asset Identity As Union-Find | Accepted | 2026-07 |
| [005](./005-failed-range-ledger-and-checkpoint-independence.md) | Failed-Range Ledger, Independent of Checkpoints | Accepted | 2026-08 |
| [006](./006-contract-versioning-by-block.md) | Contract Versioning Resolved By Block, At Decode Time | Accepted | 2026-08 |
| [007](./007-scan-floor-reconciled-against-the-checkpoint.md) | The Scan Floor Is Reconciled Against The Checkpoint, Not `bridge_contracts` | Accepted (expires with bidirectional catch-up) | 2026-08 |
| [008](./008-per-chain-concurrency-within-a-bridge.md) | Per-Chain Concurrency Within A Bridge, Cooperative And Single-Task | Accepted | 2026-08 |

## Creating a New ADR

1. Copy `template.md` to `NNN-title.md` (next sequential number)
2. Fill in **relevant** sections, remove unnecessary ones
3. Update this index
4. Submit for review
