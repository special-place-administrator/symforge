# MCP Output Quality Improvements — Design Spec

**Date:** 2026-03-20
**Scope:** 6 improvements split across 2 PRs
**Deferred:** Item 7 (find_references implementations depth) — post-canary sprint

---

## PR 1: Output Polish (Items 1-4)

Pure formatting changes in the output layer. No input struct changes, no query logic changes, no breaking changes.

### Item 1 — `diff_symbols` compact: explain filtered files

**Problem:** Compact mode silently drops files with no symbol-level changes, causing a file count mismatch vs non-compact output.

**File:** `src/protocol/format.rs` — `diff_symbols_result_view` (L6267)

**Change:** After the summary line, when `compact == true` and some files were filtered (i.e., `changed_files.len() > files_with_symbol_changes` where `files_with_symbol_changes > 0`), append:
```
(N file(s) with only non-symbol changes omitted)
```

**Test:** Add a test in `src/protocol/tools.rs` that verifies compact mode output includes the omission note when files have non-symbol changes.

### Item 2 — `search_text` ranked: show hidden count

**Problem:** `ranked=true` returns fewer results with no indication that ranking filtered anything.

**Files:**
- `src/live_index/search.rs` — `TextSearchResult` struct: add `suppressed_by_ranking: usize` field
- `src/protocol/format.rs` — `search_text_result_view`: append hint when `suppressed_by_ranking > 0`

**Change:** In the ranking codepath of `search_text_with_options`, count matches that were present before ranking but absent after. Store in `suppressed_by_ranking`. The formatter appends:
```
(N lower-ranked matches hidden — set ranked=false to see all)
```

**Test:** Add a test that searches with `ranked=true` on a dataset with both high and low-importance symbols and verifies the suppression count appears.

### Item 3 — `search_text` OR terms: annotate which term matched

**Problem:** `terms=["A", "B", "C"]` blends all matches — no way to tell which term produced which hit.

**Files:**
- `src/live_index/search.rs` — `TextLineMatch` struct: add `matched_term: Option<String>` field
- `src/live_index/search.rs` — term iteration loop: populate `matched_term` with the current term
- `src/protocol/format.rs` — `search_text_result_view`: render `[term: X]` suffix when multiple terms are active

**Change:** The search loop in `search_text_with_options` that iterates over `terms` already knows which term is being matched. Thread the term string into each `TextLineMatch`. The formatter only renders the annotation when `terms.len() > 1` to avoid noise on single-term searches.

**Output example:**
```
src/live_index/query.rs
  in fn resolve_module_path (lines 34-132):
    > 34: fn resolve_module_path(file_path: &str, language: &LanguageId) -> Option<String>  [term: LanguageId]
```

**Test:** Add a test with `terms=["A", "B"]` and verify each match line carries the correct `matched_term`.

### Item 4 — `find_dependents` mermaid: symbol-level edge labels

**Problem:** Mermaid output shows only `file -->|N refs| target` — just counts, no signal about what's referenced.

**File:** `src/protocol/format.rs` — `find_dependents_mermaid` (L2105)

**Change:** For each file in `view.files`, extract up to 3 distinct reference names from `file.lines`. Render the edge label as:
```
dep["src/daemon.rs"] -->|"SearchTextInput, GetSymbolInput +99"| target
```
Where `+99` indicates remaining references beyond the 3 named ones. If all references fit in 3 names, no `+N` suffix.

**Test:** Add a test verifying mermaid output includes symbol names in edge labels.

---

## PR 2: Edit dry_run + Used by Accuracy (Items 5-6)

Structural changes: new input fields and query filtering logic.

### Item 5 — `dry_run` on single edit tools

**Problem:** `batch_edit` and `batch_rename` have `dry_run`, but `replace_symbol_body`, `insert_symbol`, `delete_symbol`, and `edit_within_symbol` don't. Inconsistent.

**Files:**
- `src/protocol/edit.rs` — 4 input structs: `ReplaceSymbolBodyInput`, `InsertSymbolInput`, `DeleteSymbolInput`, `EditWithinSymbolInput` — add `#[serde(default, deserialize_with = "super::tools::lenient_bool")] pub dry_run: Option<bool>` field
- `src/protocol/tools.rs` — 4 handler methods: `replace_symbol_body` (L3258), `insert_symbol` (L3362), `delete_symbol` (L3428), `edit_within_symbol` (L3496) — add early-return branch after symbol resolution when `dry_run == Some(true)`

**Dry-run output format** (consistent across all 4):
```
[DRY RUN] Would <verb> `<symbol_name>` in <path> (<detail>)
```
Where `<verb>` is "replace", "insert before/after", "delete", or "edit within", and `<detail>` is size info (old → new bytes for replace, content size for insert, byte count for delete, replacement count for edit_within).

**Test:** One test per tool verifying dry_run returns preview without modifying the file. Follow the existing `test_batch_edit_applies_across_files` pattern.

### Item 6 — `get_file_context` "Used by" false positives

**Problem:** `main.rs` shows 48 files in "Used by" — impossible for a binary entry point. The `find_dependents_for_file` query matches by symbol name without verifying the reference actually resolves to the target file.

**File:** `src/sidecar/handlers.rs` — `outline_text` (L192), specifically the "Used by" section around L290.

**Root cause:** `attributed_dependents` from `find_dependents_for_file` returns `(file_path, ReferenceRecord)` pairs where the reference shares a *name* with a symbol in the target file, but doesn't verify the reference is *to* that file. Generic names like `main`, `new`, `default`, `run` cause false positives.

**Fix — two-layer filter:**

1. **Qualified-name filter (primary):** If the reference has a `qualified_name`, check that it contains the target file's module path segment. If it doesn't match, skip.

2. **Visibility heuristic (fallback):** When `qualified_name` is absent, check whether the target file actually exports a `pub` symbol with the matching name. If the target file has no `pub` symbol by that name, the reference cannot be to this file — skip it.

This approach is conservative: it may still allow some false positives when multiple files export the same `pub` symbol name, but it eliminates the `main.rs` class of false positives entirely and dramatically reduces noise for files with common internal helper names.

**Test:** Add a test with a synthetic index where file A has a non-pub symbol `foo` and file B references `foo` from a different module — verify file B does not appear in file A's "Used by" section.

---

## Out of Scope

- **Item 7 (find_references implementations depth):** Deferred to post-canary sprint. Requires index data model changes.
- **Tool description length optimization:** MCP protocol doesn't support short/long description split. Revisit if protocol evolves.
- **New tools or tool consolidation:** Not part of this work.

## Risk Assessment

- **PR 1:** Zero risk. Pure output formatting. All changes are additive — existing output gets richer, nothing is removed.
- **PR 2 Item 5:** Low risk. New `#[serde(default)]` field with `Option<bool>` — absent = false = existing behavior. Follows established `batch_edit` pattern.
- **PR 2 Item 6:** Medium risk. Filtering logic could be too aggressive (hiding real dependents) or too permissive (still showing false positives). Mitigated by the two-layer approach and conservative fallback. If investigation reveals the issue is in `find_dependents_for_file` itself rather than the rendering, we scope down to heuristic filtering only.
