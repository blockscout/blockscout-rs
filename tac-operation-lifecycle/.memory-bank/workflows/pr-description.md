# PR Description Workflow

Prepare a concise reviewer-facing PR description from task artifacts and the current implementation snapshot.

## Inputs and Output

Use `task.md`, solutions, plans, coding tasks, and `review.md` when present, then reconcile them with the actual diff, changed code/tests, configuration, migrations, generated outputs, and API surface. Write `tmp/tasks/<task-name>/pr-description.md` when that folder exists; otherwise return the draft without creating a folder.

## Rules

- Describe implemented state, not an aspirational plan. Call out any artifact/diff mismatch in Follow-ups.
- Optimize for scope, impact, risk, and reviewer comprehension—not a file-by-file changelog.
- State `None.` explicitly for absent API, environment/config, and migration impacts.
- Report only verification actually run or directly evidenced.

## Process

1. Reconstruct task goal, accepted direction, constraints, and reviewer-relevant non-goals.
2. Inspect actual changes, tests, data/schema/config/API effects, and operational needs.
3. Write the PR description and report its path and coverage.

## Template

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
