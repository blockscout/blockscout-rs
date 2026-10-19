---
name: pr-description
description: Draft or refresh a reviewer-facing Markdown PR description from `tmp/tasks/<task-name>/` and the current implementation snapshot, covering functional scope, API/config, migration, testing, and follow-ups.
---

# PR Description

Use when an implemented change needs concise reviewer context before PR review, handoff, or merge.

Follow `../../../.memory-bank/workflows/pr-description.md`.

Reconcile task artifacts with the current diff and write `pr-description.md` in the existing task folder. Describe implemented state, not plans; include explicit `None.` entries for no API/config/migration impact; and never claim checks that were not run.
