# Implementation Plan Workflow

Turn an approved task analysis into a coding-ready implementation design and handoff. The goal is to take the chosen direction from `tmp/tasks/<task-name>/`, reconcile it with the current codebase, and produce the concrete design and execution details another agent needs to implement safely.

**Use for:**

- converting an approved `task-analysis` result into a build-ready design
- preparing a concrete coding task after relevant research is already available
- breaking a non-trivial feature or refactor into explicit implementation steps
- surfacing integration, validation, migration, and rollout details before coding starts

**Do NOT use for:**

- choosing between multiple solution options before a direction is selected
- durable research notes that belong in `.memory-bank/research/`
- trivial changes where the implementation is obvious from the user request
- post-implementation review of a finished change

## Required Inputs

- an existing task folder `tmp/tasks/<task-name>/`
- `task.md` from `task-analysis`
- the selected solution by number (`solution_1.md`, `solution_2.md`, etc)
- relevant `.memory-bank/` research, ADRs, rules, and gotchas
- the current source-of-truth code and tests for the affected area

If the task analysis does not clearly identify the chosen direction, do not invent one. Either align with the human first or mark the plan as blocked on product or architectural decision.

## Output Files

Write the implementation artifacts into the existing task folder:

```text
tmp/tasks/<task-name>/
```

Required files:

- `implementation-plan.md` — shared technical design for the approved approach
- `coding-task.md` — concrete coding handoff for the implementation agent

## Default Sources to Read First

Start with the smallest useful set:

- `tmp/tasks/<task-name>/task.md`
- `tmp/tasks/<task-name>/solution_X.md` (where X - number of input solution)
- `.memory-bank/project-context.md`
- `.memory-bank/architecture.md`
- `.memory-bank/exploration-map.md`
- `.memory-bank/gotchas.md`
- relevant `.memory-bank/research/*.md`
- relevant `.memory-bank/adr/*.md`
- relevant `.memory-bank/rules/*.md`

Then read the current code, tests, migrations, configs, and API surfaces that the chosen approach would touch.

## Steps

### 1. Confirm the Chosen Direction

Establish:

- what the approved implementation direction is
- whether the task status is actually ready for implementation
- which open questions remain from the earlier analysis

If the analysis is stale relative to the current code, note the drift explicitly and adjust the plan to current reality. If the drift changes the core design choice, stop and send the task back through analysis.

### 2. Rebuild the Current Codebase Context

Verify the actual implementation surface:

- concrete modules, crates, entrypoints, and data boundaries involved
- existing abstractions to reuse instead of bypassing
- schema, API, config, indexing, or runtime invariants that must hold
- nearby tests and observability hooks that should be extended

Do not rely only on prior notes. Re-check the current code.

### 3. Write the Shared Technical Design

Produce `implementation-plan-X.md` (X - source solution number) describing:

- the chosen design in plain implementation terms
- affected layers and their responsibilities
- end-to-end data flow and control flow changes
- persistence, API, config, and runtime implications
- invariants that must remain true
- main risks, edge cases, and failure handling expectations
- validation strategy

This file should explain *what* will be built and *why this design fits the current system*.

### 4. Translate the Design Into Concrete Work

Prepare the implementation breakdown:

- exact files, modules, schemas, tests, and configs likely to change
- required migrations, backfills, or rollout ordering
- dependencies between subtasks
- validation commands and artifacts to update
- anything the implementation agent must inspect before editing

Prefer actionable statements over abstract intentions.

### 5. Write the Coding Handoff

Produce `coding-task-X.md` (X - source solution number) for the next agent. It should be specific enough that the coding agent can start without repeating the full design investigation.

Include:

- the implementation goal
- prerequisites and assumptions
- concrete work items in a sensible execution order
- file and component map
- required tests and verification commands
- acceptance criteria
- known risks and watch-outs
- any remaining blockers or questions

### 6. Decide the Outcome Status

End with one of these statuses inside both artifacts when relevant:

- `ready for coding`
- `blocked on clarification`
- `blocked on additional codebase research`
- `blocked on product or architectural decision`

If the workflow exposes reusable multi-file behavior that future agents would likely rediscover, propose or create a `.memory-bank/research/` note.

## Output Contract

The workflow output on disk should contain:

- `implementation-plan-X.md` with:
  - concise summary of the chosen approach
  - current codebase fit and affected components
  - technical design details across layers
  - invariants, risks, and edge cases
  - validation, migration, observability, and rollout notes where relevant
  - final status
- `coding-task-X.md` with:
  - implementation goal
  - assumptions and prerequisites
  - concrete ordered work items
  - file/component map
  - verification plan
  - acceptance criteria
  - blockers or open questions

Use this structure:

### `implementation-plan.md`

```markdown
# <Task Title> Implementation Plan

## Summary

[Chosen approach and why it fits]

## Inputs

- task analysis: ...
- selected solution: ...
- relevant research / ADRs / rules: ...

## Current Codebase Fit

- ...

## Design

### Responsibilities

- ...

### Flow Changes

- ...

### Data / API / Config / Schema Implications

- ...

## Risks And Invariants

- ...

## Validation

- ...

## Rollout Notes

- ...

## Open Questions

- ...

## Status

[ready for coding | blocked on clarification | blocked on additional codebase research | blocked on product or architectural decision]
```

### `coding-task.md`

```markdown
# <Task Title> Coding Task

## Goal

[Concrete implementation target]

## Preconditions And Assumptions

- ...

## Files And Components

- ...

## Ordered Work Items

1. ...
2. ...
3. ...

## Verification

- ...

## Acceptance Criteria

- ...

## Risks And Watch-Outs

- ...

## Open Questions Or Blockers

- ...

## Status

[ready for coding | blocked on clarification | blocked on additional codebase research | blocked on product or architectural decision]
```
