# Research: P1 Prompt Context Basename:Line Hint Contract

Related plan:

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [98-R-p1-prompt-context-path-line-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/98-R-p1-prompt-context-path-line-hint-contract-research.md)
- [99-T-p1-prompt-context-path-line-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/99-T-p1-prompt-context-path-line-hint-shell.md)

Goal:

- choose the smallest prompt-context follow-up contract that lets unique basename-derived `file.rs:line` prompts feed the combined file+symbol exact-selector lane without weakening file disambiguation

## Current Code Reality

After task 99, prompt-context supports:

1. exact `line N`
2. exact `<resolved-path>:<line>`

That still leaves one nearby developer prompt shape unsupported:

- `db.rs:2 connect`

This prompt can already resolve a file hint when the basename is unique, because `find_prompt_file_hint` returns the matching full path.

The current gap is narrower than general colon parsing:

- prompt-context knows which full path the basename resolved to
- but the colon-based parser only accepts the full resolved path, not the basename token that the user actually typed

## Candidate Approaches

### Option 1: stop at full-path `path:line`

- safest parser boundary
- leaves a common prompt shape unsupported even when basename resolution is already unique and trusted

### Option 2: accept `basename:line` only after unique basename resolution

- reuses the existing basename-resolution guardrail
- avoids treating arbitrary `file.rs:42` tokens as valid when the basename is ambiguous
- stays aligned with the current resolved-file-hint flow

### Option 3: accept any `*.ext:line` token globally

- more permissive
- too risky because it bypasses the current basename ambiguity checks and broadens prompt parsing too much

## Decision: Add A Unique-Basename `basename:line` Bridge

Recommendation:

- if prompt-context resolves a file hint through a unique basename match
- allow `<basename>:<line>` to feed `symbol_line`
- if the basename is ambiguous and no full path is present, keep current fallback behavior

Keep everything else unchanged:

- exact full-path `path:line` support from task 99 stays
- explicit `line N` support stays
- combined prompts with no usable line hint still keep the stable ambiguity path

## Why This Is The Smallest Useful Slice

- it improves a common prompt form without reopening general filename parsing
- it relies on the existing basename disambiguation logic rather than replacing it
- it keeps the parser honest: only uniquely resolved basenames become actionable

## Recommended Next Implementation Slice

- extend prompt-context line-hint extraction to also accept `<basename>:<line>` when the file hint came from a unique basename match
- keep exact-path and `line N` behavior unchanged
- add focused tests for:
  - unique basename `file.rs:42` disambiguates combined prompt routing
  - ambiguous basenames do not activate basename-derived `:line`
  - existing exact-path `path:line` behavior still works

Expected touch points:

- `src/sidecar/handlers.rs`
- `tests/sidecar_integration.rs`

## Carry Forward

- only activate basename-derived `:line` after unique basename resolution
- preserve exact-path `path:line` behavior
- avoid broadening this slice into generic filename token parsing
