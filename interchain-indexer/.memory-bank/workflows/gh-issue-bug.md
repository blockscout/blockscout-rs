# GitHub Bug Report Workflow

Generate a well-structured bug report for GitHub issues based on conversation context.
Save the output to `tmp/gh-issues/YYMMDD-<short-issue-name>.md`.

**Use for:**

- Bugs, errors, or broken functionality
- Incorrect behavior or unexpected results
- System failures or crashes

**Do NOT use for:**

- Feature requests or enhancements
- Improvements to existing functionality
- Documentation updates
- Refactoring suggestions

## Steps

### 1. Extract Issue Information

Review the conversation to identify:

- The bug or issue being discussed
- Steps that reproduce the problem
- Expected vs actual behavior
- Root cause analysis (if discussed)
- Potential fixes (if discussed)

### 2. Generate Filename

```text
tmp/gh-issues/YYMMDD-<short-issue-name>.md
```

- `YYMMDD`: current date (e.g., `260224` for February 24, 2026)
- `<short-issue-name>`: lowercase with dashes (e.g.,
  `message-correlation-panic`, `reorg-detection-deadlock`)

### 3. Create Issue Document

```markdown
# [Brief Title of the Issue]

## Description

[Short description of the bug — 2-3 sentences summarizing the problem]

## Steps to Reproduce

1. [First step]
2. [Second step]
3. [Third step]

## Expected Behavior

[Clear description of what should happen]

## Actual Behavior

[Clear description of what actually happens, including any error messages]

## Root Cause

[Technical explanation of why the bug occurs]

## Suggested Fix

[Concise, high-level description of the proposed solution — no code snippets]
```

### 4. Content Guidelines

**MUST INCLUDE:**

- Short description (2-3 sentences)
- Steps to reproduce (numbered list)
- Expected behavior (clear statement)
- Actual behavior (including errors/symptoms)
- Root cause (technical explanation)
- Suggested fix (concise, high-level approach)

**MUST NOT INCLUDE:**

- Implementation plan or detailed code changes
- Acceptance criteria or testing checklists
- List of affected files
- Code snippets in the suggested fix section

### 5. Confirm Output

Report the full path of the created file and a one-sentence summary of the issue.

## Notes

- If insufficient information exists in the conversation, ask for clarification before
  generating the document
- Keep the suggested fix high-level and solution-oriented, not implementation-detailed
