# Memory Bank

This directory is a shared knowledge base for AI coding assistants. It provides consistent context across tools:

- **Claude Code** (via `.claude/rules/` symlink)
- **GitHub Copilot** (via `AGENTS.md` + `.claude/rules/` fallback)
- **Cursor** (via `.cursor/rules/` symlink)
- **OpenAI Codex** (via `AGENTS.md` traversal)

## Directory Structure

```
.memory-bank/
├── README.md            # This file
├── project-context.md   # Project purpose, stack, key modules
├── architecture.md      # Module map, data flow, key abstractions
├── conventions.md       # Coding style, naming, import patterns
├── gotchas.md           # Non-obvious traps: Symptom → Root cause → Fix
├── rules/               # Scoped instructions (symlinked to tool dirs)
│   ├── rust-style.md
│   ├── error-handling.md
│   ├── async-patterns.md
│   ├── database.md
│   └── testing.md
└── adr/                 # Architectural Decision Records
    ├── README.md
    └── template.md
```

## How It Works

1. **AGENTS.md** (project root) is the canonical entry point, read by all tools
2. **rules/** files use frontmatter with both `paths:` and `globs:` for cross-tool compatibility
3. Symlinks connect tool-specific directories to this shared source

## Memory Protocol

When working on this codebase:

- **Discover a non-obvious pattern or gotcha?** → Update `gotchas.md`
- **Make an architectural decision?** → Add an ADR to `adr/`
- **Get corrected about a convention?** → Update `conventions.md`
- **Learn something project-specific?** → Update relevant file

This keeps the knowledge base current and useful for future sessions.
