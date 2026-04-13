# Memory Bank

This directory is a shared knowledge base for AI coding assistants. In this checkout, it provides consistent context through the integrations that are actually present:

- **Claude Code** (via `.claude/rules/` symlink)
- **GitHub Copilot** (via `AGENTS.md` — no dedicated config; relies on tool reading the file)
- **Cursor** (via `.cursor/rules/` symlink)
- **OpenAI Codex** (via `AGENTS.md` traversal)

## Directory Structure

```
.memory-bank/
├── README.md            # This file
├── project-context.md   # Project purpose, current scope, crates, runtime components
├── codebase-review.md   # High-level review of strengths, risks, and complexity hotspots
├── architecture.md      # Module map, data flow, key abstractions
├── exploration-map.md   # "If you need X, start here" navigation guide
├── glossary.md          # Repo-specific terminology
├── gotchas.md           # Non-obvious traps: Symptom → Root cause → Fix
├── research/            # Durable multi-file investigations
│   ├── README.md
│   ├── avalanche-blockchain-id-resolution.md
│   ├── avalanche-bridge-filtering.md
│   ├── config-loading-and-validation.md
│   ├── db-schema-and-layer.md
│   ├── message-lifecycle.md
│   ├── stats-projection.md
│   ├── stats-subsystem.md
│   └── token-info-service.md
├── rules/               # Coding conventions (symlinked to tool dirs)
│   ├── rust-style.md
│   ├── error-handling.md
│   ├── async-patterns.md
│   ├── database.md
│   └── testing.md
├── workflows/           # Tool-agnostic task workflows (shared across all AIDEs)
│   ├── gh-issue-bug.md         # Draft a GitHub bug report
│   ├── gh-issue-improvement.md # Draft a GitHub enhancement proposal
│   ├── gh-issue-publish.md     # Publish a drafted issue via gh CLI
│   ├── implementation-plan.md  # Turn approved analysis into coding-ready design
│   ├── pr-description.md       # Prepare reviewer-facing PR description
│   ├── research-scope.md       # Scope and write a .memory-bank/research/ note
│   ├── solution-review.md      # Post-implementation review against task
│   ├── task-analysis.md        # Pre-implementation task review and options
│   ├── task-to-code.md         # Execute a prepared coding task handoff
│   └── scripts/
│       └── gh-issue-publish.sh
└── adr/                 # Architectural Decision Records
    ├── README.md
    ├── template.md
    ├── 001-message-buffer-tiered-storage.md
    └── 002-primary-chain-filtering.md
```

## How It Works

1. **AGENTS.md** (project root) is the canonical entry point and router
   (`CLAUDE.md` is a symlink to it; Claude-specific overrides live in `.claude/CLAUDE.md`)
2. **.memory-bank/** holds the canonical shared repo knowledge
3. **rules/** files use frontmatter with both `paths:` and `globs:` for cross-tool compatibility
4. **workflows/** holds reusable task procedures; tool-specific integrations
   (for example `.cursor/skills/`, `.claude/skills/`, and `.codex/skills/`) should stay thin and
   reference these files
5. When present, symlinks or adapters connect tool-specific directories to this shared source
6. **Hooks** (`.claude/hooks/`) auto-approve tmp/ writes for Claude Code skills;
   Cursor and Codex lack equivalent hook support, so those tools prompt for permission instead

## Knowledge Categories

- `project-context.md`
  - what the service is, what it currently supports, and how it is organized
- `codebase-review.md`
  - overall assessment of architecture strengths, complexity hotspots, and risks
- `architecture.md`
  - structural view of the runtime system and core abstractions
- `exploration-map.md`
  - navigation guide for common codebase questions
- `glossary.md`
  - repo-specific terminology
- `research/`
  - focused investigations into multi-file behaviors and invariants
- `gotchas.md`
  - non-obvious traps and their fixes
- `rules/`
  - coding conventions and stable patterns
- `workflows/`
  - reusable task procedures
- `adr/`
  - architectural decisions and their rationale

## Memory Protocol

When working on this codebase:

- **Discover a non-obvious pattern or gotcha?** → Update `gotchas.md`
- **Make an architectural decision?** → Add an ADR to `adr/`
- **Get corrected about a convention?** → Update the relevant file in `rules/`
- **Finish a reusable multi-file investigation?** → Add or update a note in `research/`
- **Learn something project-specific?** → Update the relevant canonical file
- **Create a temporary note in `tmp/` during investigation?** → Promote durable findings into
  `.memory-bank/` and avoid leaving long-term knowledge only in `tmp/`

This keeps the knowledge base current and useful for future sessions.
