---
title: "fix: Address SymForge bugs surfaced in v7.5 review"
type: fix
status: active
date: 2026-04-19
---

# fix: Address SymForge bugs surfaced in v7.5 review

## Overview

During a tool-by-tool test pass on v7.5.0, five behavioral bugs and one documentation-consistency issue surfaced. This plan fixes the five bugs and clarifies the documentation issue without expanding scope beyond what the test pass exposed.

## Problem Frame

SymForge v7.5 generally works well, but a session exercising every discovery, inspection, reference, and edit tool found:

1. `get_repo_map` "Key types" section lists files from other indexed projects (e.g., `C:\AI_STUFF\PROGRAMMING\octogent\apps\api\tests\hookDrivenBootstrap.test.ts`) — cross-workspace path leak.
2. `analyze_file_impact` reports `Previously had 0 symbols` for a file that had symbols before deletion, because the watcher races ahead of the call and removes the index entry first.
3. `analyze_file_impact` lists callers under `Callers to review` using name-only matching, producing false positives for common names like `new()`.
4. `replace_symbol_body` silently swallows the target symbol's attached doc comment when `new_body` does not include one, because the splice range starts at `sym.effective_start()`.
5. `search_text structural=true` returns "No matches" with no diagnostic distinguishing a zero-result query from an invalid ast-grep pattern.
6. `trace_symbol` is referenced in `README.md` / `CLAUDE.md` / `SYMFORGE_TOOL_NAMES` but has no `#[tool]` attribute — it was consolidated into `get_symbol_context(sections=[...])`. The consolidation is correct; the surface area is inconsistent.

## Requirements Trace

- R1. `get_repo_map` must not leak paths outside the active workspace.
- R2. `analyze_file_impact` on a missing file must report useful state when the watcher already purged the index entry.
- R3. `analyze_file_impact` must not report generic-name callers from unrelated types.
- R4. `replace_symbol_body` must preserve attached doc comments when `new_body` does not supply its own.
- R5. `search_text structural=true` must distinguish pattern parse failures from zero-result queries.
- R6. `trace_symbol`'s documented surface must match its implemented surface.

## Scope Boundaries

- Not changing the structural search engine (`ast-grep`) itself — only the diagnostic layer around it.
- Not removing `trace_symbol` from `SYMFORGE_TOOL_NAMES` in this plan. The consolidate-mcp-tool runbook says to wait one release cycle; we respect that.
- Not redesigning `analyze_file_impact`'s caller-review section. Only scope-filtering existing name-based matches.
- Not adding a new config flag or feature gate for any of these fixes.

### Deferred to Separate Tasks

- Formal removal of `trace_symbol` from `SYMFORGE_TOOL_NAMES` and the init-time allowed-tools list: separate PR after one release cycle, per `docs/runbooks/consolidate-mcp-tool.md:71`.
- Broader review of name-only lookups elsewhere in the codebase (outside `analyze_file_impact` / `handle_edit_impact`): separate refactor pass.

## Context & Research

### Relevant Code and Patterns

- `src/sidecar/handlers.rs:1277-1391` — `repo_map_text`. Header loop at L1299 already skips absolute paths (`if path.contains(':') || path.starts_with('/')`). Key-types loop at L1338-1378 does not apply the same filter. Fix is to reuse the same guard.
- `src/sidecar/handlers.rs:748-1004` — `handle_edit_impact`. L853-856 captures `prev_symbol_count` from `state.index`, but the file-watcher may have removed the entry before this line runs. L864 prints `Previously had N symbols` unconditionally.
- `src/sidecar/handlers.rs` caller-review section (same function) — currently matches on symbol name only. `src/protocol/edit.rs:2274-2329` `detect_stale_references` already does type-scoped filtering and is used by `replace_symbol_body` via `find_parent_impl_type` (`src/protocol/tools.rs:6530`). Port the same pattern here.
- `src/protocol/tools.rs:6419-6574` — `replace_symbol_body`. L6508 uses `sym.effective_start()`, then L6515 calls `edit::extend_past_orphaned_docs`. `effective_start` includes the attached doc byte range, so the splice overwrites attached docs even when `new_body` has no doc.
- `src/protocol/edit.rs:407-450` — `extend_past_orphaned_docs`. Logic only fires when `sym.doc_byte_range.is_some()` is false, i.e., only for *orphaned* (blank-line-separated) doc comments. The attached-doc case is handled implicitly by `effective_start()`.
- `src/protocol/tools.rs:3451-3500` — `search_text`. Structural branch at L3465 calls `search::search_structural` and renders the result. No pre-validation of the ast-grep pattern; zero-result and parse-failure produce the same "No matches" output.
- `src/cli/init.rs:262-294` — `SYMFORGE_TOOL_NAMES`. Contains `mcp__symforge__trace_symbol` despite the handler having no `#[tool]` attribute.
- `src/daemon.rs:1562-1580` — `trace_symbol` backward-compat alias in `execute_tool_call`. This is the intended surface after consolidation.

