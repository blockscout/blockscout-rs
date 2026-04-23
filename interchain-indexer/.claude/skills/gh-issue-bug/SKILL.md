---
name: gh-issue-bug
description: Generate a structured bug report for GitHub issues based on conversation context about bugs, errors, or broken functionality
disable-model-invocation: true
hooks:
  PreToolUse:
    - matcher: "Write|Edit"
      hooks:
        - type: command
          command: "$CLAUDE_PROJECT_DIR/.claude/hooks/allow-tmp-writes.py"
    - matcher: "Bash"
      hooks:
        - type: command
          command: "$CLAUDE_PROJECT_DIR/.claude/hooks/allow-tmp-dirs.py"
---

# GitHub Bug Report Generator Skill

Follow the workflow defined in @../../../.memory-bank/workflows/gh-issue-bug.md
