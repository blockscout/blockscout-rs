---
name: task-analysis
description: Analyze an input task, issue, or feature request against the existing codebase, propose one or more implementation approaches, align evaluation criteria with a human when tradeoffs exist, and recommend a path with explicit reasoning.
---

# Task Analysis Skill

Use this skill when the user wants a pre-implementation review of a task, issue, feature, or design direction.

## Workflow

Follow the canonical workflow in `../../../.memory-bank/workflows/task-analysis.md`.

## Required Guardrails

- Read the relevant `.memory-bank/` context and source-of-truth code paths before proposing solutions.
- Ground every option in the actual repo structure and current abstractions.
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