### Institutional Learnings

- `docs/runbooks/consolidate-mcp-tool.md` — canonical 7-step consolidation pattern. Step 5 explicitly warns: "Defer this step by at least one release cycle" before removing from `SYMFORGE_TOOL_NAMES`. Informs scope boundary on R6.
- `docs/plans/2026-04-02-symforge-reliability-regression-safe-plan.md` — prior reliability plan, precedent for how to scope a multi-bug fix document in this repo.

### External References

- None required. All issues are internal to the repo and already well-scoped by the investigation above.

## Key Technical Decisions

- **Repo-map filter reuse.** The key-types loop should use the same absolute-path skip the header loop already has. Factor the check into a private helper `is_intra_workspace_path(&str) -> bool` in `src/sidecar/handlers.rs` and call it from both loops. Rationale: single source of truth, matches the existing design intent (the header loop's comment at L1299 names the exact octogent-style leak we hit).
- **`analyze_file_impact` missing-file wording.** Change the `NotFound` branch wording to "Status: not found on disk — no index record remains (may have been removed by watcher)" when `prev_symbol_count == 0`. Keep "Previously had N symbols" only when we genuinely captured a non-zero pre-count. Rationale: honest reporting is cheaper than caching pre-delete state, and the race is inherent to an event-driven watcher.
- **Caller-review type scoping.** Reuse `find_parent_impl_type` + the name-match-plus-parent-type predicate already used by `detect_stale_references`. For `fn new` changes inside `impl MathMachine`, only report callers in files that mention `MathMachine`. Rationale: the filter already exists and is battle-tested; we just apply it one level earlier.
- **Doc-comment preservation.** Change `replace_symbol_body` to splice from `sym.byte_range.0` (the `pub fn ...` line, without attached docs) unless `new_body`'s first non-blank line is itself a doc comment. When `new_body` starts with a doc comment, keep current behavior (splice from `effective_start` so the user's new docs replace the old). Rationale: preserves the least-surprising default ("replace body means replace body") while keeping the duplicate-doc guard when the user opts in by supplying docs.
- **Structural search diagnostics.** Pre-validate the ast-grep pattern by calling `ast_grep_core`'s pattern parser before running the search. On parse error, return `Error: structural pattern failed to parse: <reason>`. On parse OK with 0 matches, return the existing "No matches" text plus a diagnostic footer `Pattern parsed OK; 0 AST matches in N searchable files.` Rationale: zero tool-usage ambiguity for the caller.
- **`trace_symbol` docs alignment.** Update `README.md` and root `CLAUDE.md` to point users at `get_symbol_context(sections=[...])` instead of `trace_symbol`. Leave `SYMFORGE_TOOL_NAMES` alone (per runbook). Rationale: minimize surface drift in user-facing docs without prematurely removing the alias.

## Open Questions

### Resolved During Planning

- Is `trace_symbol` really missing? — No. It was consolidated into `get_symbol_context`; the alias in `daemon.rs` still routes the old name. The doc inconsistency is the only issue.
- Is the doc-swallow behavior intentional? — Partially. The intent is to prevent duplicate docs when users supply new docs. The current implementation is over-eager (swallows even when no new doc supplied). The fix preserves the intent in the narrower case.

### Deferred to Implementation

- Exact method for the "doc starts with a doc comment" check in `new_body` — prefix sniffing (`///`, `//!`, `/** ... */`, `# ` for Python, JSDoc block) is fine but the implementer may choose to reuse a helper from `extend_past_orphaned_docs`'s comment-prefix list rather than duplicate it.
- Whether `ast_grep_core::Pattern::new` is directly callable or whether the project uses a language-specific pattern factory. Implementer should check `src/parsing/` for existing ast-grep integration and reuse the same entry point.

