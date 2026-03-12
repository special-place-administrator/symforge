# Research: P1 Prompt Context Symbol And File Hint Contract

Related plan:

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [92-R-p1-get-symbol-context-exact-selector-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/92-R-p1-get-symbol-context-exact-selector-contract-research.md)
- [93-T-p1-get-symbol-context-exact-selector-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/93-T-p1-get-symbol-context-exact-selector-shell.md)

Goal:

- choose the smallest prompt-context contract that lets file hints and symbol hints compose through the new exact-selector symbol-context lane without reopening broader prompt parsing work

## Current Code Reality

Current prompt-context heuristics are:

1. file hint => file outline
2. symbol hint => symbol context
3. repo-map intent => repo map

That means a prompt containing both:

- a file hint
- a symbol hint

currently returns only the file outline and drops the symbol follow-up entirely.

Now that `get_symbol_context` supports exact-selector inputs, prompt-context has enough substrate to do better when both hints are present.

## Candidate Approaches

### Option 1: keep file hint precedence unchanged

- no code churn
- ignores the new exact-selector lane
- keeps losing useful symbol-specific context when the prompt already names both the file and the symbol

### Option 2: if both hints exist, route through exact-selector symbol context

- reuses the new exact-selector work directly
- preserves current behavior when only one hint exists
- if the selected file has duplicate same-name symbols, the user gets a stable ambiguity message instead of a noisy global-name summary

### Option 3: create a prompt-only fused renderer

- potentially richer end state
- too large for the next slice because it creates a third presentation path for the same concept

## Decision: Fuse File Hint And Symbol Hint Through `symbol_context_text`

Recommendation:

- if a prompt has both a file hint and a symbol hint, prefer `symbol_context_text` with:
  - `name`
  - `path` set from the file hint
  - `file` left unset
- if only a file hint exists, keep returning outline
- if only a symbol hint exists, keep returning name-only symbol context
- leave repo-map fallback untouched

## Why This Is The Smallest Useful Slice

- it reuses the exact-selector infrastructure already built
- it keeps prompt-context compact and heuristic-based
- it avoids inventing a second prompt-specific symbol response path
- it naturally inherits stable ambiguity messaging from the exact-selector symbol-context flow

## Recommended Next Implementation Slice

- update prompt-context routing so combined file+symbol hints prefer exact-selector symbol context
- preserve existing behavior for file-only, symbol-only, and repo-map prompts
- add focused tests for:
  - file hint only still returns outline
  - symbol hint only still returns symbol context
  - file hint plus symbol hint now routes to exact-selector symbol context
  - ambiguous same-name symbols in the hinted file return the stable ambiguity message

Expected touch points:

- `src/sidecar/handlers.rs`
- possibly `src/protocol/tools.rs` only if prompt-context proxy expectations need coverage

## Carry Forward

- keep this slice separate from deeper prompt parsing work
- preserve token-budget behavior
- do not broaden prompt-context into a general natural-language parser
