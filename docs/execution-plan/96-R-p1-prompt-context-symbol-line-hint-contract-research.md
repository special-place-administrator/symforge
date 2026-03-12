# Research: P1 Prompt Context Symbol Line Hint Contract

Related plan:

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [94-R-p1-prompt-context-symbol-file-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/94-R-p1-prompt-context-symbol-file-hint-contract-research.md)
- [95-T-p1-prompt-context-symbol-file-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/95-T-p1-prompt-context-symbol-file-hint-shell.md)

Goal:

- choose the smallest prompt-context follow-up contract that lets an explicit line hint disambiguate the new combined file+symbol exact-selector path without broadening prompt parsing into a general natural-language parser

## Current Code Reality

After task 95, prompt-context now does the right thing for:

1. file hint only => file outline
2. symbol hint only => name-only symbol context
3. file hint plus symbol hint => exact-selector symbol context

That means the remaining sharp edge is narrower:

- if the hinted file contains duplicate same-name symbols
- prompt-context now returns the stable exact-selector ambiguity message
- but it still has no way to consume an explicit line hint from the prompt and feed `symbol_line`

The exact-selector substrate already exists. Prompt-context is missing only the smallest line-hint bridge.

## Candidate Approaches

### Option 1: leave ambiguity resolution outside prompt-context

- no additional parsing work
- keeps prompt-context smaller
- forces the user back to raw tool syntax even when the prompt already contains an explicit line hint

### Option 2: parse only explicit `line N` hints

- small and testable
- reuses the existing `symbol_line` selector exactly as-is
- avoids accidentally treating unrelated numbers in prompts as symbol selectors

### Option 3: parse any standalone integer or looser line phrasing

- potentially more ergonomic
- too risky for the next slice because prompt text often contains versions, counts, and unrelated numbers

## Decision: Add A Narrow `line N` Bridge

Recommendation:

- when a prompt contains:
  - a file hint
  - a symbol hint
  - an explicit `line N` hint
- pass `symbol_line = N` into the exact-selector symbol-context flow

Keep everything else unchanged:

- file-only prompts still return outline
- symbol-only prompts still return name-only symbol context
- combined prompts without a line hint still keep the current stable ambiguity behavior
- repo-map fallback stays last

## Why This Is The Smallest Useful Slice

- it extends prompt-context with one explicit disambiguator instead of a fuzzy parser
- it reuses the exact-selector contract already proven in `get_symbol_context`
- it turns the most actionable exact-selector follow-up (`symbol_line`) into something prompt-context can actually consume
- it preserves the current stable ambiguity message as the fallback when no line hint is present

## Recommended Next Implementation Slice

- add a helper that extracts an explicit `line N` hint from prompt text
- feed that line number into the combined file+symbol exact-selector path
- keep line-hint parsing out of file-only, symbol-only, and repo-map flows
- add focused tests for:
  - combined file+symbol hint plus `line N` resolves an otherwise ambiguous symbol
  - combined file+symbol hint without `line N` still returns the stable ambiguity message
  - unrelated numbers in prompts do not get treated as line hints unless introduced by `line`

Expected touch points:

- `src/sidecar/handlers.rs`
- `tests/sidecar_integration.rs`

## Carry Forward

- preserve the current combined-hint routing from task 95
- keep the parser narrow: start with explicit `line N` only
- do not change the direct `get_symbol_context` exact-selector contract
