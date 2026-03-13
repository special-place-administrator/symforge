# Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all actionable issues from the external Tokenizor MCP review — bugs, filtering gaps, explore quality, and daemon completeness — with zero functionality regression.

**Architecture:** Surgical edits across 4 files (`tools.rs`, `format.rs`, `daemon.rs`, `query.rs`). Each task is independent and produces a compilable, testable state. Agents 1 and 2 already applied partial fixes (git temporal cascade, inspect_match off-by-one); this plan completes the remaining work and extends what they started.

**Tech Stack:** Rust, tree-sitter (indirectly via line ranges), git CLI subprocess calls.

---

## Pre-existing changes (already applied by investigation agents)

These are in the working tree and should NOT be re-applied:

- `src/daemon.rs`: Added `trace_symbol` and `inspect_match` to `execute_tool_call` + imports
- `src/live_index/query.rs:1724,1736`: Off-by-one fix for `inspect_match` (EnclosingSymbolView, SiblingSymbolView)
- `src/protocol/mod.rs:219-224`: `ensure_local_index` now spawns git temporal computation
- `src/protocol/tools.rs:4871,4885`: Test updated for 0-based input

---

## Task 1: Fix remaining 0-based line range display bugs in format.rs

**Files:**
- Modify: `src/protocol/format.rs:65` (render_file_outline)
- Modify: `src/protocol/format.rs:153` (render_symbol_detail)
- Modify: `src/protocol/format.rs:1845` (render_context_bundle_found)
- Modify: `src/protocol/format.rs:2235` (dependency rendering in context bundle)

**Context:** Tree-sitter stores line ranges as 0-based. Several formatters display these raw values instead of converting to 1-based. The `inspect_match` formatter was fixed by Agent 2 in `query.rs`; these are the same class of bug in `format.rs`. Note: some places in format.rs already correctly add `+1` (e.g., lines 395-396 in usage grouping), confirming the inconsistency.

- [ ] **Step 1: Fix `render_file_outline` (line 65)**

Change:
```rust
// Line 65: sym.line_range.0, sym.line_range.1
// becomes:
sym.line_range.0 + 1, sym.line_range.1 + 1
```

- [ ] **Step 2: Fix `render_symbol_detail` (line 153)**

Change:
```rust
// Line 153: s.line_range.0, s.line_range.1
// becomes:
s.line_range.0 + 1, s.line_range.1 + 1
```

- [ ] **Step 3: Fix `render_context_bundle_found` (line 1845)**

Change:
```rust
// Line 1845: view.line_range.0,
// Line 1846: view.line_range.1,
// becomes:
view.line_range.0 + 1,
view.line_range.1 + 1,
```

- [ ] **Step 4: Fix dependency rendering (line 2235)**

Change:
```rust
// Line 2235: dep.line_range.0,
// Line 2236: dep.line_range.1,
// becomes:
dep.line_range.0 + 1,
dep.line_range.1 + 1,
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: All tests pass. Some test assertions may need updating if they check for specific line numbers in formatted output — fix those to expect 1-based values.

- [ ] **Step 6: Commit**

```bash
git add src/protocol/format.rs
git commit -m "fix: convert 0-based line ranges to 1-based in all formatters"
```

---

## Task 2: Add missing tools to daemon's execute_tool_call

**Files:**
- Modify: `src/daemon.rs:22-26` (imports)
- Modify: `src/daemon.rs:1217+` (execute_tool_call match arms)

**Context:** 8 tools are missing from the daemon dispatcher: `find_implementations`, `replace_symbol_body`, `insert_before_symbol`, `insert_after_symbol`, `delete_symbol`, `edit_within_symbol`, `batch_edit`, `batch_rename`. Missing tools cause "unknown tool" errors, which trigger daemon degradation cascade (proxy failure → daemon_degraded=true → broken git temporal). The edit input types live in `src/protocol/edit.rs`.

- [ ] **Step 1: Add missing imports to daemon.rs**

At the existing import block (line 22-26), add the missing input types. The edit tool inputs are in `crate::protocol::edit`:
```rust
use crate::protocol::edit::{
    BatchEditInput, BatchRenameInput, DeleteSymbolInput,
    EditWithinSymbolInput, InsertSymbolInput, ReplaceSymbolBodyInput,
};
use crate::protocol::tools::FindImplementationsInput;
```

- [ ] **Step 2: Add match arms for all 8 tools in execute_tool_call**

Add after the existing `diff_symbols` arm (around line 1260):
```rust
"find_implementations" => Ok(server
    .find_implementations(Parameters(decode_params::<FindImplementationsInput>(params)?))
    .await),
