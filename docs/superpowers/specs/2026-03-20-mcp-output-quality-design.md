# MCP Output Quality Improvements — Design Spec

**Date:** 2026-03-20
**Scope:** 5 improvements split across 2 PRs (1 item dropped, 1 moved between PRs)
**Deferred:** Item 7 (find_references implementations depth) — post-canary sprint

---

## Revision History

- **v1:** 6 items across 2 PRs.
- **v2:** Spec review findings incorporated:
  - **Item 2 dropped:** `ranked` only re-sorts, never filters — `suppressed_by_ranking` would always be 0.
  - **Item 3 revised:** Post-hoc term matching in formatter, not search pipeline change. The `is_match` closure is `FnMut(&str) -> bool` — per-term identity is lost at the closure boundary.
  - **Item 4 moved to PR 2:** `DependentLineView` lacks a `name` field — requires struct change in `query.rs`.
  - **Item 5 clarified:** Follows `BatchRenameInput` pattern (`Option<bool>` + `lenient_bool`). Dry-run byte sizes report raw input size, not projected indented size.
  - **Item 6 retargeted:** Fix in `query.rs` (`find_dependents_for_file`) to benefit all consumers, not just `get_file_context`. Existing test `test_get_file_context_ignores_generic_name_noise_without_real_dependency` referenced.
- **v3 (current):** Second spec review findings:
  - **Item 1 fixed:** `files_with_symbol_changes` is a symbol count, not file count. Introduced `files_with_changes` counter to track files that passed the symbol-change filter.
  - **Item 6 clarified:** `SymbolRecord` has no `visibility` field. Visibility heuristic uses text scan of `IndexedFile.content` for `pub`/`export` keywords. Language-specific behavior documented.

---

## PR 1: Output Polish (Items 1, 3)

Pure formatting changes in the output layer. No input struct changes, no query logic changes, no breaking changes.

### Item 1 — `diff_symbols` compact: explain filtered files

**Problem:** Compact mode silently drops files with no symbol-level changes, causing a file count mismatch vs non-compact output.

**File:** `src/protocol/format.rs` — `diff_symbols_result_view` (L6266)

**Change:** Introduce a new counter `files_with_changes: usize` that increments each time the loop does NOT `continue` at the "no symbol-level changes" branch. After the summary line, when `compact == true` and `changed_files.len() > files_with_changes` (i.e., some files were omitted), append:
```
(N file(s) with only non-symbol changes omitted)
```
where `N = changed_files.len() - files_with_changes`.

**Important:** Do NOT reuse the existing `files_with_symbol_changes` variable — that holds the total *symbol* count (`total_added + total_removed + total_modified`), not the number of *files* with symbol changes.

**Test:** Add a test in `src/protocol/tools.rs` that verifies compact mode output includes the omission note when files have non-symbol changes.

### Item 3 — `search_text` OR terms: annotate which term matched

**Problem:** `terms=["A", "B", "C"]` blends all matches — no way to tell which term produced which hit.

**Files:**
- `src/protocol/format.rs` — `search_text_result_view` (L302): add post-hoc term annotation

**Implementation approach — post-hoc matching in the formatter:**

The search pipeline's `is_match` closure (`FnMut(&str) -> bool`) combines all terms into a single boolean predicate, losing per-term identity. Rather than restructuring the search pipeline, the formatter performs a cheap post-hoc check: for each `TextLineMatch`, test which original term(s) appear case-insensitively in the `line` field. This works because:
- Terms are literal substrings (not regex)
- The line already matched at least one term
- False attribution is harmless (a line matching multiple terms gets annotated with the first match)

The annotation is only rendered when `terms.len() > 1`. No changes to `TextLineMatch` struct or `search.rs`.

**Formatter change:** `search_text_result_view` receives the original `terms` list (passed through from the handler). When rendering a match line and `terms.len() > 1`, append `[term: X]`:
```
src/live_index/query.rs
  in fn resolve_module_path (lines 34-132):
    > 34: fn resolve_module_path(file_path: &str, language: &LanguageId) -> Option<String>  [term: LanguageId]
```

For regex mode: `matched_term` annotation is skipped (terms are not used in regex mode). This is stated explicitly.

**Test:** Add a test with `terms=["alpha", "beta"]` on synthetic data and verify the formatter output includes `[term: alpha]` and `[term: beta]` on the correct lines.

---

## ~~Item 2 — DROPPED~~

**Reason:** `ranked=true` only re-sorts results by importance score (`compute_importance_score`). It does not filter or suppress any results. The `total_limit` cap applies identically regardless of `ranked`. A `suppressed_by_ranking` counter would always be 0.

If ranking-based truncation is desired in the future, it should be designed as a separate feature with a defined score threshold.

---

## PR 2: Structural Improvements (Items 4, 5, 6)

New fields, struct changes, and query filtering logic.

### Item 4 — `find_dependents` mermaid: symbol-level edge labels

**Problem:** Mermaid output shows only `file -->|N refs| target` — just counts, no signal about what's referenced.

**Files:**
- `src/live_index/query.rs` — `DependentLineView` (L770): add `pub name: String` field
- `src/live_index/query.rs` — the builder that constructs `DependentLineView` instances: populate `name` from the reference record
- `src/protocol/format.rs` — `find_dependents_mermaid` (L2105): use `name` field for edge labels

**Change:** `DependentLineView` currently has `line_number`, `line_content`, `kind` but no symbol name. Add `pub name: String`. The query code that builds these views already has access to the `ReferenceRecord` which contains the `name` field — thread it through.

