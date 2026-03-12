# Research: P1 Prompt Context Module Alias File Hint Contract

Related plan:

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [106-R-p1-prompt-context-qualified-module-alias-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/106-R-p1-prompt-context-qualified-module-alias-contract-research.md)
- [107-T-p1-prompt-context-qualified-module-alias-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/107-T-p1-prompt-context-qualified-module-alias-shell.md)

Goal:

- decide whether prompt-context should accept exact qualified module aliases as file hints even when the prompt omits `:line`

## Current Code Reality

After task 107, prompt-context accepts exact module aliases like:

- `crate::db:2 connect`

That still leaves one adjacent prompt shape unsupported:

- `crate::db connect`

This is common when the symbol name is already unique inside the target file or when the user expects the module name to act like the existing exact path file hint.

## Candidate Approaches

### Option 1: require `:line` for all module aliases

- simplest current boundary
- leaves a common prompt shape on the fallback path

### Option 2: accept exact qualified module aliases as file hints without `:line`

- consistent with existing exact path and basename file hints
- still deterministic when the alias matches one indexed module path exactly
- requires an exact boundary check so `crate::dbx` does not activate `crate::db`

### Option 3: accept fuzzy module-prefix hints

- more permissive
- too risky because it overlaps with symbol and stem parsing

## Decision: Add An Exact No-Line Module File-Hint Bridge

Recommendation:

- accept a qualified module alias as a file hint when it matches one indexed module path exactly
- require an explicit namespace separator and exact prompt boundary
- keep `:line` as the path for duplicate same-name symbols inside the chosen file

Keep current behavior intact:

- `crate::db:2` still feeds the exact-selector line path
- partial aliases like `crate::dbx` or `crate::d` do not activate a file hint
- fallback remains the current name-only symbol-context path when no exact module file hint exists

## Why This Is The Smallest Useful Slice

- it reuses the module-path matcher added in task 107
- it aligns module aliases with the existing file-hint behavior used by exact paths
- it stops short of fuzzy module guessing

## Recommended Next Implementation Slice

- extend prompt-context file-hint matching so an exact qualified module alias can activate without `:line`
- add a boundary-aware prompt matcher to distinguish `crate::db` from longer prefixes such as `crate::dbx`
- add focused tests for:
  - `crate::db connect` routes through the exact file+symbol lane
  - `crate::dbx connect` does not activate a file hint
  - existing `crate::db:2` behavior remains intact

Expected touch points:

- `src/sidecar/handlers.rs`
- `tests/sidecar_integration.rs`

## Carry Forward

- keep matching exact and explicitly qualified
- preserve the line-hint lane and fallback semantics
- defer fuzzy or implicit module aliases unless later usage justifies them
