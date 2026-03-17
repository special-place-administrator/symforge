# Community Feedback Improvements — Actionable Plan

**Source:** 5 independent AI agent reviews across diverse codebases (Rust, Python, JS, C++, web projects)
**Date:** 2026-03-17

---

## Sprint 1: Quick Wins (Low Effort, High UX Impact)

### 1.1 — Better error message for `get_symbol` without path
**File:** `src/protocol/tools.rs` → `get_symbol()` handler
**Problem:** Calling `get_symbol(name="init")` without a `path` returns "File not found: ." — confusing.
**Fix:** Detect empty/missing path and return: "Path argument is required for single-symbol lookup. Use search_symbols(query=\"init\") to find candidates." If multiple matches exist, list them.
**Effort:** ~15 lines

### 1.2 — Ambiguity resolution shows signatures, not just line numbers
**File:** `src/protocol/format.rs` or wherever disambiguation messages are built
**Problem:** "Candidates: 255, 271" gives no context. Agent must do another lookup.
**Fix:** Include symbol kind + signature: `"Candidates: L255 fn handle(msg: Message), L271 async fn handle(event: Event)"`. The index already has this data — just format it.
**Effort:** ~25 lines

### 1.3 — `diff_symbols` clearer "no changes" output
**File:** `src/protocol/tools.rs` → `diff_symbols()` handler
**Problem:** Returns "No file changes found" — minimal and ambiguous.
**Fix:** Return: `"Compared {N} files between {base} and {target}. No symbol-level changes detected."` Include the ref names and file count.
**Effort:** ~10 lines

### 1.4 — `dry_run` on `insert_symbol` and `delete_symbol`
**Files:** `src/protocol/tools.rs` → `InsertSymbolInput`, `DeleteSymbolInput`; `src/protocol/edit.rs` → execution logic
**Problem:** Only `batch_edit` and `batch_rename` have `dry_run`. Users want preview safety on single operations too.
**Fix:** Add `dry_run: Option<bool>` field to both input structs. When true, resolve the symbol, format what would happen, but skip the write. Return preview like batch_edit does.
**Effort:** ~40 lines per tool (~80 total)

### 1.5 — `search_files` space-separated fuzzy matching
**File:** `src/live_index/search.rs` → file search logic
**Problem:** `search_files(query="sqlx store")` returns nothing. `search_files(query="sqlx_storage")` works.
**Fix:** Split query on whitespace, match each term independently against path segments, score by how many terms match. Similar to how `explore` handles multi-term queries.
**Effort:** ~30 lines

### 1.6 — Document `find_dependents` Mermaid/Graphviz output
**File:** Tool description in `src/protocol/tools.rs` → `find_dependents` `#[tool(description)]`
**Problem:** Users don't know `format: "mermaid"` and `format: "dot"` exist.
**Fix:** Add to description: `"Supports format='mermaid' or format='dot' for visual dependency graphs."`
**Effort:** ~1 line

---

## Sprint 2: Error Handling & Polish

### 2.1 — `batch_edit` infer kind for unique symbols
**File:** `src/protocol/edit.rs` → symbol resolution in `execute_batch_edit()`
**Problem:** `batch_edit` targeting `Layout` in a file fails without `kind: "fn"` even when `Layout` is unambiguous.
**Fix:** If symbol name resolves to exactly one match in the file, use it regardless of missing `kind`. If ambiguous, return current error with added "Did you mean?" suggestions showing kind + line for each candidate.
**Effort:** ~30 lines

### 2.2 — `batch_edit` "not found" shows context
**File:** `src/protocol/edit.rs` → `edit_within_symbol` error path
**Problem:** "Edit not found within symbol spawn" gives no clue what went wrong.
**Fix:** When `old_text` isn't found within the symbol body, show the first 100 chars of the actual symbol body in the error: `"old_text not found in fn spawn (body starts with: 'pub async fn spawn(config: Config) -> ...')"`. Helps agents self-correct.
**Effort:** ~20 lines

### 2.3 — `get_symbol` include `#[cfg(...)]` and other attributes
**File:** `src/parsing/languages/rust.rs` (or relevant parser) → symbol range calculation
**Problem:** `get_symbol` returns function body without preceding `#[cfg(...)]` attributes. Doc comments ARE included, but attributes are not. This causes `batch_edit` failures when `old_text` includes attributes.
**Fix:** Extend symbol range to include contiguous attribute lines above the definition, same way doc comments are captured. Ensure this is consistent across all languages that have decorators/attributes (Rust `#[...]`, Python `@decorator`, Java `@Annotation`).
**Effort:** ~40 lines per language (Rust first, others follow-up)
**Risk:** May change byte ranges for existing symbols — needs careful testing.

