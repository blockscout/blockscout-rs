# Task To Code Workflow

Implement a prepared coding task from `tmp/tasks/<task-name>/coding-task-X.md` without re-scoping the work. The goal is to execute an already-defined handoff faithfully against the current codebase, validate the change, and stop for human clarification whenever the task leaves material ambiguity.

**Use for:**

- applying code changes from an existing `coding-task-X.md`
- executing a prepared implementation handoff after analysis and planning are already done
- making the concrete code, test, config, and migration changes described by the coding task

**Do NOT use for:**

- inventing a design when no coding task exists
- choosing between multiple solution options
- filling in missing product or architectural decisions on your own
- broad refactors beyond the explicit task scope

## Required Inputs

- task name
- coding task index `X`
- existing task folder `tmp/tasks/<task-name>/`
- `coding-task-X.md`

Read when present and relevant:

- `implementation-plan-X.md`
- `task.md`
- `solution_X.md`
- `.memory-bank/` rules, gotchas, research notes, and ADRs for the affected area
- current source-of-truth code, tests, configs, migrations, and APIs touched by the task

If `coding-task-X.md` is missing, contradictory, or too vague to implement safely, do not invent the missing details. Ask the human for clarification.

## Default Sources To Read First

Start with:

- `tmp/tasks/<task-name>/coding-task-X.md`
- `tmp/tasks/<task-name>/implementation-plan-X.md` if present
- `tmp/tasks/<task-name>/task.md` if present
- `.memory-bank/project-context.md`
- `.memory-bank/architecture.md`
- `.memory-bank/gotchas.md`
- relevant `.memory-bank/rules/*.md`

Then read only the current code and tests needed to implement the specified work.

## Execution Rules

- Treat `coding-task-X.md` as the source of truth for scope and acceptance criteria.
- Re-check the current code before editing, but do not expand the task beyond what the handoff requires.
- Reuse existing abstractions and patterns unless the coding task explicitly says otherwise.
- Do not silently fix adjacent issues, clean up unrelated code, or add speculative improvements.
- If a required implementation detail is unclear, conflicting, or missing, stop and ask the human.
- If the current code has drifted enough that the handoff no longer fits, explain the mismatch and ask whether to update the task artifacts first.

## Steps

### 1. Load The Handoff

Confirm:

- the exact goal
- the affected files and components
- the required verification steps
- the acceptance criteria
- any explicit blockers or open questions already captured in the task

### 2. Rebuild Only The Needed Code Context

Read the smallest set of source files, tests, schemas, configs, and generated interfaces needed to apply the coding task safely. Verify that the current implementation still matches the handoff assumptions.

### 3. Implement Exactly The Requested Work

Make the code, test, config, schema, migration, or documentation changes required by the coding task. Keep edits aligned to the specified sequence when ordering matters.

If the task says to update generated artifacts, migrations, or protobuf outputs, perform that work as part of the implementation.

### 4. Validate Against The Task

Run the verification commands named in the coding task when feasible. If a required check cannot be run, say exactly why.

Confirm the result against the stated acceptance criteria instead of broad subjective judgment.

### 5. Report Outcome Precisely

Summarize:

- what was implemented
- which verification steps ran and their outcomes
- whether the acceptance criteria appear satisfied
- any unresolved blockers, ambiguity, or drift that still needs human input

## Completion Standard

The task is complete only when:

- the requested code changes are implemented
- required verification has run or any gap is explicitly explained
- the result matches the acceptance criteria in `coding-task-X.md`

If those conditions are not met, report the exact blocker instead of guessing.
