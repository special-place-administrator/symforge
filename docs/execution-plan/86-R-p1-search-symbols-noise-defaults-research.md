# Research: P1 Search Symbols Noise Defaults

Related plan:

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [67-R-phase1-dual-lane-option-defaults-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/67-R-phase1-dual-lane-option-defaults-research.md)
- [68-T-phase1-explicit-current-tool-option-defaults-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/68-T-phase1-explicit-current-tool-option-defaults-shell.md)
- [84-R-p1-search-symbols-scope-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/84-R-p1-search-symbols-scope-contract-research.md)
- [85-T-p1-search-symbols-scope-filter-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/85-T-p1-search-symbols-scope-filter-shell.md)

Goal:

- choose the smallest public `search_symbols` noise contract that suppresses common generated/test floods by default without trapping callers behind hidden filtering

## Current Code Reality

Current public `search_symbols` now exposes:

- `query`
- `kind`
- `path_prefix`
- `language`
- `limit`

The internal substrate already supports:

- `NoisePolicy`
- generated/test/vendor file classification
- code-lane defaults through `SymbolSearchOptions`

But task 85 intentionally preserved:

- `NoisePolicy::permissive()`
- no public generated/test override knobs

That means the current symbol shell still returns noisy hits from generated and test paths even though the codebase already has the metadata needed to suppress them.

## Decision: Match The Current `search_text` Noise Contract

Recommendation:

- change current-code `search_symbols` defaults to:
  - `include_generated = false`
  - `include_tests = false`
  - `include_vendor = true`
- add public `include_generated: Option<bool>` and `include_tests: Option<bool>` to `SearchSymbolsInput`
- do not add `include_vendor` in this slice

Why:

- this matches the stabilized `search_text` code-lane contract
- it removes the highest-volume low-signal symbol noise by default
- it still lets callers recover generated/test results explicitly when needed
- it keeps vendor behavior unchanged, avoiding an extra semantic jump without a dedicated task

## Why Defaults Alone Are Not Enough

Changing defaults without public escape hatches would be smaller in code, but worse in product behavior.

Reason:

- callers would silently lose generated/test symbol visibility
- recovering those hits would require shell fallback or future follow-on work
- backlog success is explicitly about reducing shell escape pressure, not just hiding noise

So the smallest safe slice is:

- default suppression
- explicit opt-in overrides for the two suppressed classes

## Why `include_vendor` Stays Deferred

Recommendation:

- leave vendor files visible for now
- keep `include_vendor` internal only

Why:

- current `search_text` already keeps vendor visible
- backlog wording calls out generated/test suppression specifically
- vendor suppression is more likely to need language- or repo-specific tuning later

## Output And Ranking Contract

Recommendation:

- keep formatter output unchanged
- keep current tier ordering and match ranking unchanged

Why:

- this slice is about candidate eligibility, not presentation or ranking
- tying it to ranking changes would make regressions harder to attribute

## Recommended Next Implementation Slice

- extend `SearchSymbolsInput` with `include_generated` and `include_tests`
- change `SymbolSearchOptions::for_current_code_search` to default-hide generated/test noise while keeping vendor visible
- update the tool-layer adapter to honor explicit opt-in overrides
- add focused tests for:
  - default suppression of generated/test symbol hits
  - explicit `include_generated=true`
  - explicit `include_tests=true`
  - preservation of existing scope/language/kind behavior

Expected touch points:

- `src/protocol/tools.rs`
- `src/live_index/search.rs`

## Carry Forward

- keep this slice separate from ranking refinements
- keep this slice separate from exact-symbol identity work
- preserve vendor visibility until a later task justifies changing it
