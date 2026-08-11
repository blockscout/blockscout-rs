# Implementation Plan Workflow

Turn an approved task-analysis result into a current technical design and a coding-ready handoff in `tmp/tasks/<task-name>/`.

## Preconditions

Require `task.md`, a selected `solution_X.md`, relevant memory-bank context, and current code/tests. If the chosen direction is unclear, do not invent it; align with the human or mark the task blocked.

## Starting Context

Read task artifacts first, then the relevant documents from `.memory-bank/README.md`, `projectbrief.md`, `sync-architecture.md`, `operation-lifecycle.md`, `api-surface.md`, `gotchas-and-edge-cases.md`, and research notes. Re-check the concrete code, tests, configuration, schema, API, and migration surfaces.

## Process

1. Confirm the selected direction, task status, and unresolved questions. If code drift changes the core choice, route back to task analysis.
2. Rebuild current context: modules, crates, entrypoints, data boundaries, reusable abstractions, invariants, tests, and observability.
3. Write `implementation-plan-X.md` describing the chosen design, responsibilities, flow changes, persistence/API/config/schema implications, invariants, errors and edge cases, validation, migration, rollout, and open questions.
4. Stop for explicit human review. Update the plan if requested; do not create the coding handoff until it is confirmed.
5. After confirmation, write a self-sufficient `coding-task-X.md`: goal, assumptions, file/component map, ordered work, tests and commands, acceptance criteria, risks, and blockers. It must be understandable without opening earlier artifacts.
6. Use `ready for coding`, `blocked on clarification`, `blocked on additional codebase research`, or `blocked on product or architectural decision` in both artifacts as relevant.

## Validation Policy

Prefer repo-native commands: `just format` after edits, `just test` for ordinary tests, `just test-with-db` for tests that need its temporary database, `just check` for static validation, and `just check-envs` when environment documentation or configuration surface changes. Mention bare Cargo commands only if no repository wrapper applies.

## Templates

`implementation-plan-X.md`: Summary; Inputs; Current Codebase Fit; Design (Responsibilities, Flow Changes, Data/API/Config/Schema); Risks And Invariants; Validation; Rollout Notes; Open Questions; Status.

`coding-task-X.md`: Goal; Preconditions And Assumptions; Files And Components; Ordered Work Items; Verification; Acceptance Criteria; Risks And Watch-Outs; Open Questions Or Blockers; Status.
