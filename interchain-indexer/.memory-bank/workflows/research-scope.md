# Scope Research Workflow

Investigate and document the intended scope of a codebase topic through an interactive discussion with a human collaborator. The durable output for this workflow is a reusable research note under `.memory-bank/research/`, but that file must not be created or updated until the human explicitly asks for it.

This workflow is designed to build durable, reusable knowledge about a subsystem, feature area, runtime flow, integration boundary, or architectural slice of the project. The resulting note should remain useful across multiple future workflows and should not be organized around a single ticket, bug, or feature request unless that artifact is itself the topic being researched.

The workflow is dialogue-first. Before any file is written, the agent should help the human build a shared understanding through short iterative exchanges. After a file is created, the discussion may continue, and new knowledge may later be merged back into the same output file only when the human explicitly requests an update.

## Interaction Model

The workflow has two persistent states:

- **Exploration state**: the agent investigates the topic and discusses findings with the human without writing or updating the output file
- **Persistence state**: the agent writes a new research note or updates an existing one only when explicitly instructed

The workflow also has a discovery step before writing:

- **Existing research resolution**: the agent first checks whether a matching research note already exists for the requested scope, and decides whether to continue that note or start a new one

The agent must not assume that first-pass understanding is ready to persist. Initial findings are provisional and should be pressure-tested through discussion before being written into durable memory.

The agent must not push the human toward persistence commands. However, at the beginning of each major stage, the agent should briefly mention what command the human can use if they want the current result written into the output file.

Examples:

- at the beginning of the initial exploration stage: mention that the human can say **"write result"** when they want the current understanding persisted
- at the beginning of a post-document correction or continuation stage: mention that the human can say **"update result"** when they want newly established knowledge merged into the output file

These mentions should be brief, neutral, and non-pushy.

## Use for

- clarifying the intended scope of a subsystem, feature area, runtime flow, integration boundary, or architectural slice
- documenting the composition, responsibilities, boundaries, interfaces, and invariants of a non-trivial part of the codebase
- building durable repo knowledge through iterative discussion
- reducing repeated rediscovery of how a part of the system is structured and where its source of truth lives
- refining and extending an existing research note after further discussion
- continuing previously created research when the current scope substantially overlaps it

## Do NOT use for

- immediate implementation planning for a specific ticket
- proposing or evaluating concrete fixes or feature designs as the main goal
- one-off temporary notes that do not need to become durable knowledge
- creating or updating a research note without explicit human instruction
- organizing the research around a single bug, ticket, or feature request unless that artifact is itself the topic being researched
- automatically merging distinct scopes into one research note just because they are loosely related

## Required Inputs

- the topic or question to investigate
- any human-provided constraints, hypotheses, or concerns
- existing `.memory-bank/` context relevant to the topic
- existing `.memory-bank/research/*.md` notes that may overlap the requested scope
- if updating an existing note, the current persisted note and the new conclusions reached in discussion

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

Treat existing research notes as secondary context unless they are validated against current source-of-truth code and configuration. When an existing note conflicts with code, schemas, config, or runtime contracts, prefer the source-of-truth implementation and record the mismatch explicitly.

## Steps

### 0. Resolve Existing Research Context

Before starting a new research thread, the agent should search for existing `.memory-bank/research/*.md` notes relevant to the requested scope.

The agent should classify the result into one of these cases:

- **strong match**: an existing note already covers essentially the same subsystem, flow, boundary, or architectural slice
- **partial match**: an existing note covers a neighboring or overlapping area, but not the exact same scope
- **no useful match**: no existing note meaningfully covers the requested scope

Then the agent should respond briefly:

- if there is a strong match, propose continuing within that existing note
- if there is only a partial match, mention it as useful context but do not automatically choose it as the output target
- if there is no useful match, proceed as a new research effort

This message should be short and discussion-oriented. It should not pressure the human to persist anything yet.

The agent should only treat an existing note as the target output file when the scope is substantially the same. Nearby or loosely related notes should be used as context, not automatically as the persistence target.

### 1. Start the Exploration Stage

At the beginning of the workflow, the agent should briefly state that the human can say **"write result"** when they want the current understanding persisted to a new output file, or **"update result"** when they want findings merged into the existing matching note.

Then restate the topic in concrete subsystem-centered terms:

- what behavior, boundary, composition, or flow is being investigated
- what is in scope
- what is intentionally out of scope
- whether the topic is best understood as a subsystem, runtime flow, integration boundary, or architectural slice
- what kind of durable knowledge this note should preserve for future reuse

If the prompt is ambiguous, ask focused clarification questions before proceeding.

Prefer framing that is stable across multiple future tasks. Avoid framing the topic as “how to solve X” unless the human explicitly wants task-specific investigation instead of reusable research.

### 2. Gather Existing Context

Review the relevant `.memory-bank/` documents and locate authoritative code paths.

Prefer:

- primary runtime entrypoints
- authoritative config models
- core data structures and persistence layers
- public/internal interfaces and contracts
- existing research notes over ad hoc comments or incidental callers

Identify the smallest set of files that define the topic’s composition and behavior. Favor source-of-truth definitions over convenience references.

### 3. Discuss Findings Iteratively

Summarize the current understanding in a compact discussion-friendly form.

By default, the agent should provide:

- 3 to 7 short key takeaways
- the main components, boundaries, or flows currently identified
- what is still unclear
- what to inspect next, if needed

