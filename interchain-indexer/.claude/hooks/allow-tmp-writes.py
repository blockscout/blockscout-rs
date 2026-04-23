#!/usr/bin/env python3
"""
PreToolUse hook to automatically approve Write operations to the tmp/ directory.

This hook is designed to be used in skill frontmatter to eliminate permission
prompts when skills create files in their designated tmp/ output directories.

Usage in skill frontmatter:
  hooks:
    PreToolUse:
      - matcher: "Write|Edit"
        hooks:
          - type: command
            command: "$CLAUDE_PROJECT_DIR/.claude/hooks/allow-tmp-writes.py"
"""

import json
import os
import sys


def is_tmp_path(file_path: str) -> bool:
    """
    Check if the file path is within the tmp/ directory.

    Handles various path formats:
    - tmp/file.md (relative from project root)
    - ./tmp/file.md (explicit relative)
    - /absolute/path/to/project/tmp/file.md (absolute within project)

    Security: Rejects paths with:
    - Absolute paths outside the project directory
    - Path traversal (../other-project/tmp/data.json)
    - Parent references (tmp/../../../etc/shadow)
    """
    if not file_path:
        return False

    # Normalize path separators for consistency
    normalized = file_path.replace("\\", "/")

    project_dir = os.environ.get("CLAUDE_PROJECT_DIR") or os.getcwd()
    project_tmp = os.path.join(project_dir, "tmp")
    project_tmp_real = os.path.realpath(project_tmp)

    if os.path.isabs(normalized):
        target_path = normalized
    else:
        target_path = os.path.join(project_dir, normalized)
    target_real = os.path.realpath(target_path)

    try:
        is_within_tmp = (
            os.path.commonpath([target_real, project_tmp_real]) == project_tmp_real
        )
    except ValueError:
        return False

    # Must resolve strictly inside tmp/, not to tmp/ itself.
    return is_within_tmp and target_real != project_tmp_real


def main():
    try:
        # Read hook input from stdin
        data = json.load(sys.stdin)

        # Extract file path from tool input
        file_path = data.get("tool_input", {}).get("file_path", "")

        # Check if the file path is within tmp/ directory
        if is_tmp_path(file_path):
            # Auto-approve the write operation
            output = {
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow",
                    "permissionDecisionReason": "Auto-approved: skill writes to tmp/ directory",
                }
            }
            print(json.dumps(output))

        # For non-tmp paths, exit cleanly without output
        # This allows normal permission flow to proceed
        sys.exit(0)

    except Exception:
        # On any error, exit cleanly to let normal permission flow proceed
        # We don't want to break tool execution due to hook failures
        sys.exit(0)


if __name__ == "__main__":
    main()
