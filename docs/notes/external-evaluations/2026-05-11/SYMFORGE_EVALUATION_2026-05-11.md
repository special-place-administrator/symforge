# SymForge Mid-Development Evaluation Report
**Date:** 2026-05-11  
**Evaluator:** Kimi Code CLI (interactive agent)  
**Method:** Systematic stress-test of the full MCP tool surface against two repositories:
1. `symforge` itself (575 files, 15,338 symbols)
2. `Agent_Army_Professionals` (1,135 files, 33,050 symbols)

**Environment:** Windows, two separate daemon processes (one per agent).

---

## Executive Summary

SymForge's **read path is production-quality when the index is stable**, but this build carries **a catastrophic index-durability bug on Windows** that silently destroys indexes after `index_folder`, **a refactoring tool deadlock**, **a cosmetic labeling bug**, and **parser gaps for modern Rust syntax**. The most severe issue (P0) is not search accuracy — it is **index integrity**.

**Critical new finding:** Even with **separate daemon processes**, the index collapses from 1,135 files to **4 files** within ~2 minutes. The files still exist on disk (confirmed by `git status` and a direct `std::fs::read` test of 119,752 files with **0 failures**).

---

## P0 — Catastrophic: Index Self-Destruction After `index_folder` on Windows 🔴

### Symptom
After calling `index_folder`, the index **monotonically collapses**:

| Time | Files | Symbols | Reconcile Repairs |
|------|-------|---------|-------------------|
| After `index_folder` | 1,135 | 33,050 | 0 |
| ~30s later | 901 | 27,638 | 1,182 |
| ~60s later | 818 | 25,227 | 1,182 |
| ~2min later | **4** | **247** | 6,034 |

All subdirectory files vanish; only root-level files (`Cargo.toml`, `README.md`, `scripts/coordination.js`) survive. Searches return "File not found" or empty results.

**Git status confirms all files still exist on disk.** A direct Rust test that mimics `reconcile_stale_files` path construction succeeded for **all 119,752 files** with **0 `NotFound` errors**.

### Root-Cause Analysis

The watcher reconciliation loop (`reconcile_stale_files` → `freshen_file_if_stale` → `maybe_reindex`) removes files when `std::fs::read` returns `ErrorKind::NotFound`:

```rust
Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
    shared.remove_file(relative_path);
    warn!("watcher: file not found, removed from index: {relative_path}");
    return ReindexResult::Removed;
}
```

Since the files exist on disk and paths are correct, `std::fs::read` must be returning `NotFound` due to one of these mechanisms:

#### Mechanism A — Stale `spawn_blocking` watcher task after `abort()`
`ProjectInstance::reload()` calls:
```rust
abort_watcher_task(&mut self.watcher_task);
self.watcher_task = start_project_watcher(...);
```

`abort_watcher_task` calls `task.abort()`, which **does NOT cancel an in-flight `tokio::task::spawn_blocking` reconciliation task**. The old watcher's reconciliation loop can continue executing.

Crucially, **both the old and new watchers hold `Arc::clone(&self.index)`** pointing to the **same `SharedIndexHandle`**. When `reload()` swaps new index data via `self.index.reload()`, both watchers see the updated data. If the old watcher is still running reconciliation with a stale `repo_root` (or even the correct one), it may remove files based on stale or incorrect state.

#### Mechanism B — `process_events` on a dropped debouncer with buffered events
When `abort_watcher_task` drops the `WatcherHandle`, the debouncer is dropped and the event channel is closed. But if the watcher task is blocked in `spawn_blocking`, it doesn't drain the channel immediately. After `spawn_blocking` returns, `try_recv` may return buffered events (including `Remove` events from prior activity). These events are processed against the **same `SharedIndex`**.

#### Mechanism C — Windows-specific race in file existence
Even though a direct test reads all files successfully, the watcher may call `std::fs::read` at moments when another process (antivirus, IDE, build tool) holds an exclusive lock or is mid-write. On Windows, this can return `NotFound` in specific timing windows rather than `PermissionDenied`.

### Most Likely Culprit
**Mechanism A (stale `spawn_blocking` + shared `SharedIndexHandle`)** is the strongest architectural bug. The fact that my test read all files successfully rules out pure path-construction failure. The fact that only root files survive suggests a systematic removal pattern consistent with a reconciliation sweep removing files it cannot verify.

### Fix Directions
1. **Immediate:** Add a `project_id` / `repo_root` generation check inside `reconcile_stale_files` and `process_events`. If the project's `canonical_root` or `project_id` has changed since the watcher started, exit early.
2. **Robust:** Replace `task.abort()` with a cancellation token that the watcher loop polls inside `spawn_blocking`.
3. **Defense in depth:** In `maybe_reindex`, before calling `shared.remove_file`, verify that the absolute path is within the current project's root and that the file genuinely does not exist (retry once with a short delay).
4. **Windows-specific:** Ensure `normalize_event_path` and `reconcile_stale_files` use identical path normalization logic.

