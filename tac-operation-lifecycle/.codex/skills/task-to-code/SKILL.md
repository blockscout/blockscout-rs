---
name: task-to-code
description: Implement a prepared `tmp/tasks/<task-name>/coding-task-X.md` exactly, validating the result without inventing missing scope or design decisions.
---

# Task To Code

Use when a prepared coding task, task name, and index are available.

Follow `../../../.memory-bank/workflows/task-to-code.md`.

Read the coding task first and treat it as the source of truth for scope, ordering, verification, and acceptance criteria. Re-check the current code but do not add unrelated refactors. Stop for human clarification if the handoff is vague, contradictory, or invalidated by code drift; report verification gaps explicitly.
