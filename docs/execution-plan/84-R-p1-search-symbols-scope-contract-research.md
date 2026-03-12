# Research: P1 Search Symbols Scope Contract

Related plan:

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [04-P-phase-plan.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/04-P-phase-plan.md)
- [66-T-phase1-shared-query-option-struct-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/66-T-phase1-shared-query-option-struct-shell.md)
- [68-T-phase1-explicit-current-tool-option-defaults-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/68-T-phase1-explicit-current-tool-option-defaults-shell.md)
- [84-T-p1-search-symbols-scope-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/84-T-p1-search-symbols-scope-contract-research.md)

Goal:

- choose the smallest public `search_symbols` scope contract that materially reduces shell fallback without mixing in exact-symbol identity or output redesign

## Current Code Reality

Current public `search_symbols` exposes only:

- `query`
- `kind`

The internal substrate already has:

- `SymbolSearchOptions.path_scope`
- `SymbolSearchOptions.search_scope`
- `SymbolSearchOptions.result_limit`
- `SymbolSearchOptions.noise_policy`

And the indexed file model already carries:

- canonical language ids on files
- generated/test/vendor classification tags

That means the public shell is behind the internal substrate, similar to where `search_text` was before Phase 3.

## Decision: Path Prefix, Language, And Limit First

Recommendation:

- add `path_prefix: Option<String>`
- add `language: Option<String>`
- add `limit: Option<u32>`

Keep existing fields:

- `query`
- `kind`

Why:

- these three fields cover the most common symbol-search shell fallback patterns
- they map directly onto the current internal query vocabulary
- they avoid prematurely coupling `search_symbols` to exact-symbol identity work from Phase 5

## Scope And Lane Semantics

Recommendation:

- keep the first scoped shell code-lane only
- preserve the current `kind` filter exactly as-is

Why:

- `search_symbols` is inherently a semantic code search path today
- widening it to text-lane or identity-aware search would blur phase boundaries

## Noise Policy Recommendation

Recommendation:

- keep current `search_symbols` defaults noise-permissive in the first scope shell
- do not add public `include_generated` / `include_tests` knobs in this slice

Why:

- task 67 explicitly kept current public search adapters on `SearchScope::Code` plus `NoisePolicy::permissive()`
- the backlog lists generated/test suppression as a separate P1 item
- mixing scope filters and noise-default changes in the same shell would make regressions harder to attribute

## Language Contract

Recommendation:

- accept the same canonical language names already used by `search_text`
- defer short aliases such as `ts` and `js`

Why:

- this keeps the first shell consistent with the recently stabilized `search_text` contract
- there is already a clear parser shape to reuse

## Limit Contract

Recommendation:

- default to the current effective limit of 50 hits
- allow callers to request a smaller or larger limit, but cap the first public shell at 100

Why:

- this preserves today's default behavior
- it adds real caller control without opening the door to unbounded output
- it keeps the first scoped shell additive rather than a ranking overhaul

## Output Contract

Recommendation:

- keep the current match-tier headers and line format unchanged
- do not change ranking beyond whatever path/language filtering removes from the candidate set

Why:

- output redesign belongs later, especially once exact-symbol follow-up identity lands
- the current formatter is readable and already tested

## Recommended Next Implementation Slice

- extend `SearchSymbolsInput` with `path_prefix`, `language`, and `limit`
- extend `SymbolSearchOptions` with a language filter
- add a small adapter in `src/protocol/tools.rs` that normalizes the new fields into `SymbolSearchOptions`
- keep default search behavior code-lane and noise-permissive
- add focused tests for:
  - path-prefix scoping
  - language filtering
  - explicit result limits
  - preservation of current kind filtering

Expected touch points:

- `src/protocol/tools.rs`
- `src/live_index/search.rs`
- `src/protocol/format.rs`

## Carry Forward

- keep this slice separate from generated/test suppression defaults
- keep this slice separate from exact-symbol identity and follow-up addressing
- preserve current formatter output while upgrading the public filter contract
