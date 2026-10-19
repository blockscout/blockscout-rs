# Task Analysis Workflow

Analyze a task before coding, identify viable approaches, make tradeoffs explicit, and persist the analysis under `tmp/tasks/<task-name>/`.

## Use for

- non-trivial features, enhancements, refactors, and design choices
- requests with hidden constraints or multiple credible implementation paths

Do not use for durable research, post-implementation review, or an obvious low-risk edit.

## Inputs and Artifacts

Use the request, constraints, relevant memory-bank context, and current source-of-truth code/tests. Create:

- `task.md` — framing, constraints, context, criteria, questions, status
- `solution_1.md` … `solution_N.md` — one file per serious option
- `solutions.md` — comparison and recommendation, only when `N >= 2`

`<task-name>` is a concise lowercase dash-separated slug.

## Starting Context

Read the smallest relevant set from `.memory-bank/README.md`, `projectbrief.md`, `sync-architecture.md`, `operation-lifecycle.md`, `api-surface.md`, `gotchas-and-edge-cases.md`, and related research notes. Then inspect the affected code, tests, configuration, migrations, and contracts.

## Process

1. Restate the target behavior, success criteria, constraints, non-goals, and unknowns. Ask concise questions if material ambiguity remains.
2. Identify reusable abstractions, runtime/persistence boundaries, invariants, and operational or compatibility constraints. Record this in `task.md`.
3. For every serious option, document the core idea, affected areas, benefits, costs/risks, poor-fit situations, and validation needs in `solution_N.md`. State when only one realistic option exists.
4. When tradeoffs exist, align evaluation criteria with the human before choosing: only use applicable criteria such as complexity, risk, compatibility, testability, observability, migration cost, extensibility, and performance.
5. Compare multiple options in `solutions.md`; make missing evidence explicit. Recommend a path with rationale, residual risks, validation points, and conditions that would change the recommendation.
6. End as `ready for implementation`, `blocked on clarification`, `blocked on additional codebase research`, or `blocked on product or architectural decision`. Propose durable research only when reusable system knowledge was uncovered.

## Templates

`task.md`: Task; Success Criteria; Constraints And Non-Goals; Codebase Context; Risks And Invariants; Evaluation Criteria; Open Questions; Status.

`solution_N.md`: Core Idea; Affected Areas; Benefits; Costs And Risks; Poor Fit When; Validation Notes. For one option, append Recommendation.

`solutions.md`: Criteria; Comparison; Recommendation; Missing Evidence.