In the mermaid formatter, for each file, collect up to 3 distinct `name` values from `file.lines`. Render:
```
dep["src/daemon.rs"] -->|"SearchTextInput, GetSymbolInput +99"| target
```
Where `+99` is the count of remaining references beyond the 3 named. Also update `find_dependents_dot` for consistency.

**Why moved from PR 1:** Requires a struct change in `query.rs`, which is not a pure formatting change.

**Test:** Add a test verifying mermaid output includes symbol names in edge labels. Extend the existing `test_find_dependents_dot_shows_true_ref_count_not_capped` pattern.

### Item 5 — `dry_run` on single edit tools

**Problem:** `batch_edit` and `batch_rename` have `dry_run`, but `replace_symbol_body`, `insert_symbol`, `delete_symbol`, and `edit_within_symbol` don't.

**Files:**
- `src/protocol/edit.rs` — 4 input structs: `ReplaceSymbolBodyInput` (L560), `InsertSymbolInput` (L575), `DeleteSymbolInput` (L593), `EditWithinSymbolInput` (L606) — add `#[serde(default, deserialize_with = "super::tools::lenient_bool")] pub dry_run: Option<bool>` field

  Pattern follows `BatchRenameInput` (not `BatchEditInput` which uses plain `bool`).

- `src/protocol/tools.rs` — 4 handler methods: `replace_symbol_body` (L3258), `insert_symbol` (L3362), `delete_symbol` (L3428), `edit_within_symbol` (L3496) — add early-return branch after symbol resolution when `dry_run == Some(true)`

**Dry-run output format** (consistent across all 4):
```
[DRY RUN] Would <verb> `<symbol_name>` in <path> (<detail>)
```
Where `<verb>` is "replace", "insert before/after", "delete", or "edit within".

`<detail>` is:
- **replace:** `old: N bytes → new: M bytes` — raw input `new_body.len()`, not projected indented size (the indented size depends on the target context and computing it without writing would add complexity for marginal value)
- **insert:** `N bytes of content`
- **delete:** `N bytes`
- **edit_within:** `N replacement(s)` — count of matches found within the symbol

**Test:** One test per tool verifying dry_run returns preview without modifying the file. Follow the existing `test_batch_edit_applies_across_files` pattern (write a file, call with dry_run, verify file unchanged, verify output contains `[DRY RUN]`).

### Item 6 — `get_file_context` "Used by" false positives

**Problem:** `main.rs` shows 48 files in "Used by" — impossible for a binary entry point.

**Root cause:** `find_dependents_for_file` in `query.rs` returns `(file_path, ReferenceRecord)` pairs where the reference shares a *name* with a symbol in the target file, but doesn't verify the reference is *to* that file. Generic names like `main`, `new`, `default`, `run` cause false positives.

**Existing mitigation:** `test_get_file_context_ignores_generic_name_noise_without_real_dependency` (tools.rs L4337) shows some noise filtering already exists. This item addresses the cases that mitigation doesn't catch.

**File:** `src/live_index/query.rs` — `find_dependents_for_file` and its internal matching logic

**Fix — two-layer filter applied in `find_dependents_for_file`:**

1. **Qualified-name filter (primary):** If the reference has a `qualified_name`, check that it contains the target file's module path segment (derived from the file path). If it doesn't match, skip. Note: for re-exported symbols, the qualified name references the re-exporting module — this is correct behavior since the dependency is on the re-exporter.

2. **Visibility heuristic (fallback):** When `qualified_name` is absent, check whether the target file exports a publicly visible symbol with the matching name. `SymbolRecord` has no `visibility` field, so this is done via a text scan of `IndexedFile.content` (which is available in `find_dependents_for_file`):
   - **Rust:** scan for `pub fn <name>`, `pub struct <name>`, `pub enum <name>`, `pub trait <name>`, `pub type <name>`, `pub const <name>`, `pub static <name>`, `pub mod <name>`
   - **JavaScript/TypeScript:** scan for `export` preceding the symbol declaration
   - **Python:** skip this layer (Python has no export keyword; all module-level symbols are importable)
   - **Other languages:** skip this layer (no false-negative risk — the qualified-name filter handles most cases)

   If the target file has no publicly visible symbol by that name (for languages where visibility applies), the reference cannot be to this file — skip it.

**Why fix in query.rs instead of handlers.rs:** Fixing at the source benefits all consumers: `get_file_context` "Used by", `find_dependents` tool, mermaid/dot output, and the sidecar outline endpoint.

**Test:** Add a test with a synthetic index where file A has a non-pub symbol `foo` (Rust file with `fn foo()` not `pub fn foo()`) and file B references `foo` from a different module — verify file B does not appear in file A's dependents. Also test that a `pub` symbol with a qualified import path that matches the target file IS included.

---

## Out of Scope

- **Item 2 (search_text ranked hint):** Dropped — ranking doesn't filter results, only re-sorts.
- **Item 7 (find_references implementations depth):** Deferred to post-canary sprint. Requires index data model changes.
- **Tool description length optimization:** MCP protocol doesn't support short/long description split. Revisit if protocol evolves.
- **New tools or tool consolidation:** Not part of this work.

## Risk Assessment

- **PR 1:** Zero risk. Pure output formatting. All changes are additive — existing output gets richer, nothing is removed.
- **PR 2 Item 4:** Low risk. Additive `name` field on `DependentLineView`. Existing tests won't break (they don't assert on the absence of this field).
- **PR 2 Item 5:** Low risk. New `#[serde(default)]` field with `Option<bool>` — absent = false = existing behavior. Follows established `BatchRenameInput` pattern.
- **PR 2 Item 6:** Medium risk. Filtering logic could be too aggressive (hiding real dependents) or too permissive. Mitigated by conservative two-layer approach and fixing at the query level. Existing noise-filtering test validates the baseline.
