---
name: task-analysis
description: Review a task, issue, or feature request before coding; persist the task framing under `tmp/tasks/<task-name>/task.md`, write one or more `solution_N.md` option files, compare them in `solutions.md` when needed, and recommend a path with explicit reasoning
disable-model-invocation: true
---

# Task Analysis Skill

Follow the workflow defined in @../../../.memory-bank/workflows/task-analysis.md

## Required Guardrails

- Read the relevant `.memory-bank/` context and source-of-truth code paths before proposing solutions.
- Ground every option in the actual repo structure and current abstractions.
- Persist each analysis under `tmp/tasks/<task-name>/` using `task.md`, `solution_N.md`, and `solutions.md` when multiple options exist.
- If multiple viable options exist, explicitly align on evaluation criteria with the human before selecting a recommendation.
- If only one realistic option exists, say so and explain why.
- Separate facts, assumptions, and recommendations.
