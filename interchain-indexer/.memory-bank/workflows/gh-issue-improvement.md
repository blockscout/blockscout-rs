# GitHub Improvement Proposal Workflow

Generate a well-structured improvement/enhancement proposal for GitHub issues based on
conversation context. Save the output to `tmp/gh-issues/YYMMDD-<short-issue-name>.md`.

**Use for:**

- Feature enhancements or improvements
- Code refactoring or restructuring
- Performance optimizations
- Architecture improvements
- Tool or library migrations
- API consolidation or simplification
- Documentation improvements
- Testing improvements

**Do NOT use for:**

- Bugs, errors, or broken functionality
- Incorrect behavior or unexpected results
- System failures or crashes

## Steps

### 1. Extract Issue Information

Review the conversation to identify:

- The improvement or enhancement being proposed
- Current state and its limitations
- Motivation and benefits of the improvement
- Proposed changes at a conceptual level
- Expected outcomes or impact

### 2. Generate Filename

```text
tmp/gh-issues/YYMMDD-<short-issue-name>.md
```

- `YYMMDD`: current date (e.g., `260224` for February 24, 2026)
- `<short-issue-name>`: lowercase with dashes (e.g.,
  `optimize-batch-processing`, `add-ictt-multi-hop-support`,
  `consolidate-rpc-retry-logic`)

### 3. Create Issue Document

```markdown
# [Brief Title of the Improvement]

## Description

[Short description — 2-3 sentences summarizing what is being proposed]

## Motivation

[Why this improvement is needed — what problems it solves, what benefits it provides]

## Current State

[How things work today, including limitations or pain points]

## Proposed Changes

[High-level description of what should change. Focus on WHAT, not HOW. Strategic and
outcome-focused — no file paths, function names, or step-by-step checklists.]

## Expected Benefits

[Concrete benefits: improved performance, better maintainability, reduced complexity,
etc.]
```

### 4. Content Guidelines

**MUST INCLUDE:**

- Short description (2-3 sentences)
- Motivation explaining why the improvement is needed
- Current state description showing what exists today
- Proposed changes (clear, organized list of what should change)
- Expected benefits (concrete outcomes)

**MUST NOT INCLUDE:**

- Detailed code implementation or snippets
- Specific file paths or module names
- Step-by-step implementation checklists
- Granular action items for each file or component
- Function names or code structure details
- Line-by-line diffs or patches

**STYLE GUIDELINES:**

- Focus on "what" should change and "why", not "how" to implement it
- Keep "Proposed Changes" to 3-6 high-level points
- Each point describes an outcome or conceptual change, not an implementation task

**PROPOSED CHANGES — GOOD EXAMPLES:**

✓ "Consolidate RPC retry logic under a unified backoff strategy"
✓ "Deprecate per-chain polling in favour of a shared log-stream abstraction"
✓ "Preserve existing message correlation semantics while reducing database round-trips"
✓ "Update configuration documentation to reflect the new bridge config format"

**PROPOSED CHANGES — BAD EXAMPLES:**

✗ "Create `RetryPolicy` struct in `interchain-indexer-logic/src/provider_layers.rs`"
✗ "Update `IndexerConfig` in `config.rs` to add `retry_max_attempts: u32` field"
✗ "Remove `poll_logs` function from `log_stream.rs`"
✗ "Migrate tests: delete `test_polling.rs` and create new stream tests"

### 5. Confirm Output

Report the full path of the created file and a one-sentence summary of the improvement.

## Notes

- If insufficient information exists in the conversation, ask for clarification before
  generating the document
- **CRITICAL**: Keep proposed changes conceptual and strategic — avoid prescriptive
  implementation details
- Think: "What outcomes do we want?" not "What steps should we take?"
- If you find yourself writing file paths, function names, or numbered checklists,
  you're being too prescriptive
