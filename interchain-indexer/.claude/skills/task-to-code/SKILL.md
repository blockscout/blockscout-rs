---
name: task-to-code
description: Apply code changes from an existing task handoff under `tmp/tasks/<task-name>/coding-task-X.md`. Use when Claude receives a task name and coding-task index and must implement that prepared coding task strictly, without inventing missing scope or design details.
disable-model-invocation: true
---

# Task To Code Skill

Follow the workflow defined in @../../../.memory-bank/workflows/task-to-code.md

## Required Guardrails

- Read `../../../tmp/tasks/<task-name>/coding-task-X.md` first.
- Read `../../../tmp/tasks/<task-name>/implementation-plan-X.md` when it exists and the coding task depends on it.
- Treat the coding task as the source of truth for scope, ordering, verification, and acceptance criteria.
- Re-check the current code before editing, but do not invent missing requirements, extra refactors, or side quests.
- If the coding task is unclear, contradictory, or incomplete, ask the human instead of guessing.
- If current code drift makes the coding task unsafe or obsolete, stop and surface the mismatch clearly.
- Run the verification steps required by the coding task when feasible, and report any verification gaps explicitly.

## Minimal Starting Reads

Start with:

- `../../../tmp/tasks/<task-name>/coding-task-X.md`
- `../../../tmp/tasks/<task-name>/implementation-plan-X.md` if present
- `../../../tmp/tasks/<task-name>/task.md` if present
- `../../../.memory-bank/project-context.md`
- `../../../.memory-bank/architecture.md`
- `../../../.memory-bank/gotchas.md`

Then read the specific rules, tests, configs, schemas, and source files needed to implement the task exactly as written.
