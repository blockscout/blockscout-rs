---
name: solution-review
description: Review applied changes against the original task and artifacts under `tmp/tasks/<task-name>/`, identify scope mismatches, regressions, and verification gaps, and write a review before handoff or merge.
---

# Solution Review

Use after code has been changed and correctness against task intent must be assessed.

Follow `../../../.memory-bank/workflows/solution-review.md`.

Review the original task source first, then the actual diff, changed files, tests, and adjacent surfaces. Separate confirmed findings, uncertainty, and verification gaps. Do not silently fix missing work. When the task folder exists, write `review.md` there with evidence and a clear recommendation.