### 2.4 — Error message audit
**Files:** `src/protocol/tools.rs`, `src/protocol/edit.rs`
**Problem:** Multiple reports flagged vague errors. Systematic pass needed.
**Fix:** Grep for generic error messages (`"not found"`, `"invalid"`, `"failed"`) and add context: what was expected, what was provided, what the user should try instead.
**Effort:** ~2 hours, ~50 lines total across files

---

## Sprint 3: New Features (Medium Effort)

### 3.1 — `git_log` tool
**Files:** New handler in `src/protocol/tools.rs`, git logic in `src/git.rs`
**Problem:** Agents must shell out to `git log` and parse text output for commit history.
**Fix:** Add `git_log` tool that returns structured commit history. Parameters: `path` (file), `name` (symbol — map to file via index), `limit`, `since`. Return: list of `{hash, author, date, message, files_changed}`. Leverage existing git integration.
**Effort:** ~120 lines
**Schema:**
```rust
struct GitLogInput {
    path: Option<String>,       // file path
    name: Option<String>,       // symbol name (resolved to file)
    limit: Option<u32>,         // default 10
    since: Option<String>,      // git date format
}
```

### 3.2 — `diff_symbols` with `uncommitted=true`
**File:** `src/protocol/tools.rs` → `DiffSymbolsInput`, `diff_symbols()` handler
**Problem:** `diff_symbols` only compares git refs. Can't see symbol-level diffs for uncommitted working tree changes.
**Fix:** Add `uncommitted: Option<bool>` parameter. When true, diff the current index state against the last committed version. The index already has current symbols; need to parse the committed version from git for comparison.
**Effort:** ~80 lines
**Depends on:** Ability to parse a file at a specific git ref (may need `git show HEAD:path`)

### 3.3 — `batch_replace_text` tool
**File:** New handler in `src/protocol/tools.rs`, execution in `src/protocol/edit.rs`
**Problem:** No way to do repo-wide plain text find-and-replace with safety. `batch_rename` is symbol-aware only.
**Fix:** Add `batch_replace_text` tool with `old_text`, `new_text`, `glob` (file filter), `dry_run`. Scans all indexed files for literal matches, replaces, writes atomically.
**Effort:** ~100 lines
**Schema:**
```rust
struct BatchReplaceTextInput {
    old_text: String,
    new_text: String,
    glob: Option<String>,       // e.g. "*.rs" or "src/**/*.ts"
    dry_run: Option<bool>,      // default false
    case_sensitive: Option<bool>, // default true
}
```

### 3.4 — `verbosity` applied to bundle mode dependencies
**File:** `src/protocol/format.rs` → bundle rendering logic
**Problem:** `get_symbol_context(bundle=true)` expands all type dependencies with full definitions. Large traits consume excessive tokens even when `verbosity='signature'` is set.
**Fix:** Apply the `verbosity` parameter to dependency definitions in bundle mode, not just the main symbol. When `verbosity='signature'`, dependencies show only name + params + return type.
**Effort:** ~40 lines

### 3.5 — `explore` grouping for large results
**File:** `src/protocol/tools.rs` → `explore()` handler, `src/protocol/format.rs` → explore formatting
**Problem:** Large explore results are a flat list — hard to scan.
**Fix:** Add `group_by: Option<String>` parameter. Values: `"file"` (default, current behavior), `"language"`, `"module"` (group by top-level directory). Format output with headers.
**Effort:** ~50 lines

---

## Sprint 4: Advanced Features (High Effort)

### 4.1 — `move_symbol` tool
**Files:** New handler in `src/protocol/tools.rs`, execution in `src/protocol/edit.rs`
**Problem:** Refactoring often requires moving a function/struct between files. Currently requires manual delete + insert + import fixup.
**Fix:** `move_symbol(from_path, name, to_path, position)` — deletes symbol from source, inserts at target, updates all imports/references across the project.
**Effort:** ~300 lines
**Risk:** High complexity — needs to handle: re-exports, qualified paths, wildcard imports, circular references. Recommend shipping with `dry_run` only initially, then adding write support after validation.
**Schema:**
```rust
struct MoveSymbolInput {
    from_path: String,
    name: String,
    to_path: String,
    position: Option<String>,    // "before"/"after" a target symbol
    target_symbol: Option<String>, // where to insert in destination
    dry_run: Option<bool>,       // default true for safety
}
```

