# Task Analysis Workflow

Review an input task, issue, feature request, or implementation idea before writing code. The goal is to understand the problem in the context of the existing codebase, identify viable solution approaches, agree on evaluation criteria with a human when tradeoffs exist, and recommend a path with explicit reasoning. Persist the analysis under `tmp/tasks/<task-name>/`.

**Use for:**

- implementation planning for a feature or enhancement
- evaluating multiple design options for a task
- turning a loosely defined request into a concrete technical direction
- surfacing hidden constraints before coding

**Do NOT use for:**

- documenting a durable codebase behavior for future reuse
- post-implementation review of a completed change
- trivial tasks where there is effectively one obvious low-risk path

## Required Inputs

- the task, issue, feature request, or implementation goal
- any stated constraints, deadlines, or non-goals
- relevant existing `.memory-bank/` research and architecture context

## Output Files

Create a task folder for each analysis:

```text
tmp/tasks/<task-name>/
```

`<task-name>` should be a short lowercase slug with dashes that identifies the task.

Required files:

- `task.md` — shared task framing, constraints, codebase context, evaluation criteria, and current status
- `solution_1.md` .. `solution_N.md` — one file per serious solution option

Conditional file:

- `solutions.md` — create only when there are 2 or more solution files and compare the options there

## Default Sources to Read First

Read the smallest useful set of canonical repo documents first:

- `.memory-bank/project-context.md`
- `.memory-bank/architecture.md`
- `.memory-bank/exploration-map.md`
- `.memory-bank/gotchas.md`
- relevant `.memory-bank/research/*.md` notes
- relevant `.memory-bank/adr/*.md` decisions
- relevant `.memory-bank/rules/*.md` conventions

Then inspect the concrete source-of-truth code paths and tests related to the task.

## Steps

### 1. Clarify the Task

Restate the request in implementation terms:

- target behavior or outcome
- explicit success criteria
- constraints and non-goals
- unknowns that may change the design

If the task statement is underspecified, ask concise clarification questions.

### 2. Gather Relevant Codebase Context

Identify:

- existing abstractions that already solve part of the problem
- runtime and persistence boundaries the task must respect
- related research notes and ADRs
- hidden constraints from configuration, schema, protocol support, or observability

### 3. Produce a Problem Framing Summary

Summarize:

- what the system does today
- where the change would fit
- what invariants must be preserved
- what risks matter most for this task

This summary should be concrete enough that a human can correct wrong assumptions early.

Record this shared context in `task.md`.

### 4. Generate Solution Options

Propose one or more viable approaches.

For each option, describe:

- the core idea
- the main affected layers or components
- expected benefits
- expected costs or risks
- situations where the option is a poor fit

If only one realistic option exists, state that directly and explain why alternatives are not serious contenders.

Write each option to its own `solution_N.md` file. Each solution file should include:

- title
- core idea
- affected layers or components
- expected benefits
- expected costs or risks
- poor-fit situations

### 5. Align on Evaluation Criteria with the Human

When tradeoffs exist, do not pick a winner silently.

Present candidate evaluation criteria and ask the human to confirm or adjust them. Common criteria include:

- implementation complexity
- change risk
- backward compatibility
- testability
- operational visibility
- migration cost
- extensibility
- performance

Use only criteria that actually matter for the task.

### 6. Compare the Options

Compare the options against the agreed criteria.

The comparison should make tradeoffs explicit rather than pretending there is a universally best choice.

If more investigation is needed to compare fairly, state what evidence is missing.

When there are multiple serious options, write the comparison to `solutions.md`.

### 7. Recommend a Path

Recommend one option when possible.

The recommendation should include:

- why it best fits the agreed criteria
- what risks remain
- what should be validated during implementation
- what would change the recommendation

Put the recommendation in `solutions.md` when that file exists. If there is only one realistic option, put the recommendation at the end of `solution_1.md` and summarize the status in `task.md`.

### 8. Hand Off to Implementation or Further Research

Conclude with one of these outcomes:

- ready for implementation
- blocked on clarification
- blocked on additional codebase research
- blocked on product or architectural decision

If the investigation uncovered a reusable multi-file behavior that future agents would likely rediscover, propose creating or updating a `.memory-bank/research/` note.

## Output Contract

The workflow output on disk should contain:

- `task.md` with:
  - a concise problem framing
  - the relevant codebase context
  - explicit success criteria
  - constraints, non-goals, and open questions
  - explicit evaluation criteria
  - current outcome status: `ready for implementation`, `blocked on clarification`, `blocked on additional codebase research`, or `blocked on product or architectural decision`
- `solution_1.md` .. `solution_N.md` with one or more solution options
- `solutions.md` with comparison results and recommendation when `N >= 2`

Use this structure:

### `task.md`

```markdown
# <Task Title>

## Task

[Restated implementation goal]

## Success Criteria

- ...

## Constraints And Non-Goals

- ...

## Codebase Context

- ...

## Risks And Invariants

- ...

## Evaluation Criteria

- ...

## Open Questions

- ...

## Status

[ready for implementation | blocked on clarification | blocked on additional codebase research | blocked on product or architectural decision]
```

### `solution_N.md`

```markdown
# Solution N: <Short Title>

## Core Idea

[Approach summary]

## Affected Areas

- ...

## Benefits

- ...

## Costs And Risks

- ...

## Poor Fit When

- ...

## Validation Notes

- ...
```

When there is only one realistic option, append:

```markdown
## Recommendation

[Why this is the recommended path, remaining risks, and what would change the recommendation]
```

### `solutions.md`

Create only when there are multiple serious options.

```markdown
# Solution Comparison

## Criteria

- ...

## Comparison

| Option | ... |
| --- | --- |

## Recommendation

[Recommended option, rationale, remaining risks, and validation points]

## Missing Evidence

- ...
```

## Quality Bar

The review should:

- anchor proposals in actual codebase structure, not generic architecture advice
- preserve known invariants and conventions
- separate facts, assumptions, and recommendations
- make tradeoffs legible to a human collaborator
- stay implementation-oriented without jumping straight into code unless asked

## Stop Conditions

Stop and ask the human before recommending a path if:

- the evaluation criteria are still disputed
- a major constraint is missing
- the task spans multiple architectural directions with materially different product implications
- the current codebase understanding is too weak to compare options honestly
