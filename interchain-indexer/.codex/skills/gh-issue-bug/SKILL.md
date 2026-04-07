---
name: gh-issue-bug
description: Draft a GitHub bug report from the current conversation, save it under tmp/gh-issues/, and follow the repository's canonical bug-report workflow.
---

# GitHub Bug Issue Skill

Use this skill when the user wants a GitHub issue drafted for a bug, failure, regression, or other broken behavior.

## Workflow

Follow the canonical workflow in `../../../.memory-bank/workflows/gh-issue-bug.md`.

## Required Guardrails

- Use this skill only for bugs and incorrect behavior, not feature requests or improvements.
- Draft the issue from the conversation and any verified local context needed to make it accurate.
- If the conversation does not contain enough information for reproduction steps, expected behavior, actual behavior, root cause, and suggested fix, ask for the missing details before writing the file.
- Keep the suggested fix high-level. Do not include code snippets, file lists, or implementation plans.
- Save the issue to `tmp/gh-issues/YYMMDD-<short-issue-name>.md` and report the full path after creation.

## Minimal Starting Reads

Start with:

- `../../../.memory-bank/workflows/gh-issue-bug.md`

Then read only the relevant code, tests, `.memory-bank/` context, and conversation details needed to draft an accurate bug report.
