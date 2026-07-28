# Task To Code Workflow

Implement a prepared `tmp/tasks/<task-name>/coding-task-X.md` faithfully against the current codebase, without re-scoping it.

## Preconditions

Read `coding-task-X.md` first, then the matching plan and task artifacts when present. Also read the relevant memory-bank notes and the current source/tests needed for the specified work. If the handoff is missing, contradictory, vague, or invalidated by code drift, stop and ask the human.

## Rules

- Treat the coding task as the source of truth for scope, order, verification, and acceptance criteria.
- Re-check current code before editing, but do not add adjacent fixes, speculative improvements, or unrelated refactors.
- Reuse existing patterns unless the handoff directs otherwise.
- Make the requested code, test, schema, migration, configuration, or documentation changes only.
- Run `just format` after the final edit. Then run the feasible verification named in the handoff; prefer `just test`, `just test-with-db`, `just check`, and `just check-envs` where applicable.

## Process

1. Confirm exact goal, components, verification, acceptance criteria, and existing blockers.
2. Read the smallest current implementation surface that validates the handoff assumptions.
3. Implement the requested work in the required sequence.
4. Format and validate against stated acceptance criteria. Report any unavailable check with its exact reason.
5. Report implemented scope, formatting result, verification results, acceptance-criteria status, and unresolved ambiguity or drift.

The task is complete only when requested changes are applied, formatting has run or its blocker is stated, and required verification either passed or its gap is explicit.
