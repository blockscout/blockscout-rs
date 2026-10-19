# Scope Research Workflow

Investigate a reusable codebase topic through dialogue. Persist the result under `.memory-bank/research/` only after the human explicitly asks to write or update it.

## Use for

- a subsystem, lifecycle, runtime flow, integration boundary, or architectural slice
- durable knowledge that should reduce future rediscovery

Do not use it for a single implementation decision, one-off notes, or automatic documentation writes.

## Starting Context

Read the smallest relevant set from:

- `.memory-bank/README.md`
- `.memory-bank/projectbrief.md`
- `.memory-bank/sync-architecture.md`
- `.memory-bank/operation-lifecycle.md`
- `.memory-bank/api-surface.md`
- `.memory-bank/gotchas-and-edge-cases.md`
- existing `.memory-bank/research/*.md`

Treat notes as secondary to current source, configuration, schemas, and tests; record any disagreement.

## Process

1. Search existing research notes and classify each as a strong match, partial match, or no match. Use a strong match as the possible update target; do not merge merely related topics.
2. Start exploration by restating the subsystem-centered scope, boundaries, and durable knowledge to retain. Mention neutrally that the human can say `write result` or `update result` to persist established conclusions.
3. Inspect the authoritative code paths and summarize 3–7 key takeaways, boundaries, and remaining unknowns. Keep discussion concise and provisional.
4. Continue investigating and correct conclusions through dialogue. Do not produce a near-final note or write a file until explicit instruction.
5. On an explicit persistence instruction, create a new note or update the strong matching note. If several targets are plausible, ask the human which one to use.
6. Ground the note in source-of-truth files and distinguish facts, inferences, and open questions. Then reconcile `.memory-bank/README.md` with what step 5 actually did: register a new entry for a new note, or update the existing entry when a strong matching note was updated. Report the note path and its source anchors.

## Quality Bar

The note explains composition, responsibilities, interfaces, invariants, and uncertainties of one non-trivial topic. It remains useful across tasks, avoids ticket-shaped organization, and is concise enough to maintain.

## Stop Conditions

Ask before persisting when scope, target note, or a material interpretation remains disputed.
