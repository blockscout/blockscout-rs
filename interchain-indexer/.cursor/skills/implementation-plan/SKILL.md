---
name: implementation-plan
description: Turn an approved task-analysis result into a shared technical design and coding-ready implementation handoff. Use when Cursor has an existing task folder under `tmp/tasks/`, plus a selected solution and related codebase research, and needs to produce `implementation-plan.md` and `coding-task.md` before writing code.
---

# Implementation Plan Skill

Use this skill when the design direction is already chosen and the next missing artifact is a coding-ready plan.

## Workflow

Follow the canonical workflow in `../../../.memory-bank/workflows/implementation-plan.md`.

## Required Guardrails

- Read the existing task folder artifacts first, especially `task.md` and the selected solution.
- Re-check the current code, tests, configs, and schema paths instead of trusting earlier analysis blindly.
- Treat this as a handoff-preparation step, not a fresh solution-comparison step.
- If the chosen direction is ambiguous or stale enough to change the recommendation, stop and route the task back to analysis.
- Write the outputs into the existing `tmp/tasks/<task-name>/` folder as `implementation-plan.md` and `coding-task.md`.
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
