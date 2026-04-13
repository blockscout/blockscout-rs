---
name: research-scope
description: Investigate the intended scope of a codebase topic through discussion with a human, propose a research-note outline, and only create a .memory-bank/research file after explicit human confirmation.
---

# Research Scope Skill

Use this skill when the user wants to investigate and clarify a codebase topic before persisting durable research.

## Workflow

Follow the canonical workflow in `../../../.memory-bank/workflows/research-scope.md`.

## Required Guardrails

- Read the relevant `.memory-bank/` context before forming conclusions.
- Treat the interaction as a discussion, not a one-shot dump.
- Propose a title and outline before creating any research file.
- Do not create `.memory-bank/research/*.md` until the human gives explicit confirmation.
- If the topic overlaps an existing research note, discuss whether to extend the existing note or create a new one.

## Minimal Starting Reads

Start with:

- `../../../.memory-bank/project-context.md`
- `../../../.memory-bank/architecture.md`
- `../../../.memory-bank/exploration-map.md`
- `../../../.memory-bank/gotchas.md`
- `../../../.memory-bank/research/README.md`

Then read only the additional research notes, ADRs, rules, and source files needed for the current topic.
