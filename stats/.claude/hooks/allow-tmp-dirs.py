#!/usr/bin/env python3
"""
PreToolUse hook to automatically approve Bash mkdir operations for the tmp/ directory.

This hook is designed to be used in skill frontmatter to eliminate permission
prompts when skills create directories in their designated tmp/ output directories.

Usage in skill frontmatter:
  hooks:
    PreToolUse:
      - matcher: "Bash"
        hooks:
          - type: command
            command: "$CLAUDE_PROJECT_DIR/.claude/hooks/allow-tmp-dirs.py"
"""

import json
import os
import shlex
import sys


def is_tmp_mkdir_command(command: str) -> bool:
    """
    Check if the Bash command is creating directories within the tmp/ directory.

    Handles various mkdir patterns:
    - mkdir tmp/subdir
    - mkdir -p tmp/subdir
    - mkdir -p tmp/gh-issues
    - mkdir -p ./tmp/impl_plans
    - mkdir -p /absolute/path/to/project/tmp/subdir (absolute within project)

    Security: Rejects commands with:
    - Multiple paths (mkdir tmp/ok /etc)
    - Shell operators (mkdir tmp/ok && rm -rf /)
    - Command substitution (mkdir tmp/$(malicious))
    - Redirections or other shell metacharacters
    - Absolute paths outside the project directory
    - Path traversal (mkdir ../other-project/tmp/data)
    - Parent references (mkdir tmp/../../../etc/shadow)
    """
    if not command:
        return False

    # Normalize whitespace
    normalized = " ".join(command.split())

    # Check if it's exactly a mkdir command (allowing absolute/relative path to mkdir)
    try:
        parts = shlex.split(normalized)
    except ValueError:
        return False

    if not parts or os.path.basename(parts[0]) != "mkdir":
        return False

    # Reject commands with shell operators or metacharacters
    dangerous_chars = ["&&", "||", ";", "|", "$(", "`", ">", "<", "$", "{", "}"]
    if any(char in command for char in dangerous_chars):
        return False

    # Extract all arguments after flags
    # Pattern: mkdir [-p] [other flags] path1 [path2...]
    paths = []
    skip_next = False

    for part in parts[1:]:  # Skip "mkdir"
        if skip_next:
            skip_next = False
            continue
        if part.startswith("-"):
            # Check if this flag takes an argument (like -m mode)
            if part in ["-m", "--mode", "-Z", "--context"]:
                skip_next = True
            continue
        # This is a path argument
        paths.append(part)

    # Must have exactly one path
    if len(paths) != 1:
        return False

    path = paths[0]

    # Normalize path separators
    normalized_path = path.replace("\\", "/")

    # Reject any path containing parent directory references
    if ".." in normalized_path:
        return False

    # Handle absolute paths: allow if within CLAUDE_PROJECT_DIR/tmp/
    if normalized_path.startswith("/"):
        project_dir = os.environ.get("CLAUDE_PROJECT_DIR", "")
        if project_dir:
            project_tmp = f"{project_dir.rstrip('/')}/tmp/"
            if normalized_path.startswith(project_tmp):
                return True
        return False

    # Only allow paths that start with tmp/ or ./tmp/
    # This ensures we're creating directories within the tmp/ directory
    return normalized_path.startswith("tmp/") or normalized_path.startswith("./tmp/")


def main():
    try:
        # Read hook input from stdin
        data = json.load(sys.stdin)

        # Extract command from tool input
        command = data.get("tool_input", {}).get("command", "")

        # Check if the command is creating a tmp/ directory
        if is_tmp_mkdir_command(command):
            # Auto-approve the mkdir operation
            output = {
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow",
                    "permissionDecisionReason": "Auto-approved: skill creates directory in tmp/",
                }
            }
            print(json.dumps(output))

        # For non-tmp mkdir commands, exit cleanly without output
        # This allows normal permission flow to proceed
        sys.exit(0)

    except Exception:
        # On any error, exit cleanly to let normal permission flow proceed
        # We don't want to break tool execution due to hook failures
        sys.exit(0)


if __name__ == "__main__":
    main()
