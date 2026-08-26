# Solution Review Workflow

Review already-applied code changes against the original task. The goal is to determine whether the implementation matches the intended scope, whether the acceptance criteria appear satisfied, what regressions or omissions remain, and what verification still needs to happen before merge or handoff.

**Use for:**

- post-implementation review of a completed or partially completed change
- checking whether applied code matches `task.md`, `solution_X.md`, `implementation-plan-X.md`, or `coding-task-X.md`
- validating that a change satisfies the original request before publishing, handoff, or merge
- identifying missing scope, behavioral regressions, and verification gaps after coding work has happened

**Do NOT use for:**

- choosing a solution direction before implementation
- planning a future change that has not been applied yet
- silently finishing missing implementation instead of reporting review findings
- durable research that belongs in `.memory-bank/research/`

## Required Inputs

Provide the strongest available source of truth for the intended work:

- the original task statement, issue, or user request
- `tmp/tasks/<task-name>/task.md` when a task folder exists
- `solution_X.md`, `solutions.md`, `implementation-plan-X.md`, and `coding-task-X.md` when they exist
- the actual applied changes: current diff, changed files, and relevant tests

If the expected behavior is too ambiguous to evaluate safely, state the ambiguity explicitly instead of inventing acceptance criteria.

## Output Files

When reviewing a task that already has a task folder, write the review artifact into that folder:

```text
tmp/tasks/<task-name>/review.md
```

If no task folder exists, still perform the review and return the findings in the response, but do not invent a task directory unless the user asks for one.

## Default Sources To Read First

Start with the smallest useful set:

- `.memory-bank/project-context.md`
- `.memory-bank/architecture.md`
- `.memory-bank/gotchas.md`
- `tmp/tasks/<task-name>/task.md` if present
- `tmp/tasks/<task-name>/solution_*.md` if present
- `tmp/tasks/<task-name>/solutions.md` if present
- `tmp/tasks/<task-name>/implementation-plan*.md` if present
- `tmp/tasks/<task-name>/coding-task*.md` if present
- relevant `.memory-bank/rules/*.md`

Then inspect the actual implementation surface:

- the diff or commit under review
- changed source files
- nearby tests, configs, migrations, schemas, and generated artifacts

## Review Rules

- Compare against the original task intent before judging code quality.
- Use evidence from the current diff and file contents, not assumptions about what the author meant.
- Distinguish clearly between:
  - confirmed task mismatches
  - likely risks or regressions
  - verification gaps
  - questions caused by ambiguous requirements
- Do not silently repair missing implementation unless the human explicitly changes the task from review to coding.
- When the change appears correct, still call out residual risk or missing verification if any exists.

## Steps

### 1. Reconstruct The Expected Outcome

Establish:

- the implementation goal
- the explicit success criteria and non-goals
- the accepted design direction, if analysis or planning artifacts exist
- any constraints or invariants that must still hold

If multiple task artifacts disagree, note the mismatch and treat it as a review blocker.

### 2. Inspect The Applied Change

Review the concrete implementation:

- current diff or changed files
- touched components and boundaries
- new or modified tests
- config, schema, migration, API, or observability changes

Do not limit the review to filenames already mentioned in the task. Check adjacent surfaces that the change could affect.

### 3. Map Implementation To Task Requirements

Compare the change against the reconstructed expectations:

- what is fully implemented
- what is partially implemented
- what is missing
- what appears out of scope

Prefer direct mapping from acceptance criterion to evidence in code or tests.

### 4. Assess Correctness And Regression Risk

Evaluate whether the change likely preserves important behavior:

- protocol or business invariants
- persistence and schema correctness
- API and config compatibility
- failure handling and edge cases
- test coverage for the changed behavior

When a concern depends on evidence you do not have, classify it as a verification gap or open question rather than a confirmed bug.

### 5. Write The Review Artifact

When a task folder exists, write `review.md` with:

- review scope and source inputs
- implementation coverage summary
- findings ordered by severity
- verification summary and gaps
- final recommendation

Use one of these outcome statuses:

- `accepted`
- `accepted with follow-ups`
- `changes required`
- `blocked on ambiguous task`

### 6. Report Findings

In the response, present:

- the most important findings first
- file references and concrete evidence
- what appears complete
- what still needs clarification, verification, or implementation

If no findings are discovered, say that explicitly and include any remaining verification risk.

## Output Contract

When written to disk, `review.md` should contain:

- the task or artifact set reviewed
- the implementation snapshot examined
- explicit coverage against the intended task
- findings with severity and evidence
- verification run or missing
- final status and recommendation

Use this structure:

```markdown
# <Task Title> Review

## Scope

- original task: ...
- reviewed artifacts: ...
- implementation snapshot: ...

## Expected Outcome

- ...

## Coverage Summary

- implemented: ...
- partial: ...
- missing: ...
- out of scope: ...

## Findings

### <Severity>: <Short Title>

- evidence: ...
- impact: ...
- recommendation: ...

## Verification

- ran: ...
- not run / missing: ...

## Recommendation

[accepted | accepted with follow-ups | changes required | blocked on ambiguous task]
```
