# Research Notes

## Purpose

This directory stores durable, question-driven codebase research. A research
note should explain one non-trivial runtime behavior or architectural flow well
enough that future agents do not need to rediscover it from scratch.

Research notes are deeper than overview docs and more stable than temporary
notes in `tmp/`.

## When to Create a Research Note

Create a note when a topic:

- spans multiple files or layers
- contains non-obvious invariants
- is likely to confuse a new contributor or agent
- was already investigated once and should not need ad hoc rediscovery

Examples in this repo:

- message lifecycle
- Avalanche bridge filtering
- blockchain ID resolution
- buffer persistence and checkpoints
- stats projection

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

Prefer file references and concrete runtime flows over generic explanations.

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
    logs, metrics, status tables, or APIs
- `Edge Cases / Gotchas`
  - capture non-obvious branches, exclusions, and surprising behavior
- `Change Triggers`
  - state when this note must be updated, such as schema changes, new protocol
    support, altered finality rules, or config model changes
- `Open Questions`
  - record unresolved ambiguities or follow-up topics

Use all sections when they add value, but keep notes concise and concrete.
Small topics may keep some sections brief.

## Current Research Notes

- `stats-projection.md` — how finalized messages are projected into
  `stats_messages` and related stats tables
- `db-schema-and-layer.md` - overview of the service database subsystem
- `token-info-service.md` - `TokenInfoService` usage sites, request-time
  lookups, async enrichment, cache semantics, and downstream stats enrichment
