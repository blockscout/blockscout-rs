---
name: implementation-plan
description: Turn an approved task-analysis result into a shared technical design and coding-ready implementation handoff. Use when Codex has an existing task folder under `tmp/tasks/`, plus a selected solution and related codebase research, and needs to produce `implementation-plan-X.md` and `coding-task-X.md` before writing code.
---

# Implementation Plan

Use this skill when the design direction is already chosen and the next missing artifact is a coding-ready plan.

## Workflow

Follow the canonical workflow in `../../../.memory-bank/workflows/implementation-plan.md`.

## Required Guardrails

- Read the existing task folder artifacts first, especially `task.md` and the selected solution.
- Re-check the current code, tests, configs, and schema paths instead of trusting earlier analysis blindly.
- Treat this as a handoff-preparation step, not a fresh solution-comparison step.
- If the chosen direction is ambiguous or stale enough to change the recommendation, stop and route the task back to analysis.
- Write the outputs into the existing `tmp/tasks/<task-name>/` folder as `implementation-plan-X.md` and `coding-task-X.md`.
- After drafting `implementation-plan-X.md`, stop for user review and do not write `coding-task-X.md` until the user confirms the plan.
- If the user requests changes to the plan, update `implementation-plan-X.md` and repeat the review loop.
- Keep the implementation plan focused on shared technical design; keep the coding task focused on actionable execution details.

## Minimal Starting Reads

Start with:

- `../../../.memory-bank/project-context.md`
- `../../../.memory-bank/architecture.md`
- `../../../.memory-bank/exploration-map.md`
- `../../../.memory-bank/gotchas.md`
- `../../../tmp/tasks/<task-name>/task.md`
- `../../../tmp/tasks/<task-name>/solution_*.md`
- `../../../tmp/tasks/<task-name>/solutions.md` if present

Then read the specific research notes, ADRs, rules, tests, and source files needed to make the design concrete and current.