"replace_symbol_body" => Ok(server
    .replace_symbol_body(Parameters(decode_params::<ReplaceSymbolBodyInput>(params)?))
    .await),
"insert_before_symbol" => Ok(server
    .insert_before_symbol(Parameters(decode_params::<InsertSymbolInput>(params)?))
    .await),
"insert_after_symbol" => Ok(server
    .insert_after_symbol(Parameters(decode_params::<InsertSymbolInput>(params)?))
    .await),
"delete_symbol" => Ok(server
    .delete_symbol(Parameters(decode_params::<DeleteSymbolInput>(params)?))
    .await),
"edit_within_symbol" => Ok(server
    .edit_within_symbol(Parameters(decode_params::<EditWithinSymbolInput>(params)?))
    .await),
"batch_edit" => Ok(server
    .batch_edit(Parameters(decode_params::<BatchEditInput>(params)?))
    .await),
"batch_rename" => Ok(server
    .batch_rename(Parameters(decode_params::<BatchRenameInput>(params)?))
    .await),
```

NOTE: Verify the exact input type names by checking `src/protocol/edit.rs` and `src/protocol/tools.rs` for `FindImplementationsInput`. The insert tools both use `InsertSymbolInput` — confirm this.

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: All tests pass. The daemon test `test_daemon_port_if_compatible_accepts_matching_identity` and related tests should still pass.

- [ ] **Step 4: Commit**

```bash
git add src/daemon.rs
git commit -m "fix: register all 34 tools in daemon execute_tool_call"
```

---

## Task 3: Fix search_files changed_with to report Unavailable reason

**Files:**
- Modify: `src/protocol/tools.rs:1488-1515` (search_files handler, changed_with branch)

**Context:** When git temporal state is `Unavailable(reason)`, the current code falls through to a generic "not yet available" message, hiding the actual reason. It should report the reason like `get_co_changes` does.

- [ ] **Step 1: Replace the simple Ready check with a proper match**

Current code (lines 1491-1515):
```rust
if temporal.state == crate::live_index::git_temporal::GitTemporalState::Ready {
    // ... use data
}
return "Git temporal data is not yet available...";
```

Replace with:
```rust
match temporal.state {
    crate::live_index::git_temporal::GitTemporalState::Ready => {}
    crate::live_index::git_temporal::GitTemporalState::Unavailable(ref reason) => {
        return format!("Git temporal data unavailable: {reason}");
    }
    _ => {
        return "Git temporal data is still loading. Try again in a few seconds.".to_string();
    }
}
// ... existing Ready logic (the if let Some(history) block) moves here
```

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add src/protocol/tools.rs
git commit -m "fix: report git temporal unavailable reason in search_files changed_with"
```

---

## Task 4: Add path_prefix and language filtering to what_changed

**Files:**
- Modify: `src/protocol/tools.rs:280-289` (WhatChangedInput struct)
- Modify: `src/protocol/tools.rs:1636-1693` (what_changed handler)

**Context:** `what_changed` has no filtering params. The infrastructure (`parse_language_filter`, `LanguageId::from_extension`) already exists at `tools.rs:614-643` and `domain/index.rs:27-47` — just needs wiring.

- [ ] **Step 1: Add fields to WhatChangedInput**

