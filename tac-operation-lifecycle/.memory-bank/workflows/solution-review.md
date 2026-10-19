# Solution Review Workflow

Review already-applied changes against the original task and task artifacts. Do not silently complete missing implementation.

## Inputs and Output

Use the strongest intended-work source: user request, `task.md`, solutions, plan, coding task, and the actual diff, changed files, tests, configuration, migrations, and API surface. When `tmp/tasks/<task-name>/` exists, write `review.md` there; otherwise return findings without inventing a task folder.

## Starting Context

Read the relevant task artifacts plus the smallest applicable set from `.memory-bank/README.md`, `projectbrief.md`, `sync-architecture.md`, `operation-lifecycle.md`, `api-surface.md`, `gotchas-and-edge-cases.md`, and research notes. Then inspect the implementation surface and adjacent effects.

## Process

1. Reconstruct expected goal, success criteria, non-goals, selected design, and invariants. Artifact disagreement is a review blocker.
2. Inspect the actual diff, touched boundaries, tests, and relevant compatibility/persistence/operational surfaces.
3. Map each requirement to direct code or test evidence: implemented, partial, missing, or out of scope.
4. Assess correctness and regression risk. Classify unsupported concerns as verification gaps or questions, not confirmed defects.
5. Write findings by severity with evidence, impact, and recommendation. Finish as `accepted`, `accepted with follow-ups`, `changes required`, or `blocked on ambiguous task`.

## Template

`review.md`: Scope; Expected Outcome; Coverage Summary; Findings; Verification; Recommendation. Include the reviewed snapshot and concrete file/criterion evidence. If no findings exist, say so and retain residual verification risk.
