---
name: solution-review
description: Review already-applied code changes against the original task statement or task folder artifacts under `tmp/tasks/<task-name>/`. Use when implementation work is done and Claude must verify scope coverage, detect mismatches or regressions, assess verification gaps, and write a review summary before handoff or merge.
disable-model-invocation: true
---

# Solution Review Skill

Follow the workflow defined in @../../../.memory-bank/workflows/solution-review.md

## Required Guardrails

- Review against the original task source of truth first: `task.md`, `solution_X.md`, `implementation-plan-X.md`, `coding-task-X.md`, or the user-provided task statement.
- Inspect the actual applied change, not just the final code shape. Read the relevant diff, changed files, and nearby tests.
- Treat this as a post-implementation review step, not a chance to silently finish missing work.
- Separate confirmed findings, uncertainties, and verification gaps.
- Prefer concrete evidence with file paths, acceptance-criteria mapping, and commands that were or were not run.
- Write the review artifact into the existing task folder as `review.md` when the task lives under `tmp/tasks/<task-name>/`.

## Minimal Starting Reads

Start with:

- `../../../.memory-bank/project-context.md`
- `../../../.memory-bank/architecture.md`
- `../../../.memory-bank/gotchas.md`
- `../../../tmp/tasks/<task-name>/task.md` if present
- `../../../tmp/tasks/<task-name>/solution_*.md` if present
- `../../../tmp/tasks/<task-name>/implementation-plan*.md` if present
- `../../../tmp/tasks/<task-name>/coding-task*.md` if present

Then read the current diff, changed files, tests, configs, and any user-provided task statement needed to judge whether the implementation is complete and correct.