After the existing `uncommitted` field:
```rust
/// Optional relative path prefix scope, for example `src/` or `src/protocol`.
pub path_prefix: Option<String>,
/// Optional canonical language name such as `Rust`, `TypeScript`, `C#`, or `C++`.
pub language: Option<String>,
```

- [ ] **Step 2: Add a shared filter helper function**

Place near line 661 (after `normalize_path_prefix`):
```rust
fn filter_paths_by_prefix_and_language(
    paths: Vec<String>,
    path_prefix: Option<&str>,
    language: Option<&str>,
) -> Result<Vec<String>, String> {
    let lang_filter = parse_language_filter(language)?;
    let prefix = path_prefix
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| {
            p.replace('\\', "/")
                .trim_start_matches("./")
                .trim_start_matches('/')
                .trim_end_matches('/')
                .to_string()
        });

    Ok(paths
        .into_iter()
        .filter(|path| {
            if let Some(ref pfx) = prefix {
                if !path.starts_with(pfx.as_str()) {
                    return false;
                }
            }
            if let Some(ref lang) = lang_filter {
                let ext = path.rsplit('.').next().unwrap_or("");
                if crate::domain::index::LanguageId::from_extension(ext).as_ref() != Some(lang) {
                    return false;
                }
            }
            true
        })
        .collect())
}
```

- [ ] **Step 3: Apply filter in the Uncommitted and GitRef branches**

In the `Uncommitted` branch (around line 1668), wrap the result:
```rust
Ok(output) => {
    let paths = parse_git_status_paths(&output);
    match filter_paths_by_prefix_and_language(
        paths,
        params.0.path_prefix.as_deref(),
        params.0.language.as_deref(),
    ) {
        Ok(filtered) => format::what_changed_paths_result(
            &filtered,
            "No uncommitted changes detected.",
        ),
        Err(e) => e,
    }
}
```

Same pattern in the `GitRef` branch (around line 1685):
```rust
Ok(output) => {
    let paths = parse_git_name_only_paths(&output);
    match filter_paths_by_prefix_and_language(
        paths,
        params.0.path_prefix.as_deref(),
        params.0.language.as_deref(),
    ) {
        Ok(filtered) => format::what_changed_paths_result(
            &filtered,
            &format!("No changes detected relative to git ref '{git_ref}'."),
        ),
        Err(e) => e,
    }
}
```

- [ ] **Step 4: Update the tool description**

Update the `#[tool(description = ...)]` at line 1634 to mention the new params:
```
"List changed files: uncommitted=true for working tree, git_ref for ref comparison, since for timestamp filter. Filter with path_prefix and/or language. NOT for symbol-level diffs (use diff_symbols)."
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add src/protocol/tools.rs
git commit -m "feat: add path_prefix and language filtering to what_changed"
```

---

## Task 5: Add language filtering to diff_symbols

**Files:**
- Modify: `src/protocol/tools.rs:505-512` (DiffSymbolsInput struct)
- Modify: `src/protocol/tools.rs:2031-2042` (diff_symbols handler, filter section)

**Context:** `diff_symbols` already has `path_prefix` but not `language`. This is the main fix for the "markdown files with embedded JS polluting results" issue. Uses same `parse_language_filter` infrastructure.

- [ ] **Step 1: Add language field to DiffSymbolsInput**

After the `path_prefix` field:
```rust
/// Optional canonical language name such as `Rust`, `TypeScript`, `C#`, or `C++`.
pub language: Option<String>,
```

- [ ] **Step 2: Add language filter to the changed_files filtering logic**

Replace the filter block (lines 2032-2042):
```rust
let lang_filter = match parse_language_filter(params.0.language.as_deref()) {
    Ok(f) => f,
    Err(e) => return e,
};
let changed_files: Vec<&str> = changed_files_owned
    .iter()
    .map(|s| s.as_str())
    .filter(|p| {
        if let Some(ref prefix) = params.0.path_prefix {
            if !p.starts_with(prefix.as_str()) {
                return false;
            }
        }
        if let Some(ref lang) = lang_filter {
            let ext = p.rsplit('.').next().unwrap_or("");
            if crate::domain::index::LanguageId::from_extension(ext).as_ref() != Some(lang) {
                return false;
            }
        }
        true
    })
    .collect();
```

- [ ] **Step 3: Update tool description**

```
"Symbol-level diff between two git refs. Shows +added, -removed, ~modified symbols per changed file. Filter with path_prefix and/or language. NOT for file-level change lists (use what_changed)."
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add src/protocol/tools.rs
git commit -m "feat: add language filtering to diff_symbols"
```

---

## Task 6: Improve explore tool — filter noise from text patterns

**Files:**
- Modify: `src/protocol/format.rs` (add `is_noise_line` public function, near line 370)
- Modify: `src/protocol/format.rs:375-385` (refactor usage grouping to call `is_noise_line`)
- Modify: `src/protocol/tools.rs:1924-1941` (explore text hit collection)

**Context:** The explore tool's "Code patterns" section surfaces import lines and comments because it collects raw `m.line` strings with zero filtering. The `search_text` tool already filters these in `group_by=usage` mode (format.rs:375-385). We extract that logic into a shared function and apply it in explore.

- [ ] **Step 1: Add `is_noise_line` utility function in format.rs**

Add before the `usage` grouping block (near line 370):
```rust
/// Returns true if the line looks like an import statement or a non-doc comment.
/// Used by `search_text` usage grouping and `explore` to filter noise.
pub fn is_noise_line(line: &str) -> bool {
    let trimmed = line.trim();
    // Allow doc comments through
    if trimmed.starts_with("///") || trimmed.starts_with("//!") || trimmed.starts_with("/**") {
        return false;
    }
    trimmed.starts_with("use ")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("require(")
        || trimmed.starts_with("#include")
        || trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("--")
        || line.contains("require(")
}
```

- [ ] **Step 2: Refactor usage grouping to call `is_noise_line`**

Replace lines 375-387:
```rust
// Before:
let is_import_or_comment = trimmed.starts_with("use ") || ...;
if is_import_or_comment { continue; }

