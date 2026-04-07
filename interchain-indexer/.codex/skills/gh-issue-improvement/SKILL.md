---
name: gh-issue-improvement
description: Draft a GitHub improvement or enhancement proposal from the current conversation, save it under tmp/gh-issues/, and follow the repository's canonical improvement workflow.
---

# GitHub Improvement Issue Skill

Use this skill when the user wants a GitHub issue drafted for an enhancement, refactor, migration, documentation improvement, or other non-bug change.

## Workflow

Follow the canonical workflow in `../../../.memory-bank/workflows/gh-issue-improvement.md`.

## Required Guardrails

- Use this skill only for improvements and enhancements, not for bugs or broken behavior.
- Draft the issue from the conversation and any verified local context needed to make it accurate.
- If the conversation does not contain enough information for description, motivation, current state, proposed changes, and expected benefits, ask for the missing details before writing the file.
- Keep `Proposed Changes` conceptual and outcome-focused. Do not include file paths, function names, code snippets, or implementation checklists.
- Save the issue to `tmp/gh-issues/YYMMDD-<short-issue-name>.md` and report the full path after creation.

## Minimal Starting Reads

Start with:

- `../../../.memory-bank/workflows/gh-issue-improvement.md`

Then read only the relevant code, tests, `.memory-bank/` context, and conversation details needed to draft an accurate improvement proposal.
