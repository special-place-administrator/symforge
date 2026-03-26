# Reviewer Suggestions Wave 1 — Quick Wins

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement 4 low-effort, high-value improvements from external AI review feedback.

**Architecture:** Each fix is independent — touches different functions in `src/protocol/edit.rs`, `src/protocol/tools.rs`, and `src/protocol/format.rs`. No cross-dependencies between tasks.

**Tech Stack:** Rust, SymForge MCP server codebase

---

## Task 1: Add `code_only` flag to `batch_rename`

**Problem:** `batch_rename` includes markdown/doc files when renaming symbols, corrupting documentation.

**Files:**
- Modify: `src/protocol/edit.rs` — `BatchRenameInput` struct (line ~970) and `execute_batch_rename` fn (line ~1027)
- Reference: `src/protocol/tools.rs` — `filter_paths_by_prefix_and_language` (line ~758) for the `code_only` pattern

**Approach:** Follow the established `code_only` pattern from `WhatChangedInput` and `DiffSymbolsInput`.

- [ ] **Step 1: Add `code_only` field to `BatchRenameInput`**

In `src/protocol/edit.rs`, add to the `BatchRenameInput` struct after `dry_run`:

```rust
/// When true, exclude non-source files (docs, configs, images) from renaming.
/// Only files with a recognized programming language extension are included.
#[serde(default, deserialize_with = "super::tools::lenient_bool")]
pub code_only: Option<bool>,
```

- [ ] **Step 2: Filter Phase 2 ref_sites by code_only**

In `execute_batch_rename`, after collecting `ref_sites` (around line ~1058-1065), add filtering:

```rust
// Filter ref_sites by code_only
let ref_sites: Vec<(String, (u32, u32))> = if input.code_only.unwrap_or(false) {
    ref_sites.into_iter().filter(|(path, _)| {
        let ext = path.rsplit('.').next().unwrap_or("");
        match crate::domain::index::LanguageId::from_extension(ext) {
            None => false,
            Some(lang) => !crate::parsing::config_extractors::is_config_language(&lang),
        }
    }).collect()
} else {
    ref_sites
};
```

- [ ] **Step 3: Filter Phase 2b file_contents scan by code_only**

In `execute_batch_rename`, when collecting `file_contents` (around line ~1074-1082), add filter:

```rust
let file_contents: Vec<(String, Vec<u8>)> = {
    let guard = index.read();
    guard
        .files
        .iter()
        .filter(|(path, _)| {
            if !input.code_only.unwrap_or(false) {
                return true;
            }
            let ext = path.rsplit('.').next().unwrap_or("");
            match crate::domain::index::LanguageId::from_extension(ext) {
                None => false,
                Some(lang) => !crate::parsing::config_extractors::is_config_language(&lang),
            }
        })
        .map(|(path, file)| (path.clone(), file.content.clone()))
        .collect()
};
```

- [ ] **Step 4: Update tool description**

In `src/protocol/tools.rs` at line ~3897, append to the batch_rename description:
`" Set code_only=true to exclude non-source files (docs, configs) from renaming."`

- [ ] **Step 5: Build and verify**

Run: `cargo check`
Expected: clean compilation

- [ ] **Step 6: Commit**

```bash
git add src/protocol/edit.rs src/protocol/tools.rs
git commit -m "feat(batch_rename): add code_only flag to exclude non-source files"
```

---

## Task 2: Add external trait hint to `find_references` implementations mode

**Problem:** When searching for an external trait (e.g. `serde::DeserializeOwned`), `find_references` with `mode="implementations"` returns "No implementations found" with no hint that the trait isn't in the indexed project.

**Files:**
- Modify: `src/protocol/tools.rs` — `find_references` fn (line ~2838), specifically the empty-results branch (line ~2858-2879)

**Approach:** After checking `is_concrete`, add an `else` branch that checks whether the name exists as ANY symbol definition in the index. If not, add a helpful hint.

- [ ] **Step 1: Add external symbol detection**

In `src/protocol/tools.rs`, in the `find_references` function, after the existing `if is_concrete { ... }` block (around line ~2875), add an else clause:

