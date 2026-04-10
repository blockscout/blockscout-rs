# Interchain Indexer

Rust microservice indexing cross-chain messages and token transfers. Currently supports Avalanche Teleporter (ICM) and ICTT protocols.

## Stack

- Rust 2021
- Tokio
- PostgreSQL + SeaORM
- Actix-web + Tonic
- Alloy

## Build & Test

Run `just` to see the available commands, or check the @justfile.

## Navigation

Start with these files:

- `.memory-bank/project-context.md` — service purpose, scope, crates, runtime components, local workflow
- `.memory-bank/codebase-review.md` — strengths, risks, and documentation priorities
- `.memory-bank/architecture.md` — high-level data flow and core abstractions
- `.memory-bank/exploration-map.md` — where to start for specific codebase questions
- `.memory-bank/glossary.md` — repo-specific terminology
- `.memory-bank/gotchas.md` — non-obvious traps and operational edge cases
- `.memory-bank/research/README.md` — durable deep-dive investigations
- `.memory-bank/rules/` — coding conventions
- `.memory-bank/workflows/` — reusable task procedures
- `.memory-bank/adr/README.md` — architectural decision records

## Architecture

Start with:

- `.memory-bank/project-context.md` for crate responsibilities and runtime components
- `.memory-bank/architecture.md` for system flow and core abstractions
- `.memory-bank/exploration-map.md` for code entrypoints by question

## Conventions

Use `.memory-bank/rules/` as the canonical source for coding conventions.

Start with:

- `rust-style.md`
- `error-handling.md`
- `async-patterns.md`
- `database.md`
- `testing.md`

## Configuration

- **Files:** `config/avalanche/chains.json`, `config/avalanche/bridges.json`
- **Env vars:** `INTERCHAIN_INDEXER__<SECTION>__<KEY>`

## Key Decisions

See `.memory-bank/adr/README.md` for architectural decision records.

## Known Gotchas

1. **Message finality is complex** — Requires execution success AND ICTT completion
2. **Unconfigured chains filtered** — Events to/from chains not in bridge config are skipped (trace-logged)
3. **Config typos fail hard** — `deny_unknown_fields` rejects typos
4. **Entity regeneration overwrites codegen/** — Put customizations in manual/
5. **PostgreSQL bind limit** — Use batched operations for large inserts

For details see: `.memory-bank/gotchas.md`

## Memory Protocol

When you discover a non-obvious pattern or gotcha, update `.memory-bank/gotchas.md`.
When finishing a reusable investigation, add or update a note in `.memory-bank/research/`.
When making an architectural decision, add an ADR to `.memory-bank/adr/`.
When corrected about a convention, update the relevant file in `.memory-bank/rules/`.
When a new coding rule emerges, update the relevant file in `.memory-bank/rules/` or create a new one if needed.

## Workflows

Reusable task procedures are in `.memory-bank/workflows/`. These are tool-agnostic —
tool-specific integrations (Cursor `.cursor/skills/`, Claude Code `.claude/skills`, Codex `.codex/skills/`) are thin wrappers.

- `gh-issue-bug.md` — draft a GitHub bug report
- `gh-issue-improvement.md` — draft a GitHub enhancement proposal
- `gh-issue-publish.md` — publish a drafted issue via the `gh` CLI
- `task-analysis.md` — pre-implementation task review and options
- `implementation-plan.md` — turn approved analysis into a coding-ready design
- `task-to-code.md` — execute a prepared coding task handoff
- `solution-review.md` — post-implementation review against the original task
- `pr-description.md` — prepare a reviewer-facing PR description
- `research-scope.md` — scope a research note before writing `.memory-bank/research/`
