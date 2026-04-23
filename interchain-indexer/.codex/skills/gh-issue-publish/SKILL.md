---
name: gh-issue-publish
description: Publish a drafted GitHub issue from tmp/gh-issues/ by running the repository's gh CLI publish script and relaying the resulting URL or remediation.
---

# GitHub Issue Publish Skill

Use this skill when the user wants to publish a previously drafted GitHub issue markdown file with the `gh` CLI.

## Workflow

Follow the canonical workflow in `../../../.memory-bank/workflows/gh-issue-publish.md`.

## Required Guardrails

- Determine the issue file path from the user input or the most recent `tmp/gh-issues/*.md` path in the conversation. If no path can be determined, ask for one.
- Use the checked-in script at `../../../.memory-bank/workflows/scripts/gh-issue-publish.sh` rather than recreating its logic inline.
- Relay the script result exactly at the semantic level:
  - On success, report the issue URL and applied label.
  - On failure, report the error and the documented remediation.
- Do not edit the issue markdown unless the user asks for content changes.

## Minimal Starting Reads

Start with:

- `../../../.memory-bank/workflows/gh-issue-publish.md`
- `../../../.memory-bank/workflows/scripts/gh-issue-publish.sh`

Then run the script with the resolved issue path and report the outcome.
