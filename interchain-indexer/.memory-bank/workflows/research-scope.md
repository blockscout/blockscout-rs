# Scope Research Workflow

Investigate and document the intended scope of a codebase topic through an interactive discussion with a human collaborator. The durable output for this workflow is a research note under `.memory-bank/research/`, but that file must not be created until the human explicitly confirms the topic framing and asks to persist it.

**Use for:**

- clarifying the intended scope of a subsystem, feature area, runtime flow, or architectural boundary
- converting exploratory discussion into durable repo knowledge
- reducing repeated rediscovery of a non-trivial topic

**Do NOT use for:**

- immediate implementation planning for a specific ticket
- one-off temporary notes that do not need to become durable knowledge
- creating a research note without first discussing scope with a human

## Required Inputs

- the topic or question to investigate
- any human-provided constraints, hypotheses, or concerns
- existing `.memory-bank/` context relevant to the topic

## Default Sources to Read First

Start by reading the smallest set of canonical files that can frame the topic:

- `.memory-bank/project-context.md`
- `.memory-bank/architecture.md`
- `.memory-bank/exploration-map.md`
- `.memory-bank/gotchas.md`
- `.memory-bank/research/README.md`
- existing `.memory-bank/research/*.md` notes related to the topic
- relevant `.memory-bank/adr/*.md` and `.memory-bank/rules/*.md` files when they materially affect the topic

Then read the source-of-truth code paths for the topic.

## Steps

### 1. Frame the Research Question

Restate the topic in concrete terms:

- what behavior, boundary, or flow is being investigated
- what is in scope
- what is intentionally out of scope
- what decisions or future work this research should support

If the prompt is ambiguous, ask focused clarification questions before proceeding.

### 2. Gather Existing Context

Review the relevant `.memory-bank/` documents and locate authoritative code paths.

Prefer:

- primary runtime entrypoints
- authoritative config models
- core data structures and persistence layers
- existing research notes over ad hoc comments or incidental callers

### 3. Discuss Findings with the Human

Summarize the emerging understanding in plain language:

- the short answer
- key flows or boundaries
- important invariants
- unresolved uncertainties

Use discussion to test whether the current framing matches the human's intent. If it does not, adjust the scope and continue investigation.

### 4. Propose a Research Note Outline

Before creating any file, present a proposed note outline based on `.memory-bank/research/README.md`.

Include:

- proposed title
- scope statement
- main sections that will contain substance
- source-of-truth files expected to anchor the note
- any open questions that still need explicit confirmation

### 5. Wait for Explicit Confirmation

Do not create `.memory-bank/research/<topic>.md` yet.

Pause and wait for explicit human confirmation that the topic framing and outline are correct. Confirmation should be unambiguous, such as:

- "yes, write it"
- "create the research note"
- "persist this under `.memory-bank/research/`"

If confirmation is not given, continue the discussion instead of writing files.

### 6. Create the Research Note After Confirmation

Once the human confirms:

- create the file under `.memory-bank/research/`
- use the standard research template from `.memory-bank/research/README.md`
- keep it durable, concrete, and grounded in source-of-truth files
- record open questions instead of guessing

### 7. Close the Loop

Report:

- the full path of the created file
- the final title
- any remaining open questions or maintenance triggers

## Output Contract Before Confirmation

Before confirmation, the output is conversational only. It may include:

- a scoped summary
- candidate file name
- proposed outline
- source-of-truth references
- open questions

It must not include a newly created research file.

## Output Contract After Confirmation

After confirmation, the output is:

- a new or updated `.memory-bank/research/*.md` note
- a concise summary of what was persisted

## Quality Bar

The resulting note should:

- explain one non-trivial runtime behavior or architectural boundary
- reduce future rediscovery work
- distinguish facts from inferred conclusions
- name the files, types, tables, configs, or contracts that define the behavior
- stay concise enough to remain maintainable

## Stop Conditions

Stop and ask the human before writing if:

- the scope is still disputed
- multiple plausible interpretations remain
- the correct durable location is unclear
- the topic overlaps heavily with an existing research note and the merge strategy is uncertain

