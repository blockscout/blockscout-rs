---
name: task-analysis
description: Analyze an input task, issue, or feature request against the existing codebase, persist the task framing under `tmp/tasks/<task-name>/task.md`, write one or more `solution_N.md` option files, align evaluation criteria with a human when tradeoffs exist, and recommend a path with explicit reasoning.
---

# Task Analysis Skill

Use this skill when the user wants a pre-implementation review of a task, issue, feature, or design direction.

## Workflow

Follow the canonical workflow in `../../../.memory-bank/workflows/task-analysis.md`.

## Required Guardrails

- Read the relevant `.memory-bank/` context and source-of-truth code paths before proposing solutions.
- Ground every option in the actual repo structure and current abstractions.
- Persist each analysis under `tmp/tasks/<task-name>/` using `task.md`, `solution_N.md`, and `solutions.md` when multiple options exist.
- If multiple viable options exist, explicitly align on evaluation criteria with the human before selecting a recommendation.
- If only one realistic option exists, say so and explain why.
- Separate facts, assumptions, and recommendations.

## Minimal Starting Reads

Start with:

- `../../../.memory-bank/project-context.md`
- `../../../.memory-bank/architecture.md`
- `../../../.memory-bank/exploration-map.md`
- `../../../.memory-bank/gotchas.md`

Then read the specific research notes, ADRs, rules, tests, and code paths relevant to the task under review.
