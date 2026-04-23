---
name: pr-description
description: Prepare a pull request description from task artifacts under `tmp/tasks/<task-name>/` and the current implementation snapshot. Use when drafting or refreshing a Markdown PR description before review, handoff, or merge, including implementation summary, API changes, environment or configuration changes, database or migration impact, testing, and rollout notes.
---

# PR Description Skill

Use this skill when the repository already has a task folder and the next step is to explain the implemented change clearly for reviewers.

## Workflow

Follow the canonical workflow in `../../../.memory-bank/workflows/pr-description.md`.

## Required Guardrails

- Treat task artifacts as the source of truth for intended scope, then reconcile them against the current diff and changed files.
- Write the artifact into the task folder as `pr-description.md`.
- Keep the document reviewer-facing: explain what changed and why it matters, not a file-by-file changelog.
- Include explicit sections for API changes and environment or configuration changes. If there are none, say `None.` instead of omitting the section.
- Call out schema, migration, backfill, rollout, or operational impacts when they exist.
- Summarize verification that actually happened. Do not claim tests or checks were run unless you verified that they were run.

## Minimal Starting Reads

Start with:

- `../../../.memory-bank/project-context.md`
- `../../../.memory-bank/architecture.md`
- `../../../.memory-bank/gotchas.md`
- `../../../tmp/tasks/<task-name>/task.md`
- `../../../tmp/tasks/<task-name>/solution_*.md` if present
- `../../../tmp/tasks/<task-name>/implementation-plan*.md` if present
- `../../../tmp/tasks/<task-name>/coding-task*.md` if present
- `../../../tmp/tasks/<task-name>/review.md` if present

Then inspect the current diff, changed files, tests, migrations, config updates, and any API surface needed to describe the implemented change accurately.