// After:
if is_noise_line(&line_match.line) { continue; }
```

- [ ] **Step 3: Apply `is_noise_line` filter in explore text collection**

In `tools.rs`, change the explore text hit loop (lines 1933-1939):
```rust
for m in &file.matches {
    if text_hits.len() >= limit {
        break;
    }
    if format::is_noise_line(&m.line) {
        continue;
    }
    text_hits.push((file.path.clone(), m.line.clone(), m.line_number));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add src/protocol/format.rs src/protocol/tools.rs
git commit -m "feat: filter imports and comments from explore code patterns"
```

---

## Task 7: Git temporal robustness — avoid resetting Ready state

**Files:**
- Modify: `src/live_index/git_temporal.rs:32-66` (spawn_git_temporal_computation)

**Context:** `spawn_git_temporal_computation` unconditionally sets state to `Computing`, even if data is already `Ready`. This means any re-spawn (reload, index_folder) creates a window where queries fail. The fix: only set `Computing` if the current state is not `Ready`, or if the data is stale.

- [ ] **Step 1: Add staleness check before overwriting Ready state**

Change `spawn_git_temporal_computation` to check current state:
```rust
pub fn spawn_git_temporal_computation(index: SharedIndex, repo_root: PathBuf) {
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }

    // Don't clobber existing Ready data — serve stale while recomputing.
    let current = index.git_temporal();
    let was_ready = current.state == GitTemporalState::Ready;

    if !was_ready {
        index.update_git_temporal(GitTemporalIndex {
            state: GitTemporalState::Computing,
            ..GitTemporalIndex::pending()
        });
    }

    tokio::spawn(async move {
        let result =
            tokio::task::spawn_blocking(move || GitTemporalIndex::compute(&repo_root)).await;

        match result {
            Ok(temporal) => {
                tracing::info!(
                    files = temporal.files.len(),
                    commits = temporal.stats.total_commits_analyzed,
                    duration_ms = temporal.stats.compute_duration.as_millis() as u64,
                    "git temporal index computed"
                );
                index.update_git_temporal(temporal);
            }
            Err(error) => {
                tracing::warn!("git temporal computation panicked: {error}");
                // Only overwrite with Unavailable if we weren't serving Ready data
                if !was_ready {
                    index.update_git_temporal(GitTemporalIndex::unavailable(format!(
                        "computation panicked: {error}"
                    )));
                }
            }
        }
    });
}
```

This preserves existing `Ready` data during recomputation, so queries continue working during background refresh.

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: All pass. The `test_pending_state` test should still pass since it creates a fresh index (not Ready).

- [ ] **Step 3: Commit**

```bash
git add src/live_index/git_temporal.rs
git commit -m "fix: preserve Ready git temporal data during background recomputation"
```

---

## Summary

| Task | Type | Files | Impact |
|------|------|-------|--------|
| 1 | Bug fix | format.rs | 0-based line ranges → 1-based in 4 formatters |
| 2 | Bug fix | daemon.rs | Register all 34 tools in daemon dispatcher |
| 3 | Bug fix | tools.rs | Report Unavailable reason in search_files |
| 4 | Feature | tools.rs | path_prefix + language on what_changed |
| 5 | Feature | tools.rs | language filter on diff_symbols |
| 6 | Feature | format.rs, tools.rs | Filter noise from explore patterns |
| 7 | Robustness | git_temporal.rs | Don't reset Ready during recomputation |

Total: 7 tasks, 5 files modified, all independent (can be done in any order).

Tool consolidation (34→25) is deferred to a separate plan — it's a larger architectural change that deserves its own branch and review cycle.
