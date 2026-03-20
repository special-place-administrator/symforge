# Phase 1: Symbol Disambiguation - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Add kind-tier priority auto-disambiguation to `resolve_symbol_selector` so that C# class/constructor name collisions (and similar cross-kind ambiguities in other languages) resolve to the higher-priority symbol kind instead of returning an Ambiguous error.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion

All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints from codebase analysis:
- Fix lives in `resolve_symbol_selector` at `src/live_index/query.rs:383`
- Must only auto-disambiguate when candidates span DIFFERENT kind tiers
- Same-tier ambiguity must still return `SymbolSelectorMatch::Ambiguous`
- Priority tiers: class/struct/enum/interface/trait > module/namespace > fn/method/function > constructor > other

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `resolve_symbol_selector` at query.rs:383 — the target function
- `SymbolSelectorMatch` enum at query.rs:377 — `Found`, `Ambiguous`, `NotFound` variants
- `symbol_kind_priority` at search.rs:1038 — existing kind-to-score function for search ranking (may be reusable or serve as pattern)

### Established Patterns
- Symbol kind filtering via `symbol_kind` parameter already exists in the selector
- `is_filtered_name` at query.rs:542 already filters builtins
- Tests in query.rs use `LiveIndex` test fixtures

### Integration Points
- Every tool that resolves symbols by name goes through `resolve_symbol_selector`
- `get_symbol`, `get_symbol_context`, `replace_symbol_body`, `edit_within_symbol`, `delete_symbol` all call it
- No sidecar/hook changes needed

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