### 4.2 — HTML cross-reference support
**Files:** `src/parsing/languages/html.rs` → extractor, `src/parsing/xref.rs` → reference building
**Problem:** `find_dependents` for JS/CSS files doesn't find HTML files that reference them via `<script src>` or `<link href>`.
**Fix:** Extract `src` and `href` attributes from HTML as cross-references to the target files. Also extract `id` and `class` attributes as symbols for CSS → HTML reference tracking.
**Effort:** ~150 lines
**Risk:** HTML attributes are strings, not code — false positive potential is higher. May need a confidence threshold.

---

## Priority Matrix

| Priority | Items | Rationale |
|----------|-------|-----------|
| **P0 — Do first** | 1.1, 1.2, 1.3, 1.6 | Trivial effort, immediate UX improvement |
| **P1 — This sprint** | 1.4, 1.5, 2.1, 2.2, 2.4 | Error handling + safety polish |
| **P2 — Next sprint** | 3.1, 3.2, 3.3, 3.4, 3.5 | New capabilities that fill real gaps |
| **P3 — Backlog** | 2.3, 4.1, 4.2 | High effort or high risk, needs design |

---

## Orchestrator Prompt Template

To have Kilo implement a sprint, use this prompt pattern:

```
## Task: Implement [Sprint N] from community feedback

Read `plans/community-feedback-improvements.md` — section "Sprint N". Implement all items in that sprint.

### Rules
1. You and ALL subagents MUST use Tokenizor MCP tools for codebase navigation and editing.
2. Each item has exact files, symbols, and effort estimates — follow them.
3. Add tests for every change. Follow existing test patterns in the affected files.
4. Run `cargo check` and `cargo test --lib` after each item.
5. Do NOT commit — report results and let the human review.

### Execution
Items within a sprint are independent — parallelize where files don't overlap.
```

---

## Sprint 0: Index Freshness Guarantee (P0 — Critical Infrastructure)

Discovered during the SymForge rename: Tokenizor's file watcher missed rapid bulk writes (30+ files via PowerShell), causing `search_text` to return stale results. The index silently served outdated data. **An index that lies is worse than no index at all.**

### 0.1 — Mtime guard on read (the primary fix)
**Files:** `src/live_index/store.rs` → `read()` or query entry points, `src/live_index/search.rs`
**Problem:** Queries trust the index blindly. If a file changed on disk after indexing, stale results are returned silently.
**Fix:** Before returning indexed data for a file, `stat()` the file and compare `mtime_secs` against the stored value. If different, re-index that file on demand before responding. This is how IntelliJ, VS Code, and most LSP servers guarantee freshness.
**Cost:** One syscall per file per query. Negligible for typical searches (50 files). For `get_repo_map` (all files), consider sampling or skipping the guard.
**Effort:** ~60 lines
**Priority:** P0 — this is a correctness guarantee, not a feature

### 0.2 — Watcher overflow detection and full rescan
**Files:** `src/watcher/mod.rs`
**Problem:** OS file watchers have finite event buffers. When they overflow (rapid bulk writes, large git operations), events are silently dropped. The `notify` crate emits a special overflow/rescan event.
**Fix:** Detect `notify::Event::Rescan` or overflow events. When detected, trigger a full mtime reconciliation sweep across all indexed files. Log `tracing::warn!("Watcher buffer overflow detected — running full reconciliation")`.
**Effort:** ~30 lines

### 0.3 — Periodic reconciliation sweep (belt-and-suspenders)
**Files:** `src/watcher/mod.rs` or new background task in `src/main.rs`
**Problem:** Even with mtime guards and overflow detection, edge cases exist (network drives, sleep/wake, external tools). A periodic sweep catches everything.
**Fix:** Background task every 30 seconds walks all indexed files, compares mtime on disk vs index. Re-indexes any that drifted. Configurable via `SYMFORGE_RECONCILE_INTERVAL` (default 30s, 0 to disable).
**Effort:** ~80 lines

### 0.4 — Health warning for suspected stale index
**Files:** `src/protocol/tools.rs` → `health()` handler
**Problem:** If the watcher detected an overflow or reconciliation found drifted files, the health output should surface this.
**Fix:** Add a `stale_warnings: Vec<String>` field to health output. Populate with messages like "Watcher overflow at {time} — {N} files reconciled" or "Reconciliation found {N} stale files at {time}".
**Effort:** ~25 lines

### Implementation order
0.1 (mtime guard) alone fixes the root cause. Ship that first. 0.2-0.4 are defense-in-depth.

**Total effort:** ~195 lines
