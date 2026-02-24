# GitHub Issue Publish Workflow

Publish a GitHub issue from a Markdown file in `tmp/gh-issues/` produced by the
[gh-issue-bug](gh-issue-bug.md) or [gh-issue-improvement](gh-issue-improvement.md)
workflows.

## Steps

### 1. Determine File Path

- If a path is provided as an argument, use it.
- Otherwise, look for the most recent `tmp/gh-issues/*.md` path mentioned in the
  conversation — typically the output of a preceding bug or improvement workflow.
- If no path can be determined, ask the user to provide one.

### 2. Run the Publish Script

```bash
.memory-bank/workflows/scripts/gh-issue-publish.sh <file-path>
```

The script:

- Validates the file exists
- Checks `gh auth status`
- Parses the title from line 1 (strips `#` prefix)
- Detects issue type from section headers → applies `bug` or `enhancement` label
- Creates the issue via `gh issue create`

### 3. Relay the Result

**On success** (output starts with `OK`):

- Report the issue URL (from the `OK` line)
- Report the label applied (from the `LABEL` line)

**On failure** (output starts with `ERROR`):

| Error message | Remediation |
|---|---|
| `GitHub CLI not authenticated` | Run `gh auth login` |
| `File not found` | Verify the path |
| `Cannot detect issue type` | File doesn't match bug or improvement templates |
| `Could not extract title` | First line is empty or malformed |
| `Failed to create GitHub issue` | Check network connectivity and repo permissions |

## Notes

- Issue type detection relies on section headers: `gh-issue-bug` uses `## Steps to
  Reproduce`, `## Root Cause`, etc.; `gh-issue-improvement` uses `## Motivation`,
  `## Proposed Changes`, etc. If those templates change, update the detection patterns
  in the script accordingly.
- The `gh` CLI infers the target repo from the current git remote — no `--repo` flag
  needed.
