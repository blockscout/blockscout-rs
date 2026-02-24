#!/usr/bin/env bash
#
# Publishes a GitHub issue from a markdown file created by
# the gh-issue-bug or gh-issue-improvement workflows.
#
# Canonical location: .memory-bank/workflows/scripts/gh-issue-publish.sh
# Usage: gh-issue-publish.sh <path-to-issue-file>
#
# The script:
#   1. Validates the file exists
#   2. Checks GitHub CLI authentication
#   3. Parses title (first line, strips "# " prefix) and body (line 2+)
#   4. Detects issue type (bug vs improvement) by section headers
#   5. Creates the issue with appropriate label
#
# Output on success:
#   OK <issue-url>
#   LABEL <label>
#
# Output on failure:
#   ERROR <message>
#
# Exit codes:
#   0 - Success
#   1 - Missing or invalid argument
#   2 - File not found
#   3 - GitHub CLI not authenticated
#   4 - Cannot detect issue type
#   5 - Failed to create issue

set -euo pipefail

# --- Validate input ---

if [[ $# -lt 1 || -z "${1:-}" ]]; then
    echo "ERROR Missing file path argument"
    exit 1
fi

FILE="$1"

if [[ ! -f "$FILE" ]]; then
    echo "ERROR File not found: $FILE"
    exit 2
fi

# --- Check authentication ---

if ! gh auth status &>/dev/null; then
    echo "ERROR GitHub CLI not authenticated. Run: gh auth login"
    exit 3
fi

# --- Parse title ---

TITLE=$(head -1 "$FILE" | sed 's/^#[[:space:]]*//')

if [[ -z "$TITLE" ]]; then
    echo "ERROR Could not extract title from first line"
    exit 1
fi

# --- Detect issue type ---

if grep -qE "^## (Steps to Reproduce|Actual Behavior|Root Cause|Suggested Fix)" "$FILE"; then
    LABEL="bug"
elif grep -qE "^## (Motivation|Proposed Changes|Expected Benefits)" "$FILE"; then
    LABEL="enhancement"
else
    echo "ERROR Cannot detect issue type from section headers"
    exit 4
fi

# --- Create issue ---

ISSUE_URL=$(tail -n +2 "$FILE" | gh issue create \
    --title "$TITLE" \
    --body-file - \
    --label "$LABEL") || {
    echo "ERROR Failed to create GitHub issue"
    exit 5
}

echo "OK $ISSUE_URL"
echo "LABEL $LABEL"