```rust
if is_concrete {
    return format!(
        "No implementations found for \"{}\" — it is a class/struct, not an \
         interface/trait.\nUse find_references with mode=\"references\" to find \
         callers and usages instead.",
        input.name
    );
} else {
    // Check if the symbol exists at all in the indexed project
    let exists_in_project = guard.all_files().any(|(_, file)| {
        file.symbols.iter().any(|s| s.name == input.name)
    });
    drop(guard);
    if !exists_in_project {
        return format!(
            "No implementations found for \"{}\" — this symbol is not defined \
             in the indexed project (likely from an external dependency).\n\
             Use search_text to find usages of this symbol in your code instead.",
            input.name
        );
    }
}
```

Note: the `guard` is already acquired before the `is_concrete` check. The existing code has a `drop(guard)` after the `is_concrete` check — we need to restructure so the `drop` happens after both branches. Currently the `drop(guard)` is implicit (goes out of scope). Check the exact code flow.

- [ ] **Step 2: Build and verify**

Run: `cargo check`
Expected: clean compilation

- [ ] **Step 3: Commit**

```bash
git add src/protocol/tools.rs
git commit -m "feat(find_references): hint when implementations search targets external symbol"
```

---

## Task 3: Add follow_refs clarification note to search_text output

**Problem:** `search_text` with `follow_refs=true` shows callers of the enclosing symbol, not callers that reference the search text. This is correct but confusing.

**Files:**
- Modify: `src/protocol/format.rs` — `search_text_result_view` fn, around line ~534 where "Called by:" is rendered

**Approach:** Add a one-time note at the bottom of the output when `follow_refs` data is present, explaining the behavior.

- [ ] **Step 1: Add clarification note**

In `src/protocol/format.rs`, in `search_text_result_view`, after the main loop that builds the output (just before the final `lines.join("\n")` at line ~538), add:

```rust
// Add follow_refs clarification if any callers were included
let has_callers = result.files.iter().any(|f| f.callers.is_some());
if has_callers {
    lines.push(String::new());
    lines.push("Note: \"Called by\" lists callers of the enclosing symbol, not callers that reference the search text.".to_string());
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo check`
Expected: clean compilation

- [ ] **Step 3: Commit**

```bash
git add src/protocol/format.rs
git commit -m "feat(search_text): add clarification note for follow_refs caller output"
```

---

## Task 4: Enhance diff_symbols compact mode with symbol names

**Problem:** Compact mode shows `state.rs (+2)` but doesn't tell you WHAT was added. Need `state.rs (+2: TEST_STATE_LOCK, state_test_guard)`.

**Files:**
- Modify: `src/protocol/format.rs` — `diff_symbols_result_view` fn, compact mode block (lines ~6414-6426)

**Approach:** The `file_added`, `file_removed`, `file_modified` vectors already contain the symbol names (they're `Vec<&str>`). Include up to 3 names in the compact notation.

- [ ] **Step 1: Enhance compact format to include symbol names**

In `src/protocol/format.rs`, replace the compact mode block (lines ~6414-6426):

```rust
if compact {
    // Compact mode: one line per file with counts and symbol names
    let mut parts = Vec::new();
    if !file_added.is_empty() {
        let names = compact_symbol_list(&file_added);
        parts.push(format!("+{}: {}", file_added.len(), names));
    }
    if !file_removed.is_empty() {
        let names = compact_symbol_list(&file_removed);
        parts.push(format!("-{}: {}", file_removed.len(), names));
    }
    if !file_modified.is_empty() {
        let names = compact_symbol_list(&file_modified);
        parts.push(format!("~{}: {}", file_modified.len(), names));
    }
    lines.push(format!("  {} ({})", file_path, parts.join(", ")));
}
```

And add a helper function nearby:

```rust
/// Format a list of symbol names for compact display: up to 3 names, then "..."
fn compact_symbol_list(names: &[&str]) -> String {
    let mut sorted: Vec<&str> = names.to_vec();
    sorted.sort_unstable();
    if sorted.len() <= 3 {
        sorted.join(", ")
    } else {
        format!("{}, ... +{} more", sorted[..3].join(", "), sorted.len() - 3)
    }
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo check`
Expected: clean compilation

- [ ] **Step 3: Commit**

```bash
git add src/protocol/format.rs
git commit -m "feat(diff_symbols): show symbol names in compact mode output"
```
