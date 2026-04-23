---
name: implementation-plan
description: Turn an approved task-analysis result into a shared technical design and coding-ready implementation handoff; use when Claude has an existing task folder under `tmp/tasks/`, plus a selected solution and related codebase research, and needs to produce `implementation-plan.md` and `coding-task.md` before writing code
disable-model-invocation: true
---

# Implementation Plan Skill

Follow the workflow defined in @../../../.memory-bank/workflows/implementation-plan.md

## Required Guardrails

- Read the existing task folder artifacts first, especially `task.md` and the selected solution.
- Re-check the current code, tests, configs, and schema paths instead of trusting earlier analysis blindly.
- Treat this as a handoff-preparation step, not a fresh solution-comparison step.
- If the chosen direction is ambiguous or stale enough to change the recommendation, stop and route the task back to analysis.
- Write the outputs into the existing `tmp/tasks/<task-name>/` folder as `implementation-plan.md` and `coding-task.md`.
- Keep the implementation plan focused on shared technical design; keep the coding task focused on actionable execution details.