---

## P1 — Critical: `batch_rename` Hard Timeout 🔴

### Symptom
`batch_rename` (even `dry_run=true`) against modest symbols consistently **times out**.

### Impact
The primary refactoring tool is **unusable**.

### Hypothesis
Either unbounded reference traversal or a **lock-order inversion** between the index read-lock and the git-temporal lock during rename impact analysis.

### Fix
Profile `batch_rename` with `RUST_LOG=debug`. Common causes:
- Recursive reference traversal without cycle detection
- Holding a lock while waiting for a blocking git-temporal query

---

## P1 — Primary Defect: `search_text` Structural Search Mislabeled

### Symptom
When `structural=true`, envelope says:
```
Match type: constrained (literal)
```

### Root Cause
`search_text_match_type_label()` in `src/protocol/tools.rs:1836` has no branch for structural search.

### Fix
Add `structural: bool` parameter and return `"structural (ast-grep)"`.

---

## P2 — Notable: Partial Parse Failures on Modern Rust Syntax

### Symptom
Tree-sitter errors on `&raw` (Rust 1.82+):
- `src/live_index/persist.rs` — "syntax error near `&raw`" (line 1199)
- `src/worktree.rs` — "syntax error near `&raw`" (line 296)

### Fix
Update vendored tree-sitter Rust grammar for raw references.

---

## P2 — Peripheral: Obsidian MCP Path Resolution

- `vault list` fails: `Directory not found: docs`
- `graph statistics` reports 0 links for files with wikilinks

---

## ✅ What Works Well (When the Index Is Stable)

| Category | Tools | Verdict |
|----------|-------|---------|
| **Discovery** | `search_symbols`, `search_text`, `search_files`, `explore` | ✅ Accurate, fast |
| **Retrieval** | `get_symbol`, `get_symbol_context`, `get_file_context`, `get_file_content` | ✅ Excellent token savings |
| **Navigation** | `find_references`, `find_dependents`, `inspect_match`, `get_repo_map` | ✅ Correct cross-references |
| **Git Intelligence** | `what_changed`, `diff_symbols`, `analyze_file_impact` | ✅ Temporal coupling works |
| **Diagnostics** | `health`, `conventions`, `context_inventory` | ✅ Detailed |
| **Dry-Run Edits** | All edit tools with `dry_run=true` | ✅ Safe previews |
| **Structural Search** | `search_text` with `structural=true` | ✅ **Functionally correct** |

---

## Reproduction Scripts

### P0 — Index Destruction (Windows)
```bash
# 1. Start daemon
# 2. Index project with 1,000+ files
# 3. Wait 30–120 seconds without indexing anything else
# 4. Call health — file count declines monotonically
# 5. Eventually only root files remain
```

### P1 — batch_rename Timeout
```json
{
  "path": "src/daemon.rs",
  "name": "health",
  "new_name": "get_health",
  "dry_run": true
}
```

### P1 — Structural Label Bug
```json
{
  "query": "fn $NAME($$$) { $$$ }",
  "structural": true,
  "language": "Rust",
  "limit": 5
}
```

---

## Priority Ranking

| Priority | Item | Location | Effort |
|----------|------|----------|--------|
| 🔴 **P0** | Fix watcher race / index destruction | `src/daemon.rs:reload`, `src/watcher/mod.rs:run_watcher` | Medium |
| 🔴 **P1** | Fix `batch_rename` timeout/deadlock | `src/protocol/edit.rs:execute_batch_rename` | Unknown — needs profiling |
| 🟡 **P1** | Fix structural search envelope label | `src/protocol/tools.rs:search_text_match_type_label` | Trivial |
| 🟢 **P2** | Update Rust grammar for `&raw` syntax | `vendor/tree-sitter-rust` | Medium |
| 🟢 **P2** | Investigate Obsidian vault path resolution | Obsidian MCP server config | Medium |

---

## Architecture Note

The user explicitly stated:
> *"index should be per agent or in memory and idempotent regardless of who triggers it and how many times"*

The current daemon design **does not meet this expectation**:
- `reload()` mutates the same `SharedIndexHandle` in place
- `abort_watcher_task()` leaves stale `spawn_blocking` tasks running
- Stale tasks share the same `Arc<SharedIndexHandle>` as the new watcher
- No cancellation token or project-generation guard prevents stale tasks from mutating the index

**Recommendation:** Consider either:
1. **Per-session index isolation** (each session gets its own `SharedIndex` clone), or
2. **A robust cancellation protocol** inside `run_watcher` that detects project switches and exits `spawn_blocking` early.

---

*Report generated by live systematic invocation against a running SymForge daemon. The index was destroyed three times during this session. A direct Rust file-read test of 119,752 paths returned 0 failures, ruling out simple path-construction bugs.*