## Implementation Units

- [x] **Unit 1: Fix cross-project leak in `repo_map_text` key-types loop**

**Goal:** Key-types section of `get_repo_map` no longer lists paths from other indexed workspaces.

**Requirements:** R1

**Dependencies:** None

**Files:**
- Modify: `src/sidecar/handlers.rs`
- Test: `src/sidecar/handlers.rs` (inline `#[cfg(test)]` module) or the relevant outline test in `tests/`

**Approach:**
- Introduce a small private helper that centralizes the "intra-workspace" path check currently inlined at L1299 (`path.contains(':') || path.starts_with('/')`).
- Apply the helper in the key-types loop (L1344) so absolute paths from other repos are excluded before the symbol is added to `entry_points`.

**Patterns to follow:**
- Existing header-loop guard at `src/sidecar/handlers.rs:1299`.

**Test scenarios:**
- Happy path: `repo_map_text` called on a state whose index contains both relative and absolute paths returns a key-types block containing only the relative-path entries.
- Edge case: index contains only absolute paths — key-types block is omitted (or shown with zero entries), not rendered with foreign types.
- Regression: existing `repo_map_text` golden tests pass unchanged.

**Verification:**
- New test fails on current `main`, passes after change.
- Manual `get_repo_map` against a multi-indexed SymForge daemon shows only the active workspace's symbols.

- [x] **Unit 2: Honest wording for post-watcher-race `analyze_file_impact`**

**Goal:** When the file is already absent from the index at the time `analyze_file_impact` runs, the response should not claim "Previously had 0 symbols" as if that were ground truth.

**Requirements:** R2

**Dependencies:** None

**Files:**
- Modify: `src/sidecar/handlers.rs` (lines 850-867)
- Test: inline test in the same file or `tests/`

**Approach:**
- When `prev_symbol_count == 0` in the `NotFound` branch, emit: `Status: not found on disk — no index record remains (may have been removed by watcher).`
- When `prev_symbol_count > 0`, preserve the existing message.

**Patterns to follow:**
- The current message format at `src/sidecar/handlers.rs:864`.

**Test scenarios:**
- Happy path (index has the file): delete file on disk, call `analyze_file_impact` before the watcher fires — message reads `Previously had N symbols.`
- Edge case (watcher race): simulate watcher-first removal by calling `state.index.remove_file(path)` before the handler, then call the handler — message reads `no index record remains`.
- Error path: directory passed instead of file — existing error handling unchanged.

**Verification:**
- New test covering the zero-count branch.
- Existing tests for the non-zero branch still pass.

- [x] **Unit 3: Type-scoped caller-review in `handle_edit_impact`**

**Goal:** `analyze_file_impact`'s `Callers to review` section should not list callers that reference a same-named method on a different type.

**Requirements:** R3

**Dependencies:** None (parallel with Units 1 and 2)

**Files:**
- Modify: `src/sidecar/handlers.rs` (caller-review section inside `handle_edit_impact`)
- Possibly extract/reuse: `src/protocol/edit.rs::find_parent_impl_type`, `src/protocol/edit.rs::detect_stale_references` filter predicate
- Test: inline test in `src/sidecar/handlers.rs` or dedicated fixture in `tests/`

**Approach:**
- When iterating `changed_post` symbols, look up each changed symbol's parent impl/class (reusing `find_parent_impl_type` or an equivalent query against the updated indexed file).
- When listing callers of a changed symbol, filter reference files with a type-presence check (the file must also reference the parent type name).
- Generic-name methods with no parent type (module-level functions) keep the current behavior.

**Patterns to follow:**
- `src/protocol/edit.rs:2274-2329` — `detect_stale_references` uses `parent_type: Option<&str>` to filter.
- `src/protocol/tools.rs:6530` — call site that derives `parent_type`.

**Test scenarios:**
- Happy path: changing `fn new` inside `impl Foo` only reports callers in files that also reference `Foo`.
- Edge case: module-level `fn helper` (no parent type) — report all callers, same as today.
- Edge case: multiple impls of the same method name in different types — each filters to its own type's call sites.
- Integration: regression test on a fixture with `Foo::new` and `Bar::new` proves no cross-type leak.

