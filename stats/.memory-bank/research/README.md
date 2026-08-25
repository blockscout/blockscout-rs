# Research Notes

## Purpose

This directory stores durable, question-driven codebase research. A research
note should explain one non-trivial runtime behavior or architectural flow well
enough that future agents do not need to rediscover it from scratch.

Research notes are deeper than overview docs (`architecture.md`,
`project-context.md`) and more stable than temporary notes in `tmp/`.

## When to Create a Research Note

Create a note when a topic:

- spans multiple files or layers
- contains non-obvious invariants
- is likely to confuse a new contributor or agent
- was already investigated once and should not need ad hoc rediscovery

Candidate topics in this repo (none written yet — see "Current Research
Notes" below):

- the full `DataSource` recursive init/update/query contract and how
  `data_manipulation` composition actually resolves types at compile time
- update-group scheduling and mutex lock ordering in `SyncUpdateGroup`
- per-mode config loading and env-override merging
  (`stats-server/src/config/{json,env,read}`)
- linked-stats gap-filling and hop-limit semantics
  (`stats-server/src/linked_stats*.rs`)
- conditional-start / indexing-status wait logic
  (`stats-server/src/blockscout_waiter.rs`)

## Standard Template

Use this structure for new research files:

```markdown
# <Topic>

## Scope

## Short Answer

## Why This Matters

## Source-of-Truth Files

## Key Types / Tables / Contracts

## Step-by-Step Flow

## Invariants

## Failure Modes / Observability

## Edge Cases / Gotchas

## Change Triggers

## Open Questions
```

Section guidance:

- `Scope`
  - define what is covered and what is intentionally out of scope
- `Short Answer`
  - provide the high-signal takeaway in a few sentences
- `Why This Matters`
  - explain why this topic is operationally or architecturally important
- `Source-of-Truth Files`
  - list the primary files that define behavior; prefer authoritative code paths
    over incidental callers
- `Key Types / Tables / Contracts`
  - name the structs, enums, traits, database tables, API contracts, or config
    models that carry the behavior
- `Step-by-Step Flow`
  - describe the runtime flow in order, from input to persisted or exposed
    result
- `Invariants`
  - capture guarantees, assumptions, and conditions that must remain true
- `Failure Modes / Observability`
  - note how this behavior fails, what symptoms appear, and where to inspect
    logs, metrics, or APIs
- `Edge Cases / Gotchas`
  - capture non-obvious branches, exclusions, and surprising behavior
- `Change Triggers`
  - state when this note must be updated, such as schema changes, new mode
    support, or altered chart-framework contracts
- `Open Questions`
  - record unresolved ambiguities or follow-up topics

Use all sections when they add value, but keep notes concise and concrete.
Small topics may keep some sections brief.

## Current Research Notes

- `interchain-mode-and-filtering.md` — **Interchain Mode: Data Flow and Read
  Filtering.** How `Mode::Interchain` is selected and wired, the 7 counters and
  9 line charts and their update pipeline, the `interchain_primary_id` filtering
  mechanism per chart, the indexer-side schema stats reads, the timespan/range
  machinery, and the parity gap against the interchain-indexer's read-time API
  filtering (`ChainBridgeFilter`).

Add an entry here for every new note. Use the `research-scope` workflow
(`.memory-bank/workflows/research-scope.md`) to scope one before writing it.
