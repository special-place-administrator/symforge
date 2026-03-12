# Research: P1 Prompt Context Qualified Symbol Alias Contract

Related plan:

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [106-R-p1-prompt-context-qualified-module-alias-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/106-R-p1-prompt-context-qualified-module-alias-contract-research.md)
- [109-T-p1-prompt-context-module-alias-file-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/109-T-p1-prompt-context-module-alias-file-hint-shell.md)

Goal:

- decide whether prompt-context should add an exact qualified-symbol lane after finishing exact module and file hints

## Current Code Reality

After task 109, prompt-context accepts:

1. exact paths and basenames
2. extensionless file aliases with and without `:line`
3. exact qualified module aliases with and without `:line`

That still leaves one adjacent prompt shape unsupported:

- `crate::db::connect`

The current flow can only reach exact selection by combining a file hint with a separate symbol hint token, not by consuming the fully qualified symbol path directly.

## Candidate Approaches

### Option 1: stop at file and module hints

- simplest current boundary
- leaves a precise prompt shape unhandled

### Option 2: accept exact qualified symbol aliases

- deterministic if the alias must exactly match one indexed file/module plus one symbol name
- reuses the exact-selector surface already built for `symbol_line`
- keeps the lane distinct from fuzzy symbol search

### Option 3: accept fuzzy qualified symbol prefixes

- more permissive
- too risky because it blurs into generic symbol-path guessing

## Decision: Add An Exact Qualified-Symbol Bridge

Recommendation:

- accept fully qualified symbol aliases only when they exactly match a derived file/module path plus symbol name
- require explicit namespace separators so this lane stays distinct from simple symbol tokens
- keep line hints available for duplicate same-name symbols inside the matched file

Keep current behavior intact:

- file and module hints remain valid entry points
- partial qualified symbols still fall back to current prompt-context behavior
- no fuzzy namespace search is introduced

## Why This Is The Smallest Useful Slice

- it supports a precise prompt shape users already type
- it composes directly with the exact-selector machinery already in place
- it avoids speculative semantic parsing

## Recommended Next Implementation Slice

- extend prompt-context to detect exact qualified symbol aliases like `crate::db::connect`
- route the qualified module portion through the existing exact file-hint lane and the trailing symbol through exact selection
- add focused tests for:
  - exact qualified symbol aliases disambiguate combined prompts
  - partial qualified symbols do not activate exact selection
  - existing module alias routes keep working

Expected touch points:

- `src/sidecar/handlers.rs`
- `tests/sidecar_integration.rs`

## Carry Forward

- keep qualified symbol matching exact and explicitly namespaced
- preserve current file/module hint behavior and fallbacks
- defer fuzzy qualified-symbol search unless later usage shows clear value
