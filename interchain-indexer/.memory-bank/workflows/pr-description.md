# PR Description Workflow

Prepare a reviewer-facing pull request description from the task folder and the current implementation snapshot. The goal is to explain the intent, implemented scope, notable interfaces or operational changes, and verification status in a format that can be pasted into a GitHub pull request or stored with the task.

**Use for:**

- drafting a new PR description from `tmp/tasks/<task-name>/`
- refreshing a PR description after the implementation changed
- preparing reviewer context before opening or updating a pull request
- summarizing applied work with explicit API, ENV, config, and verification notes

**Do NOT use for:**

- choosing the implementation direction before coding
- post-implementation correctness review instead of `solution-review`
- writing a GitHub issue proposal instead of a PR description
- inventing behavior that is not present in the task artifacts or implementation snapshot

## Required Inputs

Use the strongest available sources:

- `tmp/tasks/<task-name>/task.md`
- `solution_*.md`, `solutions.md`, `implementation-plan*.md`, `coding-task*.md`, and `review.md` when present
- the current implementation snapshot: diff, changed files, tests, migrations, config changes, and generated artifacts when relevant

If task artifacts and the implementation disagree, reflect the implemented state accurately and call out the mismatch in `Follow-ups` or `Notes`.

## Output File

When a task folder exists, write the PR description to:

```text
tmp/tasks/<task-name>/pr-description.md
```

If no task folder exists, return the draft in the response and do not invent a task directory unless asked.

## Default Sources To Read First

Start with:

- `.memory-bank/project-context.md`
- `.memory-bank/architecture.md`
- `.memory-bank/gotchas.md`
- `tmp/tasks/<task-name>/task.md`
- `tmp/tasks/<task-name>/solution_*.md` if present
- `tmp/tasks/<task-name>/solutions.md` if present
- `tmp/tasks/<task-name>/implementation-plan*.md` if present
- `tmp/tasks/<task-name>/coding-task*.md` if present
- `tmp/tasks/<task-name>/review.md` if present

Then inspect the applied change:

- current diff or branch delta
- changed code and tests
- config, ENV, schema, migration, API, or observability changes

## Writing Rules

- Write for reviewers. Optimize for fast comprehension of scope, impact, and risk.
- Prefer concise summaries over file-by-file detail.
- Include explicit sections for `API Changes` and `ENV / Config Changes`. If none, write `None.`
- Include `Database / Migration Impact` when persistence, backfills, schemas, or data expectations changed. If none, write `None.`
- Include a short implementation plan of the code changes as they exist now, not as an aspirational future plan.
- Summarize only verification that was actually run or directly evidenced by the repo state.
- If rollout or operational coordination is needed, make it explicit.

## Steps

### 1. Reconstruct Intended Scope

Establish:

- the task goal
- the accepted implementation direction, if planning artifacts exist
- reviewer-relevant non-goals or constraints

### 2. Inspect The Implemented Change

Review:

- the current diff
- touched modules and boundaries
- tests and fixtures
- schema, config, API, or ENV-related changes

### 3. Extract Reviewer-Facing Impacts

Summarize:

- what changed functionally
- what changed structurally
- whether any external API or contract changed
- whether operators must change config or ENV
- whether deploy, migration, or backfill coordination is needed

### 4. Write The PR Description

Use this structure:

```markdown
# <Short PR Title>

## Summary

- ...

## Implementation Plan

- ...

## API Changes

- None.

## ENV / Config Changes

- None.

## Database / Migration Impact

- None.

## Testing

- ...

## Follow-ups

- None.
```

Section guidance:

- `Summary`: 2-4 bullets describing the user-facing or system-facing outcome.
- `Implementation Plan`: 3-6 bullets describing the main code changes that were made.
- `API Changes`: endpoint, payload, RPC, event, schema, or contract surface changes. Use `None.` if unchanged.
- `ENV / Config Changes`: new or changed env vars, config files, defaults, or operational toggles. Use `None.` if unchanged.
- `Database / Migration Impact`: migrations, backfills, schema assumptions, data fixes, or `None.`
- `Testing`: tests run, checks run, or explicit note that verification was not run.
- `Follow-ups`: residual work, known caveats, reviewer attention points, or `None.`

### 5. Save And Report

When a task folder exists:

- write `pr-description.md` into that folder
- return the full path and a short summary of what the description covers

## Output Contract

The generated `pr-description.md` must:

- be valid Markdown
- reflect the implemented state, not just the intended plan
- include explicit `None.` entries for absent API, ENV, or migration changes
- be concise enough to paste into a PR body without major editing
