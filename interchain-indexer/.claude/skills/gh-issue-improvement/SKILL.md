---
name: gh-issue-improvement
description: Generate a structured improvement/enhancement proposal for GitHub issues based on conversation context
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

# GitHub Improvement Proposal Generator Skill

Follow the workflow defined in @../../../.memory-bank/workflows/gh-issue-improvement.md
