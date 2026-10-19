# Stats

Rust microservice that calculates and serves statistical charts/counters (lines and counters) derived from Blockscout-family data. Runs in one of four mutually-exclusive modes (`Blockscout`, `MultichainAggregator`, `Zetachain`, `Interchain`) depending on which indexer database schema it reads from.

## Stack

- Rust 2024 edition
- Tokio
- PostgreSQL + SeaORM
- Actix-web + Tonic (REST via `actix-prost` route generation + gRPC, same service implementation)

## Build & Test

Run `just` to see the available commands, or check the @justfile.

## Navigation

Start with these files:

- `.memory-bank/project-context.md` — service purpose, modes, crates, runtime components, local workflow
- `.memory-bank/architecture.md` — chart/data-source framework, update groups, runtime wiring
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
- `.memory-bank/architecture.md` for the chart/data-source framework and system flow
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

- **Files:** `config/blockscout_instance/`, `config/multichain/`, `config/interchain/` (each with `charts.json`, `layout.json`, `update_groups.json`)
- **Env vars:** `STATS__<KEY>` (e.g. `STATS__MODE`, `STATS__DB_URL`, `STATS__INDEXER_DB_URL`)
- **Generated env docs:** the env var table in `stats/README.md` is generated from `Settings` by the `env-docs-generation` crate — run `just check-envs` to validate it's in sync, `just generate-envs` to regenerate it

## Key Decisions

See `.memory-bank/adr/README.md` for architectural decision records.

## Known Gotchas

1. **Update-group membership matters** — a chart enabled only as a dependency (not a group member) is never triggered on its own
2. **`just test` includes ignored DB tests, and `justfile` overrides `DATABASE_URL`** — prefer `just test-with-db` for a full run
3. **Chart enable/disable is keyed by what a config entry *serves*** (its `implementation` remap), not its config key name
4. **`enable_all_*` flags silently no-op** if the target chart id isn't in the currently loaded mode's config
5. **The second (CCTX) indexer DB only connects in `Zetachain` mode** — `STATS__SECOND_INDEXER_DB_URL` is ignored otherwise
6. **`IndexerMigrations` is only ever queried for `Blockscout`/`Zetachain` mode** — other modes always see `IndexerMigrations::empty()`

For details see: `.memory-bank/gotchas.md`

## Memory Protocol

When you discover a non-obvious pattern or gotcha, update `.memory-bank/gotchas.md`.
When finishing a reusable investigation, add or update a note in `.memory-bank/research/`.
When making an architectural decision, add an ADR to `.memory-bank/adr/`.
When corrected about a convention, update the relevant file in `.memory-bank/rules/`.
When a new coding rule emerges, update the relevant file in `.memory-bank/rules/` or create a new one if needed.
Never cite a `tmp/` path from a committed file — it is gitignored and resolves to nothing for anyone else; cite code, a commit, or an ADR instead.

## Workflows

Reusable task procedures are in `.memory-bank/workflows/`. These are tool-agnostic —
tool-specific integrations (Cursor `.cursor/skills/`, Claude Code `.claude/skills`, Codex `.codex/skills/`) are thin wrappers.

- `research-scope.md` — scope a research note before writing `.memory-bank/research/`
- `task-analysis.md` — pre-implementation task review and options
- `implementation-plan.md` — turn approved analysis into a coding-ready design
- `task-to-code.md` — execute a prepared coding task handoff
- `solution-review.md` — post-implementation review against the original task
- `pr-description.md` — prepare a reviewer-facing PR description

## graphify

This project has a knowledge graph at `graphify-out/` with god nodes, community structure, and cross-file relationships. **It is local-only**: `graphify-out/` is gitignored (`stats/.gitignore`) and must never be committed — it's a local cache, not a project artifact.

When the user types `/graphify`, use the installed graphify skill or instructions before doing anything else.

Rules:
- For codebase questions, first run `graphify query "<question>"` when `graphify-out/graph.json` exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than `GRAPH_REPORT.md` or raw grep output.
- This applies to every spawned subagent that explores code, not just to you. Include the appropriate graphify command in the subagent prompt, before it reads source.
- If `graphify-out/graph.json` does not exist, bootstrap it with a full build: `graphify .` from `stats/`.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost) — this costs nothing in diff since the output is gitignored, but skipping it leaves local navigation stale.
- Rebuild fully (`graphify .`) after a large refactor, a long gap since the last build, or a branch switch that moved many files.
- If `graphify-out/wiki/index.md` exists, use it for broad navigation instead of raw source browsing.
- Read `graphify-out/GRAPH_REPORT.md` only for broad architecture review or when query/path/explain do not surface enough context.
- A missing local graph that cannot be built in the current environment is never a blocker — fall back to Read/Grep/Glob and say so explicitly.