**Verification:**
- Fixture test proves cross-type callers are excluded.
- Existing caller-review tests still pass for module-level symbols.

- [x] **Unit 4: Preserve attached docs in `replace_symbol_body` unless `new_body` supplies its own**

**Goal:** `replace_symbol_body` stops silently deleting the target's doc comment when the user provides a bodies-only replacement.

**Requirements:** R4

**Dependencies:** None

**Files:**
- Modify: `src/protocol/tools.rs` (around lines 6508-6525 in `replace_symbol_body`)
- Possibly modify: `src/protocol/edit.rs` to expose a `new_body_starts_with_doc_comment(body: &str, language: &Language) -> bool` helper
- Test: existing `test_replace_symbol_body_preserves_indentation` file (add new cases adjacent to the canonical tests), e.g., `test_replace_symbol_body_preserves_attached_docs`

**Approach:**
- Before computing `line_start`, inspect `new_body`'s first non-blank line. If it is a doc-comment prefix for the file's language (`///`, `//!`, `/**`, `# `, `* `, language-specific JSDoc), use `sym.effective_start()` as today. Otherwise, start the splice at the symbol's signature line (after any attached docs) by using `sym.byte_range.0` and its own line start, skipping `extend_past_orphaned_docs`.
- Keep `extend_past_orphaned_docs` for the `delete_symbol` path unchanged — that tool is expected to remove docs with the symbol.

**Patterns to follow:**
- `src/protocol/edit.rs:407-450` — existing doc-prefix detection list to mirror for the "starts with doc" check.
- Existing tests at `tests`/`src/protocol/tools.rs` that exercise `replace_symbol_body` indentation and orphan-doc handling.

**Test scenarios:**
- Happy path A (no new doc): symbol has `/// foo`, `new_body` is `pub fn foo() {}`. Existing doc is preserved above the new body.
- Happy path B (new doc supplied): symbol has `/// old`, `new_body` is `/// new\npub fn foo() {}`. Old doc is replaced by the new one (no duplicate).
- Edge case (orphaned doc with blank line): symbol has `/// detached`, blank line, `pub fn foo()`. `new_body` has no doc. Current orphan-extension behavior preserved (no regression from current behavior because orphan docs are not considered attached).
- Edge case (attribute on symbol): `#[inline]\npub fn foo()`. `new_body` is `pub fn foo()`. Attribute preserved.
- Integration: TypeScript JSDoc case — `/** doc */\nexport function foo() {}`, `new_body` is `export function foo() {}`. JSDoc preserved.

**Verification:**
- All new tests above pass.
- Existing `test_replace_symbol_body_*` tests pass unchanged.

- [x] **Unit 5: Diagnostic output for `search_text structural=true`**

**Goal:** Distinguish pattern parse failures from zero-match results, so the caller knows which state they are in.

**Requirements:** R5

**Dependencies:** None

**Files:**
- Modify: `src/protocol/tools.rs` (structural branch around L3465-3495) or the underlying `src/live_index/search.rs` `search_structural` function, whichever is the right place to attach pattern validation.
- Test: inline test in the same module, or fixture test in `tests/`.

**Approach:**
- Call the ast-grep pattern parser up-front. On parse error, return `Error: structural pattern failed to parse: <reason>. Hint: metavariables use $VAR, multi-node wildcards use $$$.`
- On parse OK with 0 results, append a diagnostic footer to the existing no-match message: `Structural pattern parsed OK; 0 AST matches across N searchable files. Consider widening scope (include_tests=true) or simplifying the pattern.`

**Patterns to follow:**
- Existing error-return style in the same function (e.g., `"Error: \`query\` is required for structural search."` at L3468).

**Test scenarios:**
- Error path: malformed pattern (e.g., unterminated `$$$`) returns the parse-error string with a specific hint.
- Happy path (zero matches on a valid pattern): returns existing "No matches" text with the `Structural pattern parsed OK` footer.
- Happy path (matches exist): output unchanged from today.
- Regression: the structural tests currently in `tests/` continue to pass.

**Verification:**
- New tests cover the parse-error and zero-match-with-footer branches.
- Manual call from MCP client shows the parse-error branch for an invalid pattern.

- [x] **Unit 6: Align `trace_symbol` docs with implementation**

**Goal:** Public docs point users at `get_symbol_context(sections=[...])`, not the removed `trace_symbol` top-level tool.

