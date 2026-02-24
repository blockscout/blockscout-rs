# Claude Code Overrides

This file adds Claude-specific features on top of AGENTS.md.

## Commands

@justfile

## Extended Context

@.memory-bank/architecture.md
@.memory-bank/gotchas.md
@.memory-bank/adr/README.md
@.memory-bank/rules/

## Claude-specific Instructions

- When session ends, offer to update .memory-bank/ files if new patterns were discovered
- For ADRs, use the template at .memory-bank/adr/template.md
- When exploring unfamiliar code, check .memory-bank/ files first for context