Keep the response short. Do not produce a long-form draft note, a detailed final structure, or a near-final document in chat unless the human explicitly asks for that.

The purpose of this stage is to build shared understanding, not to finalize wording.

### 4. Continue Exploration Until the Human Signals Sufficiency

Continue investigating and refining the understanding through iterative discussion.

The agent should:

- answer follow-up questions
- inspect deeper when the human points at specific parts
- revise earlier conclusions when the human corrects framing
- keep outputs concise and discussion-oriented
- treat current understanding as provisional until the human decides otherwise

The agent must not prematurely switch into document-authoring mode.

### 5. Wait for Explicit Persistence Instruction

Do not create `.memory-bank/research/<topic>.md`, do not update an existing research note, and do not simulate the final document in detail until the human explicitly asks to persist the result.

Valid examples include:

- "write result"
- "save result"
- "create the file"
- "persist this research"
- "update result"
- "refresh the file"
- "merge this into the note"
- "update the research note"

If the human does not give such an instruction, remain in exploration mode.

### 6. Decide the Persistence Target

When the human explicitly asks to persist, the agent must decide whether to:

- create a new note
- update an existing strongly matching note

Use these rules:

- if a strong existing match was found and the current scope is still aligned with it, update that note
- if only a partial match exists, prefer creating a new note unless the human explicitly wants to extend the older one
- if multiple plausible target notes exist, stop and ask the human which one should be the durable target
- if no useful match exists, create a new note

The agent must avoid collapsing distinct scopes into one file unless the human explicitly wants that merge.

### 7. Create the Initial Output File Only After Explicit Instruction

If the persistence target is a new note, then once the human explicitly asks to persist the result:

- create the file under `.memory-bank/research/`
- add the new note to the `Current Research Notes` section in `.memory-bank/research/README.md`
- use the standard research template from `.memory-bank/research/README.md`
- ground the note in source-of-truth files and in the refined discussion, not only in the first-pass exploration
- organize it around the structure of the system, not around one task or issue
- distinguish clearly between facts, inferred conclusions, and unresolved unknowns
- keep it concise enough to remain maintainable

After the file is created, the conversation does not end. The human may continue reviewing the result, asking clarifying questions, challenging conclusions, or extending the investigation.

### 8. Continue Discussion After Persistence

After the initial output file exists, the workflow returns to discussion mode.

At the beginning of this correction or continuation stage, the agent should briefly state that the human can say **"update result"** when they want newly established knowledge merged into the existing output file.

The agent should then continue normal research dialogue:

- answer follow-up questions
- inspect additional source-of-truth paths
- refine or correct the understanding
- identify what in the persisted note is incomplete, weak, outdated, or wrong

The agent must not update the file automatically just because new insight appeared in conversation.

### 9. Update the Existing Output File Only After Explicit Instruction

If the persistence target is an existing note, or if the human later asks to merge additional findings, then only after explicit instruction should the agent:

- merge the newly established knowledge into the existing output file
- preserve useful existing material that is still correct
- revise or remove statements that are no longer supported
- keep the file aligned with the latest shared understanding
- maintain the distinction between facts, inferred conclusions, and unresolved unknowns

### 10. Close the Loop After Write or Update

After a write or update action, report:

- the full path of the written or updated file
- the title
- whether this was a new note or a continuation of an existing one
- whether `.memory-bank/research/README.md` was updated to include the note in `Current Research Notes`
- the main source-of-truth anchors used
- any remaining open questions or maintenance triggers

Then return to discussion mode if the human wants to continue.

## Output Contract During Exploration

Before any explicit write or update instruction, the output is conversational only. It may include:

- a short, scoped summary
- key takeaways
- current uncertainties
- source-of-truth references
- possible next inspection targets
- brief mention of related existing research notes when relevant

It must not include a newly created file or a near-final document draft.

## Output Contract After Initial Persistence

After an explicit write instruction, the output is:

- a new `.memory-bank/research/*.md` note
- a concise summary of what was persisted

Further discussion may continue normally after that.

## Output Contract After Update

After an explicit update instruction, the output is:

- an updated `.memory-bank/research/*.md` note
- a concise summary of what changed

Further discussion may continue normally after that.

## Quality Bar

The resulting note should:

- explain one non-trivial subsystem, runtime behavior, integration boundary, or architectural slice
- reduce future rediscovery work
- describe the composition of the topic, not just one observed symptom
- distinguish facts from inferred conclusions
- distinguish unresolved unknowns from both facts and conclusions
- name the files, types, tables, configs, or contracts that define the behavior
- stay concise enough to remain maintainable
- remain useful across multiple future tasks, not only the current discussion
- reflect the refined understanding reached through dialogue, not just the initial exploration pass
- avoid duplicating an existing note when a strong match already exists
- avoid overloading an existing note when the scope should remain separate
- when a new research note is created, it is registered in the `Current Research Notes` section of `.memory-bank/research/README.md`

## Stop Conditions

Stop and ask the human before writing or updating if:

- the scope is still disputed
- multiple plausible interpretations remain
- the correct durable location is unclear
- multiple existing research notes could plausibly be the persistence target
- the topic overlaps heavily with an existing research note and the merge strategy is uncertain
- the discussion is drifting from reusable system research into task-specific planning
- the proposed update would overwrite unresolved disagreement instead of recording it clearly