**Requirements:** R6

**Dependencies:** None

**Files:**
- Modify: `README.md` — replace/append `trace_symbol` references with the consolidated form.
- Modify: `CLAUDE.md` — same.
- Leave alone: `src/cli/init.rs:SYMFORGE_TOOL_NAMES` (deferred per runbook).
- Leave alone: `src/daemon.rs:1562-1580` (canonical alias).

**Approach:**
- Grep `README.md` and `CLAUDE.md` for `trace_symbol`. For each mention, either:
  - Replace with `get_symbol_context(..., sections=[...])` where the sentence describes the intended use.
  - Add a short "consolidated into `get_symbol_context`" note in the tool list, if the tool list enumerates removed/renamed tools.

**Patterns to follow:**
- `docs/runbooks/consolidate-mcp-tool.md` describes the canonical surface after consolidation.

**Test expectation:** none — documentation-only change.

**Verification:**
- `grep -R "trace_symbol" README.md CLAUDE.md` returns either zero hits or only the explicit "consolidated into" note.
- `cargo build` succeeds (no code changes).

## System-Wide Impact

- **Interaction graph:** Unit 3 reuses `find_parent_impl_type` / stale-reference filtering from the edit path; no new coupling introduced. Unit 4 changes the splice range inside `replace_symbol_body` but does not change its return contract.
- **Error propagation:** Unit 5 adds a new error-string shape for ast-grep parse failures. Callers currently treat any non-empty output as success text; adding an `Error:` prefix matches other error paths in the same function.
- **State lifecycle risks:** Unit 2 does not cache pre-delete state, so the watcher race remains real — we only describe it more honestly. Acceptable trade-off.
- **API surface parity:** `trace_symbol` remains aliased in `daemon.rs`, so backward-compat callers keep working. No MCP tool list change in this plan.
- **Integration coverage:** Units 3 and 4 need multi-file fixture tests (not just mocks) to prove the type-scoping and doc-preservation behaviors across real parsed files.
- **Unchanged invariants:** The MCP tool surface (tool names, input shapes, output envelopes) is untouched. The file-watcher behavior is unchanged.

## Risks & Dependencies

| Risk | Mitigation |
|---|---|
| Unit 4 changes a splice range — subtle off-by-one could corrupt files on certain edge cases (attributes, macros, inner docs) | Add fixture tests for attribute-prefixed symbols, inner-doc (`//!`) symbols, and language variants (Rust/TS/Python). Run `cargo test --all-targets -- --test-threads=1` before merge. |
| Unit 3's type-scoping may under-report callers when a file legitimately uses multiple types sharing a name (e.g., `Foo` from two modules) | Keep filter conservative: if the file references the parent type name at all, include its callers. Document the heuristic in the handler comment. |
| Unit 5's pattern pre-validation may double-parse and cost CPU on hot paths | ast-grep's parser is fast; structural search is already the slowest search mode. Net overhead is negligible. If benchmarked as regressing, cache the parsed pattern for the duration of the call. |
| Unit 6 text drift — future doc rewrites may reintroduce `trace_symbol` | None material. Runbook already warns; deferred removal in a follow-up PR will close the loop. |

## Documentation / Operational Notes

- `CHANGELOG.md` entry grouping: `### Fixed` (bugs 1–5), `### Changed` (doc cleanup for bug 6).
- No schema, config, or migration implications.
- Release notes should mention the `replace_symbol_body` behavior change (Unit 4) as a user-visible correction.

## Sources & References

- Review session output (inline this session, 2026-04-19).
- `src/sidecar/handlers.rs:1277-1391` — `repo_map_text`.
- `src/sidecar/handlers.rs:748-1004` — `handle_edit_impact`.
- `src/protocol/tools.rs:6419-6574` — `replace_symbol_body`.
- `src/protocol/tools.rs:3451-3500` — `search_text` structural branch.
- `src/protocol/edit.rs:407-450` — `extend_past_orphaned_docs`.
- `src/protocol/edit.rs:2274-2329` — `detect_stale_references`.
- `src/daemon.rs:1562-1580` — `trace_symbol` alias.
- `src/cli/init.rs:262-294` — `SYMFORGE_TOOL_NAMES`.
- `docs/runbooks/consolidate-mcp-tool.md` — tool-consolidation runbook.
