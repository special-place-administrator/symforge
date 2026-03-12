# Handoff 2026-03-13: Phase A Done, Phase B+C Next

Repo: `tokenizor_agentic_mcp`, branch: `main`
Commit: aa15fed — Phase A fixes committed.

## What Was Done (Phase A)

4 quality fixes to existing tools, all tests green (718+):

1. **Callee noise filtering** — `callees_for_symbol` in `query.rs:1768` now calls `is_filtered_name()` to skip stdlib iterator methods (iter, collect, map, etc.) from `get_context_bundle` Callees section
2. **Self-referential deps** — `capture_context_bundle_view` in `query.rs:1409` excludes the target symbol's own name from type_names before resolving dependencies
3. **File path in context_bundle footer** — Added `file_path: String` to `ContextBundleFoundView` (query.rs:716), footer now renders `[fn, src/lib.rs:1-3, 41 bytes]`
4. **Truncation guidance** — `symbol_context_text` in `handlers.rs:760` now says `"showing N of M matches — use path or file to narrow"`

## What's Next

### Phase B: Build `trace_symbol` (NEW TOOL — highest remaining value)

**What it is:** Single-call semantic investigation tool that composes existing data into one response. Replaces the current 3-5 tool call pattern of search_symbols → get_context_bundle → find_dependents → get_file_context.

**Specified output sections** (from `02-P-workstreams-and-tool-surface.md` Workstream F):
- Symbol signature/body (from context_bundle)
- Enclosing path/module
- Callers (from context_bundle)
- Callees (from context_bundle, now noise-filtered)
- Type usages (from context_bundle)
- Type dependencies (from context_bundle, recursive depth-2)
- Dependents — files that import the target file (from `capture_find_dependents_view`)
- Nearby sibling symbols (from `capture_file_outline_view`, filter to same depth)
- Trait implementations (from `capture_find_implementations_view`)
- Git activity (from `git_temporal` module, same as `get_file_context` rendering)

**Proposed input:**
```rust
pub struct TraceSymbolInput {
    pub path: String,                    // File containing the symbol
    pub name: String,                    // Symbol name
    pub kind: Option<String>,            // Optional kind filter
    pub symbol_line: Option<u32>,        // Disambiguator
    pub sections: Option<Vec<String>>,   // Optional filter for output sections
}
```

**Touch points:**
- `src/live_index/query.rs` — new `capture_trace_symbol_view`, new view structs: `TraceSymbolView`, `SiblingSymbolView`, `GitActivityView`
- `src/protocol/tools.rs` — new `TraceSymbolInput`, new `trace_symbol` method on `TokenizorServer`
- `src/protocol/format.rs` — new `trace_symbol_result_view` formatter
- Tests in tools.rs and live_index_integration.rs

**Key design rule:** Compose existing capture methods, don't duplicate logic. `trace_symbol` wraps `capture_context_bundle_view` + `capture_find_dependents_view` + `capture_find_implementations_view` + file outline + git temporal.

**Disambiguation:** Reuse existing `resolve_symbol_selector` — same as `get_context_bundle`.

**Plan references:**
- `docs/execution-plan/02-P-workstreams-and-tool-surface.md` — Workstream F, trace_symbol spec
- `docs/execution-plan/04-P-phase-plan.md` — Phase 7
- `docs/execution-plan/05-P-validation-and-backlog.md` — P2 backlog

### Phase C: `inspect_match` + Ranking (lower priority)

**inspect_match:** Bridge tool that converts a text search hit into structured context. Input: path + line + optional context. Output: numbered excerpt + enclosing symbol + sibling symbols. The existing `get_file_content` `around_match` mode partially covers this but lacks enclosing symbol annotation.

**Module/path locality ranking:** Results from `find_references` and `search_symbols` aren't ranked by proximity to query file. Closer results should sort higher. Touches `build_find_references_view` and `search_symbols_with_options` in `search.rs`.

## v2 Roadmap Status

All 5 items from `memory/project_v2_roadmap.md` are DONE:
1. ✅ Recursive type resolution in get_context_bundle
2. ✅ find_implementations tool
3. ✅ Richer get_file_context (imports/exports/used-by/git)
4. ✅ Mermaid graph output for find_dependents
5. ✅ Git churn metadata

## Original Execution Plan Status

- Phases 0-4 + P1 backlog: DONE (137 tasks)
- Phase 5 (stable symbol identity): Partially done via exact selectors
- Phase 6 (noise/ranking): Partially done, locality ranking still missing
- **Phase 7 (trace_symbol, inspect_match): NOT STARTED — this is Phase B+C**
- Phase 8 (prompt/resource alignment): Partially done
- Phase 9 (deeper decisions): Future gate

## Resume Prompt

```
Resume work on tokenizor_agentic_mcp.

Read:
1. docs/summaries/handoff-2026-03-13-phase-a-done-phase-bc-next.md
2. docs/execution-plan/02-P-workstreams-and-tool-surface.md (Workstream F for trace_symbol spec)

Task: Build `trace_symbol` tool (Phase B from the handoff).

Design rules:
- Compose existing capture methods — do NOT duplicate query logic
- Reuse resolve_symbol_selector for disambiguation (same as get_context_bundle)
- New view structs go in query.rs, new input struct + tool method in tools.rs, formatter in format.rs
- The tool should register alongside existing tools (update the tools_registered count test)
- All existing tests must stay green

Implementation order:
1. Add view structs to query.rs (TraceSymbolView, SiblingSymbolView, GitActivityView)
2. Add capture_trace_symbol_view composing existing captures
3. Add TraceSymbolInput and trace_symbol method to tools.rs
4. Add trace_symbol_result_view formatter to format.rs
5. Add focused tests
6. Run cargo test, fix any failures
7. Commit

After trace_symbol is done, Phase C (inspect_match + locality ranking) is optional follow-up — ask user before starting it.
```
