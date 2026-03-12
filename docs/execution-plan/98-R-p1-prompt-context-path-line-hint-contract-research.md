# Research: P1 Prompt Context Path:Line Hint Contract

Related plan:

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [96-R-p1-prompt-context-symbol-line-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/96-R-p1-prompt-context-symbol-line-hint-contract-research.md)
- [97-T-p1-prompt-context-symbol-line-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/97-T-p1-prompt-context-symbol-line-hint-shell.md)

Goal:

- choose the smallest prompt-context follow-up contract that lets common `path:line` phrasing feed the combined file+symbol exact-selector lane without broadening number parsing globally

## Current Code Reality

After task 97, prompt-context can disambiguate combined file+symbol prompts when the prompt says `line N`.

That leaves one adjacent ergonomic gap:

- many coding prompts naturally include `src/file.rs:42`
- prompt-context already resolves `src/file.rs` as a file hint because the path is present verbatim
- but it ignores the `:42` suffix, so the line information is currently lost

The exact-selector path already accepts `symbol_line`. The missing piece is a narrow `path:line` bridge tied to the resolved file hint.

## Candidate Approaches

### Option 1: stop at `line N`

- smallest parser surface
- leaves a common coding prompt shape unsupported even though the needed information is already present

### Option 2: parse `path:line` only when it matches the resolved file hint

- keeps the parser narrow
- avoids treating arbitrary colon numbers as line hints
- composes naturally with the current exact file-hint path

### Option 3: parse any `:<number>` suffix in the prompt

- more permissive
- too risky because prompts may contain ports, ratios, timestamps, and unrelated numeric suffixes

## Decision: Add A Resolved-Path `path:line` Bridge

Recommendation:

- when prompt-context has already resolved a concrete file hint
- check whether the prompt contains that exact path followed by `:<line>`
- if so, feed that line into the combined file+symbol exact-selector flow

Keep existing behavior intact:

- `line N` support from task 97 stays
- file-only, symbol-only, and repo-map behavior stay unchanged
- combined prompts with no usable line hint still keep the stable ambiguity path

## Why This Is The Smallest Useful Slice

- it handles a common developer prompt shape without generalizing the parser
- it reuses the file hint that prompt-context already trusts
- it keeps colon-based line parsing anchored to a known path instead of scanning arbitrary numbers

## Recommended Next Implementation Slice

- extend prompt-context line-hint extraction to also look for `<resolved-path>:<line>`
- keep `line N` support as-is
- add focused tests for:
  - combined prompt with `src/file.rs:42` disambiguates via `symbol_line`
  - unrelated colon numbers do not get treated as line hints
  - existing `line N` behavior still works

Expected touch points:

- `src/sidecar/handlers.rs`
- `tests/sidecar_integration.rs`

## Carry Forward

- reuse the resolved file hint instead of adding global colon parsing
- preserve the exact-selector fallback when no usable line hint exists
- do not broaden this slice into full prompt grammar work
