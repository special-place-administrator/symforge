# Sprint 13 — Eval Quality Sweep Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 14 issues (4 bugs, 10 UX) identified in the 5-project evaluation to improve tool quality across get_file_content, edit tools, explore/search, and diagnostics.

**Architecture:** Wave-based execution. Wave 0 defines contracts (no code). Waves 1-5 implement changes with tests first. Each wave's items are independent except where noted.

**Tech Stack:** Rust, tree-sitter, serde, MCP protocol. Tests: `cargo test --all-targets -- --test-threads=1`

**Spec:** `docs/superpowers/specs/2026-03-15-sprint-13-eval-quality-sweep-design.md`

---

## Chunk 1: Wave 1 — Noise Foundation (U7)

### File Structure

| Action | Path | Responsibility |
|--------|------|---------------|
| Modify | `src/live_index/search.rs` | Add gitignore fields to `NoisePolicy`, classification logic |
| Modify | `src/discovery/mod.rs` | Load `.gitignore` patterns at discovery time |
| Modify | `src/live_index/store.rs` | Store noise classification per `IndexedFile` |
| Modify | `src/protocol/format.rs` | Tag noise files in `get_repo_map` output |
| Create | `tests/noise_policy_integration.rs` | Integration tests for gitignore-aware noise |

### Task 1: U7 — `.gitignore`-aware noise policy

**Files:**
- Modify: `src/live_index/search.rs` (NoisePolicy struct, L231-265)
- Modify: `src/domain/index.rs` (IndexedFile or FileClassification)
- Modify: `src/discovery/mod.rs`
- Modify: `src/protocol/format.rs` (repo_map output, ~L838)
- Test: `src/live_index/search.rs` (inline tests) + `tests/noise_policy_integration.rs`

**Dependencies:** `ignore` crate (Rust gitignore implementation). Check if already in `Cargo.toml`; if not, add it.

- [ ] **Step 1: Check if `ignore` crate is available**

Run: `grep 'ignore' Cargo.toml`

If not present, add to `[dependencies]`:
```toml
ignore = "0.4"
```

- [ ] **Step 2: Add `NoiseClass` enum and extend `NoisePolicy`**

In `src/live_index/search.rs`, add above the existing `NoisePolicy` struct:

```rust
/// Classification for files that should be de-prioritized in explore/repo_map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseClass {
    /// Normal source file.
    None,
    /// File matches a `.gitignore` pattern or lives in a vendor directory.
    Vendor,
    /// File is generated (e.g., lock files, build artifacts).
    Generated,
}

impl Default for NoiseClass {
    fn default() -> Self {
        Self::None
    }
}
```

Extend `NoisePolicy` with a method to classify paths:

```rust
impl NoisePolicy {
    /// Classify a file path as noise based on gitignore patterns and heuristics.
    pub fn classify_path(&self, path: &str, gitignore: Option<&ignore::gitignore::Gitignore>) -> NoiseClass {
        // Check gitignore first
        if let Some(gi) = gitignore {
            if gi.matched(path, false).is_ignore() {
                return NoiseClass::Vendor;
            }
        }

        // Heuristic classification for common vendor/generated patterns
        let lower = path.to_lowercase();
        if lower.contains("vendor/")
            || lower.contains("node_modules/")
            || lower.contains("third_party/")
            || lower.contains("third-party/")
        {
            return NoiseClass::Vendor;
        }

        if lower.ends_with(".lock")
            || lower.ends_with(".min.js")
            || lower.ends_with(".min.css")
            || lower.contains("/generated/")
            || lower.contains("/dist/")
        {
            return NoiseClass::Generated;
        }

        NoiseClass::None
    }

    /// Returns true if the given noise class should be hidden under this policy.
    pub fn should_hide(&self, class: NoiseClass) -> bool {
        match class {
            NoiseClass::None => false,
            NoiseClass::Vendor => !self.include_vendor,
            NoiseClass::Generated => !self.include_generated,
        }
    }
}
```

- [ ] **Step 3: Write failing test for noise classification**

Add to the test module in `src/live_index/search.rs`:

```rust
#[test]
fn test_noise_policy_classifies_vendor_paths() {
    let policy = NoisePolicy::hide_classified_noise();
    assert_eq!(policy.classify_path("vendor/lib/foo.js", None), NoiseClass::Vendor);
    assert_eq!(policy.classify_path("node_modules/pkg/index.js", None), NoiseClass::Vendor);
    assert_eq!(policy.classify_path("src/main.rs", None), NoiseClass::None);
}

#[test]
fn test_noise_policy_classifies_generated_paths() {
    let policy = NoisePolicy::hide_classified_noise();
    assert_eq!(policy.classify_path("Cargo.lock", None), NoiseClass::Generated);
    assert_eq!(policy.classify_path("dist/bundle.min.js", None), NoiseClass::Generated);
}

#[test]
fn test_noise_policy_should_hide_respects_flags() {
    let mut policy = NoisePolicy::hide_classified_noise();
    assert!(policy.should_hide(NoiseClass::Vendor));
    policy.include_vendor = true;
    assert!(!policy.should_hide(NoiseClass::Vendor));
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test test_noise_policy_classifies -- --test-threads=1`
Expected: FAIL (NoiseClass not defined yet or classify_path not implemented)

- [ ] **Step 5: Implement and verify tests pass**

Run: `cargo test test_noise_policy -- --test-threads=1`
Expected: All 3 tests PASS

- [ ] **Step 6: Add gitignore loading to discovery**

In `src/discovery/mod.rs`, add a function to load gitignore patterns:

```rust
/// Load `.gitignore` patterns from the repo root and nested directories.
pub fn load_gitignore(repo_root: &Path) -> Option<ignore::gitignore::Gitignore> {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(repo_root);
    let gi_path = repo_root.join(".gitignore");
    if gi_path.exists() {
        let _ = builder.add(&gi_path);
    }
    // Walk for nested .gitignore files
    for entry in walkdir::WalkDir::new(repo_root)
        .max_depth(6)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name() == ".gitignore" && entry.path() != gi_path {
            let _ = builder.add(entry.path());
        }
    }
    builder.build().ok()
}
```

- [ ] **Step 7: Write gitignore integration test**

Create `tests/noise_policy_integration.rs`:

```rust
//! Integration tests for gitignore-aware noise classification.

use std::fs;
use tempfile::TempDir;

#[test]
fn test_gitignore_vendor_classified_as_noise() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".gitignore"), "vendor/\n").unwrap();
    fs::create_dir_all(dir.path().join("vendor")).unwrap();
    fs::write(dir.path().join("vendor/lib.js"), "// vendor").unwrap();
    fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

    // Load gitignore and classify
    let gi = tokenizor::discovery::load_gitignore(dir.path()).unwrap();
    let policy = tokenizor::live_index::search::NoisePolicy::hide_classified_noise();
    assert_eq!(
        policy.classify_path("vendor/lib.js", Some(&gi)),
        tokenizor::live_index::search::NoiseClass::Vendor,
    );
    assert_eq!(
        policy.classify_path("src/main.rs", Some(&gi)),
        tokenizor::live_index::search::NoiseClass::None,
    );
}

#[test]
fn test_gitignore_negation_exempts_file() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".gitignore"), "vendor/\n!vendor/important.js\n").unwrap();
    fs::create_dir_all(dir.path().join("vendor")).unwrap();
    fs::write(dir.path().join("vendor/important.js"), "// keep").unwrap();

    let gi = tokenizor::discovery::load_gitignore(dir.path()).unwrap();
    let policy = tokenizor::live_index::search::NoisePolicy::hide_classified_noise();
    // Negated file should NOT be classified as noise
    assert_eq!(
        policy.classify_path("vendor/important.js", Some(&gi)),
        tokenizor::live_index::search::NoiseClass::None,
    );
}
```

- [ ] **Step 8: Run integration tests**

Run: `cargo test --test noise_policy_integration -- --test-threads=1`
Expected: PASS (adjust module paths as needed for the crate's public API)

- [ ] **Step 9: Tag noise files in `get_repo_map` output**

In `src/protocol/format.rs`, find the `file_tree_view` function (used by get_repo_map with detail="tree"). When rendering file entries, append `[vendor]` or `[generated]` tag based on `NoiseClass`:

```rust
// In the file tree rendering, after the file name:
let noise_tag = match file.noise_class {
    NoiseClass::Vendor => " [vendor]",
    NoiseClass::Generated => " [generated]",
    NoiseClass::None => "",
};
```

- [ ] **Step 10: Write test for repo_map tagging**

```rust
#[test]
fn test_repo_map_tree_tags_vendor_files() {
    // Create index with a vendor-classified file
    // Assert output contains "[vendor]" tag
}
```

- [ ] **Step 11: Run full test suite**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: All tests PASS

- [ ] **Step 12: Commit**

```bash
git add -A
git commit -m "feat: gitignore-aware noise policy (U7)

Add NoiseClass enum and gitignore pattern loading to NoisePolicy.
Files matching .gitignore patterns are classified as Vendor noise.
Supports negation rules and nested .gitignore files. Patterns loaded
at index time only (no mid-session refresh).

get_repo_map tags noise files with [vendor] or [generated] labels."
```

---

## Chunk 2: Wave 2 — Independent Fixes (Part 1: Edit Tools)

### Task 2: B1 — `batch_insert` extra blank line

**Files:**
- Modify: `src/protocol/edit.rs` (`build_insert_before`, L130-151)
- Test: `src/protocol/edit.rs` (inline tests)

The bug: `build_insert_before` always appends `\n\n` (no doc comments) or `\n` (doc comments) after inserted code. If the file already has a blank line before the target symbol, the result is a double blank line.

- [ ] **Step 1: Write failing test — extra blank line when file already has one**

Add in `src/protocol/edit.rs` test module:

```rust
#[test]
fn test_build_insert_before_no_double_blank_line() {
    // File has a blank line before the function
    let content = b"use std::io;\n\n\nfn target() {}\n";
    let sym = SymbolRecord {
        name: "target".to_string(),
        kind: SymbolKind::Function,
        depth: 0,
        sort_order: 0,
        byte_range: (15, 30),  // "fn target() {}\n"
        line_range: (3, 3),
        doc_byte_range: None,
    };
    let result = build_insert_before(content, &sym, "fn inserted() {}");
    let text = String::from_utf8_lossy(&result);
    // Should NOT have 3+ consecutive newlines (double blank line)
    assert!(!text.contains("\n\n\n"), "Got triple newline:\n{text}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_build_insert_before_no_double_blank_line -- --test-threads=1`
Expected: FAIL — triple newline present

- [ ] **Step 3: Fix `build_insert_before`**

In `build_insert_before` (L130-151), after computing `line_start`, check whether a blank line already precedes the splice point. If so, use `\n` instead of `\n\n`:

```rust
// Replace the separator logic (L145-149):
let separator = if sym.doc_byte_range.is_some() {
    b"\n" as &[u8]
} else {
    // Check if there's already a blank line before the symbol
    let prefix = &file_content[..line_start as usize];
    // Check if there are any trailing blank lines (one or more \n\n sequences)
    let has_trailing_blank = prefix.len() >= 2
        && prefix[prefix.len() - 1] == b'\n'
        && prefix[prefix.len() - 2] == b'\n';
    let already_has_blank = has_trailing_blank || prefix.is_empty();
    if already_has_blank { b"\n" } else { b"\n\n" }
};
```

- [ ] **Step 4: Run full test matrix**

Run: `cargo test test_build_insert_before -- --test-threads=1`
Expected: All `build_insert_before` tests PASS, including existing tests for doc comments and no-doc-comments cases.

- [ ] **Step 5: Add additional test cases**

```rust
#[test]
fn test_build_insert_before_first_symbol_in_file() {
    let content = b"fn target() {}\n";
    let sym = SymbolRecord {
        name: "target".to_string(),
        kind: SymbolKind::Function,
        depth: 0,
        sort_order: 0,
        byte_range: (0, 15),
        line_range: (0, 0),
        doc_byte_range: None,
    };
    let result = build_insert_before(content, &sym, "fn inserted() {}");
    let text = String::from_utf8_lossy(&result);
    assert!(!text.contains("\n\n\n"), "Got triple newline:\n{text}");
    assert!(text.starts_with("fn inserted() {}"));
}

#[test]
fn test_build_insert_before_with_attributes() {
    let content = b"use std::io;\n\n#[derive(Debug)]\nfn target() {}\n";
    let sym = SymbolRecord {
        name: "target".to_string(),
        kind: SymbolKind::Function,
        depth: 0,
        sort_order: 0,
        byte_range: (30, 45),
        line_range: (3, 3),
        doc_byte_range: Some((14, 29)),  // #[derive(Debug)]
    };
    let result = build_insert_before(content, &sym, "fn inserted() {}");
    let text = String::from_utf8_lossy(&result);
    // With doc_byte_range, separator is \n — no double blank
    assert!(!text.contains("\n\n\n"), "Got triple newline:\n{text}");
}
```

- [ ] **Step 6: Run and verify**

Run: `cargo test test_build_insert_before -- --test-threads=1`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add src/protocol/edit.rs
git commit -m "fix: batch_insert no extra blank line before function (B1)

build_insert_before now checks for existing blank lines before
the splice point. Uses single newline when a blank line already
separates the insertion from the target symbol."
```

### Task 3: B4 — `batch_edit` rollback message

**Files:**
- Modify: `src/protocol/edit.rs` (`execute_batch_edit`, L432-596)
- Test: `src/protocol/edit.rs` (inline tests)

The current error path returns bare error strings. Need to add explicit rollback context.

- [ ] **Step 1: Write failing test — rollback message on validation failure**

```rust
#[test]
fn test_execute_batch_edit_rollback_message_on_validation_failure() {
    let (index, _dir) = create_test_index_with_content(
        "test.rs",
        "fn alpha() {}\nfn beta() {}\n",
    );
    let repo_root = _dir.path().to_path_buf();

    let edits = vec![
        SingleEdit {
            path: "test.rs".to_string(),
            name: "alpha".to_string(),
            kind: None,
            symbol_line: None,
            operation: EditOperation::Replace {
                new_body: "fn alpha_new() {}".to_string(),
            },
        },
        SingleEdit {
            path: "test.rs".to_string(),
            name: "nonexistent".to_string(),
            kind: None,
            symbol_line: None,
            operation: EditOperation::Replace {
                new_body: "fn nope() {}".to_string(),
            },
        },
    ];

    let result = execute_batch_edit(&index, &repo_root, &edits);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("ROLLED BACK"), "Missing rollback label: {err}");
    assert!(err.contains("No files were modified"), "Missing no-write confirmation: {err}");
    assert!(err.contains("2 edits"), "Missing edit count: {err}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_execute_batch_edit_rollback_message -- --test-threads=1`
Expected: FAIL — error message doesn't contain "ROLLED BACK"

- [ ] **Step 3: Implement rollback message**

In `execute_batch_edit`, wrap Phase 1 and Phase 1b error returns to include rollback context:

```rust
// Phase 1: Resolve all symbols — wrap errors
// Replace: .map_err(|e| format!("Edit {}: {e}", i + 1))?;
// With:
.map_err(|e| format!(
    "ROLLED BACK — {} edits targeting {} files. Edit {}: {e}. No files were modified.",
    edits.len(),
    edits.iter().map(|e| &e.path).collect::<std::collections::HashSet<_>>().len(),
    i + 1,
))?;
```

Apply the same pattern to Phase 1b overlap errors and Phase 3 write errors (noting which files were already written if a late write fails).

- [ ] **Step 4: Run tests**

Run: `cargo test test_execute_batch_edit -- --test-threads=1`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add src/protocol/edit.rs
git commit -m "fix: batch_edit shows ROLLED BACK message on failure (B4)

Error output now includes: ROLLED BACK status, edit count,
targeted file paths, and confirmation that no files were modified
(for validation-phase failures)."
```

### Task 4: U5 — `batch_edit` dry-run mode

**Files:**
- Modify: `src/protocol/edit.rs` (`BatchEditInput`, `execute_batch_edit`)
- Modify: `src/protocol/tools.rs` (`batch_edit` handler)
- Test: `src/protocol/edit.rs`

- [ ] **Step 1: Add `dry_run` field to `BatchEditInput`**

In `src/protocol/edit.rs`, `BatchEditInput` struct (L389-392):

```rust
pub struct BatchEditInput {
    pub edits: Vec<SingleEdit>,
    /// When true, validate and preview all edits without writing to disk.
    #[serde(default)]
    pub dry_run: bool,
}
```

- [ ] **Step 2: Write failing test**

```rust
#[test]
fn test_execute_batch_edit_dry_run_previews_without_writing() {
    let (index, dir) = create_test_index_with_content(
        "test.rs",
        "fn alpha() { 1 }\nfn beta() { 2 }\n",
    );
    let repo_root = dir.path().to_path_buf();

    let edits = vec![SingleEdit {
        path: "test.rs".to_string(),
        name: "alpha".to_string(),
        kind: None,
        symbol_line: None,
        operation: EditOperation::Replace {
            new_body: "fn alpha() { 999 }".to_string(),
        },
    }];

    let result = execute_batch_edit_impl(&index, &repo_root, &edits, /* dry_run: */ true);
    assert!(result.is_ok());
    let summaries = result.unwrap();
    assert!(!summaries.is_empty(), "Dry-run should produce preview output");

    // Verify file was NOT modified on disk
    let disk_content = std::fs::read_to_string(dir.path().join("test.rs")).unwrap();
    assert!(disk_content.contains("{ 1 }"), "File should be unchanged after dry-run");
}

#[test]
fn test_execute_batch_edit_dry_run_same_error_as_real() {
    let (index, dir) = create_test_index_with_content(
        "test.rs",
        "fn alpha() {}\n",
    );
    let repo_root = dir.path().to_path_buf();

    let edits = vec![SingleEdit {
        path: "test.rs".to_string(),
        name: "nonexistent".to_string(),
        kind: None,
        symbol_line: None,
        operation: EditOperation::Replace {
            new_body: "fn nope() {}".to_string(),
        },
    }];

    let real = execute_batch_edit_impl(&index, &repo_root, &edits, false);
    let dry = execute_batch_edit_impl(&index, &repo_root, &edits, true);
    // Both should fail with similar errors
    assert!(real.is_err());
    assert!(dry.is_err());
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test test_execute_batch_edit_dry_run -- --test-threads=1`
Expected: FAIL — `execute_batch_edit_impl` doesn't exist yet

- [ ] **Step 4: Refactor `execute_batch_edit` to accept `dry_run` flag**

Rename existing `execute_batch_edit` to `execute_batch_edit_impl` with a `dry_run: bool` parameter. The original `execute_batch_edit` becomes a thin wrapper:

```rust
pub(crate) fn execute_batch_edit(
    index: &SharedIndex,
    repo_root: &Path,
    edits: &[SingleEdit],
) -> Result<Vec<String>, String> {
    execute_batch_edit_impl(index, repo_root, edits, false)
}

fn execute_batch_edit_impl(
    index: &SharedIndex,
    repo_root: &Path,
    edits: &[SingleEdit],
    dry_run: bool,
) -> Result<Vec<String>, String> {
    // ... existing Phase 1 and Phase 1b (validation) — unchanged ...

    // Phase 3: Apply edits — gate on dry_run
    for (path, indices) in &by_file {
        // ... compute new content as before ...

        if dry_run {
            // Preview: show what would change per edit (capped at 20 lines)
            for &ri in indices {
                let r = &resolved[ri];
                let edit = &edits[r.operation];
                summaries.push(format!(
                    "[DRY RUN] Would {} `{}` in {}",
                    match &edit.operation {
                        EditOperation::Replace { .. } => "replace",
                        EditOperation::InsertBefore { .. } => "insert before",
                        EditOperation::InsertAfter { .. } => "insert after",
                        EditOperation::Delete => "delete",
                        EditOperation::EditWithin { .. } => "edit within",
                    },
                    r.sym.name,
                    path,
                ));
            }
        } else {
            // Real write path — existing code
            atomic_write_file(&abs_path, &content)
                .map_err(|e| format!("Write failed for {path}: {e}"))?;
            reindex_after_write(index, path, content, language);
        }
    }

    Ok(summaries)
}
```

**Critical:** The validation path (Phase 1, 1b, 2) is shared. Only Phase 3 write+reindex is gated.

- [ ] **Step 5: Update `batch_edit` handler in tools.rs**

In the `batch_edit` method, pass `input.dry_run`:

```rust
// Replace: edit::execute_batch_edit(&self.index, &repo_root, &params.0.edits)
// With: edit::execute_batch_edit_with_mode(&self.index, &repo_root, &params.0.edits, params.0.dry_run)
```

Or update `execute_batch_edit` to accept `&BatchEditInput` directly.

- [ ] **Step 6: Run tests**

Run: `cargo test test_execute_batch_edit -- --test-threads=1`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add src/protocol/edit.rs src/protocol/tools.rs
git commit -m "feat: batch_edit dry_run mode (U5)

dry_run=true validates all edits through the same code path as
real execution but skips disk writes and index mutation. Preview
output shows what each edit would do."
```

---

## Chunk 3: Wave 2 — Independent Fixes (Part 2: Search, Output, Diagnostics)

### Task 5: U2 — `search_symbols` browse mode

**Files:**
- Modify: `src/protocol/tools.rs` (`SearchSymbolsInput` L186-204, `search_symbols_options_from_input` L756-770, `search_symbols` handler L1573+)
- Test: `src/protocol/tools.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn test_search_symbols_browse_by_kind_and_path_without_query() {
    // Setup index with structs in src/protocol/
    // Call search_symbols with query=None, kind="struct", path_prefix="src/protocol/"
    // Assert results returned
}

#[test]
fn test_search_symbols_rejects_fully_unbounded_request() {
    // Call with query=None, kind=None, path_prefix=None
    // Assert error: "search_symbols requires at least one of: query, kind, or path_prefix"
}

#[test]
fn test_search_symbols_browse_mode_default_limit_20() {
    // Call with kind="fn", path_prefix=None, query=None, limit=None
    // Assert result count <= 20
}

#[test]
fn test_search_symbols_query_mode_still_defaults_to_50() {
    // Call with query="test", limit=None
    // Assert limit used is 50 (check via result count on large index)
}
```

- [ ] **Step 2: Make `query` optional in `SearchSymbolsInput`**

```rust
pub struct SearchSymbolsInput {
    /// Search query (case-insensitive substring match). Optional — omit for browse mode.
    pub query: Option<String>,  // Changed from String to Option<String>
    // ... rest unchanged
}
```

- [ ] **Step 3: Add validation in handler**

In the `search_symbols` handler, before calling `search_symbols_options_from_input`:

```rust
// Validate browse mode guardrails
if input.query.as_ref().map(|q| q.trim().is_empty()).unwrap_or(true)
    && input.kind.is_none()
    && input.path_prefix.is_none()
{
    return "Invalid search_symbols request: requires at least one of: query, kind, or path_prefix.".to_string();
}
```

- [ ] **Step 4: Adjust limit logic in `search_symbols_options_from_input`**

```rust
let is_browse = input.query.as_ref().map(|q| q.trim().is_empty()).unwrap_or(true);
let default_limit = if is_browse { 20 } else { 50 };
result_limit: search::ResultLimit::new(input.limit.unwrap_or(default_limit).min(100) as usize),
```

- [ ] **Step 5: Update downstream callers of `SearchSymbolsInput.query`**

Anywhere `input.query` is used as `&str`, change to `input.query.as_deref().unwrap_or("")`.

- [ ] **Step 6: Run tests**

Run: `cargo test test_search_symbols -- --test-threads=1`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add src/protocol/tools.rs
git commit -m "feat: search_symbols browse mode without query (U2)

query is now optional. When omitted, at least kind or path_prefix
is required. Browse mode defaults to limit=20, sorted by path+line.
Query mode still defaults to limit=50."
```

### Task 6: U3 — `inspect_match` sibling cap

**Files:**
- Modify: `src/live_index/query.rs` (`capture_inspect_match_view`, L1822-1895)
- Modify: `src/protocol/tools.rs` (`InspectMatchInput` — add `sibling_limit`)
- Test: `src/live_index/query.rs` or `src/protocol/tools.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn test_inspect_match_caps_siblings_at_10_by_default() {
    // Create index with a file containing 70+ sibling symbols
    // Call capture_inspect_match_view with default sibling_limit
    // Assert siblings.len() <= 10
    // Assert output contains "... and N more siblings"
}

#[test]
fn test_inspect_match_custom_sibling_limit() {
    // sibling_limit=5 → 5 siblings shown
    // sibling_limit=0 → no siblings section
}
```

- [ ] **Step 2: Add `sibling_limit` param to `InspectMatchInput`**

```rust
/// Maximum number of sibling symbols to show (default 10, 0 = none).
#[serde(default, deserialize_with = "lenient_u32")]
pub sibling_limit: Option<u32>,
```

- [ ] **Step 3: Cap siblings in `capture_inspect_match_view`**

In `src/live_index/query.rs`, find where siblings are collected (around L1774). After collecting, truncate:

```rust
let sibling_limit = sibling_limit.unwrap_or(10) as usize;
let total_siblings = siblings.len();
if sibling_limit == 0 {
    siblings.clear();
} else if siblings.len() > sibling_limit {
    siblings.truncate(sibling_limit);
}
// Pass total_siblings to the view for the overflow hint
```

In the formatter, add overflow hint:

```rust
if total_siblings > siblings.len() {
    output.push_str(&format!("  ... and {} more siblings\n", total_siblings - siblings.len()));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test test_inspect_match -- --test-threads=1`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add src/live_index/query.rs src/protocol/tools.rs
git commit -m "feat: inspect_match caps siblings at 10 by default (U3)

Add sibling_limit param (default 10). Overflow shows
'... and N more siblings' hint. sibling_limit=0 hides siblings."
```

### Task 7: U4 — `analyze_file_impact` better status

**Files:**
- Modify: `src/protocol/tools.rs` (`analyze_file_impact` handler, L1496-1556)
- Test: `src/protocol/tools.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn test_analyze_file_impact_unchanged_shows_status() {
    // Index a file, then call analyze_file_impact without modifying
    // Assert output contains "indexed and unchanged"
    // Assert output contains symbol count
    // Assert output contains "what_changed" suggestion
}

#[test]
fn test_analyze_file_impact_deleted_file() {
    // Index a file, delete from disk, call analyze_file_impact
    // Assert output contains "not found on disk"
}
```

- [ ] **Step 2: Implement status taxonomy**

In the `analyze_file_impact` handler, after checking the file:

```rust
// When file content matches indexed version:
format!(
    "Status: indexed and unchanged\nSymbols: {}\nLast indexed: {}\nTip: Use what_changed to see recent modifications.",
    file.symbols.len(),
    format_system_time(file.indexed_at),
)

// When file not found on disk:
format!(
    "Status: not found on disk — removed from index\nPreviously had {} symbols.",
    file.symbols.len(),
)

// When changed on disk:
// (existing re-index behavior, enhanced with status line)
format!("Status: changed on disk since last index\n{}", existing_diff_output)
```

- [ ] **Step 3: Run tests, then commit**

Run: `cargo test test_analyze_file_impact -- --test-threads=1`

```bash
git add src/protocol/tools.rs
git commit -m "feat: analyze_file_impact shows clear status taxonomy (U4)

Three mutually exclusive states: 'indexed and unchanged',
'changed on disk since last index', 'not found on disk'.
Unchanged shows symbol count + suggestion to use what_changed."
```

### Task 8: U6 — Richer `verbosity=signature`

**Files:**
- Modify: `src/protocol/format.rs` (signature rendering functions)
- Test: `src/protocol/format.rs`

- [ ] **Step 1: Find signature rendering**

Search for the signature formatting logic. Use Tokenizor:
```
search_text(query="verbosity.*signature", glob="src/protocol/format.rs")
```

- [ ] **Step 2: Write failing test**

```rust
#[test]
fn test_signature_verbosity_includes_visibility_and_return_type() {
    // Create a SymbolRecord with full signature info
    // Render with verbosity=signature
    // Assert output contains "pub", return type, generic params
    // Assert output is one line
}
```

- [ ] **Step 3: Enhance signature rendering**

Add visibility, return type, and generic parameters to the signature mode output. The exact fields depend on what `SymbolRecord` stores — likely need to pull from the indexed source content's first line.

- [ ] **Step 4: Run tests, then commit**

```bash
git add src/protocol/format.rs
git commit -m "feat: richer verbosity=signature includes visibility and return type (U6)

Signature mode now shows pub/pub(crate) visibility, generic params,
and return type. Kept to one line per symbol."
```

### Task 9: U8 — Health partial parse file list

**Files:**
- Modify: `src/live_index/query.rs` (`HealthStats`, L597-612, `health_stats` function)
- Modify: `src/protocol/format.rs` (`health_report_from_stats`, L858-895)
- Test: `src/protocol/format.rs`

- [ ] **Step 1: Add `partial_parse_files` to `HealthStats`**

```rust
pub struct HealthStats {
    // ... existing fields ...
    /// Repo-relative paths of partially parsed files, sorted alphabetically, deduplicated.
    pub partial_parse_files: Vec<String>,
}
```

- [ ] **Step 2: Populate in `health_stats` function**

In the `health_stats` function in `query.rs`, collect partial-parse file paths:

```rust
let mut partial_parse_files: Vec<String> = guard
    .files
    .iter()
    .filter(|(_, f)| f.parse_status == ParseStatus::PartialParse)
    .map(|(path, _)| path.clone())
    .collect();
partial_parse_files.sort();
partial_parse_files.dedup();
```

- [ ] **Step 3: Render in health report**

In `health_report_from_stats` (format.rs), after the existing stats:

```rust
if !stats.partial_parse_files.is_empty() {
    output.push_str(&format!("\nPartial parse files ({}):\n", stats.partial_parse_files.len()));
    for (i, path) in stats.partial_parse_files.iter().take(10).enumerate() {
        output.push_str(&format!("  {}. {}\n", i + 1, path));
    }
    if stats.partial_parse_files.len() > 10 {
        output.push_str(&format!(
            "  ... and {} more partial files\n",
            stats.partial_parse_files.len() - 10
        ));
    }
}
```

- [ ] **Step 4: Write tests**

```rust
#[test]
fn test_health_report_lists_partial_parse_files() {
    let stats = HealthStats {
        partial_parse_files: vec!["a.rs".into(), "b.rs".into(), "c.rs".into()],
        // ... other fields
    };
    let report = health_report_from_stats("Ready", &stats);
    assert!(report.contains("Partial parse files (3)"));
    assert!(report.contains("a.rs"));
}

#[test]
fn test_health_report_caps_partial_list_at_10() {
    let files: Vec<String> = (0..50).map(|i| format!("file_{:03}.rs", i)).collect();
    let stats = HealthStats {
        partial_parse_files: files,
        // ...
    };
    let report = health_report_from_stats("Ready", &stats);
    assert!(report.contains("... and 40 more partial files"));
}
```

- [ ] **Step 5: Run tests, then commit**

Run: `cargo test test_health_report -- --test-threads=1`

```bash
git add src/live_index/query.rs src/protocol/format.rs
git commit -m "feat: health shows partial parse file paths (U8)

HealthStats now includes sorted, deduplicated list of partial-parse
file paths. Health report shows first 10 with overflow hint."
```

### Task 10: U9 — Tool-use counters

**Files:**
- Modify: `src/sidecar/mod.rs` (`TokenStats`)
- Modify: `src/protocol/mod.rs` (tool dispatch — increment counters)
- Modify: `src/protocol/format.rs` (health report rendering)
- Test: `src/sidecar/mod.rs`

- [ ] **Step 1: Add per-tool counter to `TokenStats`**

**Note:** `TokenStats` currently uses `Atomic*` fields. Adding a `Mutex<HashMap>` will break any `Clone`/`PartialEq` derives. Check existing derives and remove or adjust as needed. Initialize in `TokenStats::new()` with `Mutex::new(HashMap::new())`.

```rust
pub struct TokenStats {
    // ... existing Atomic fields ...
    /// Per-tool invocation counts since daemon start.
    pub tool_calls: std::sync::Mutex<std::collections::HashMap<String, usize>>,
}
```

- [ ] **Step 2: Add increment method**

```rust
impl TokenStats {
    pub fn record_tool_call(&self, tool_name: &str) {
        if let Ok(mut map) = self.tool_calls.lock() {
            *map.entry(tool_name.to_string()).or_insert(0) += 1;
        }
    }

    pub fn tool_call_counts(&self) -> Vec<(String, usize)> {
        let map = self.tool_calls.lock().unwrap_or_else(|e| e.into_inner());
        let mut counts: Vec<_> = map.iter().map(|(k, &v)| (k.clone(), v)).collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1));  // Sort by count descending
        counts
    }
}
```

- [ ] **Step 3: Increment in tool dispatch**

In `src/protocol/mod.rs` or the tool dispatch code, add at the start of each tool handler:

```rust
self.token_stats.record_tool_call("tool_name_here");
```

Or add a single call in the daemon's `execute_tool_call` dispatch.

- [ ] **Step 4: Render in health report**

In `health_report_from_stats`, add tool usage section:

```rust
let counts = stats.tool_call_counts();
if !counts.is_empty() {
    output.push_str("\nTool calls (since start):\n");
    for (name, count) in counts.iter().take(15) {
        output.push_str(&format!("  {}: {}\n", name, count));
    }
}
```

- [ ] **Step 5: Write test**

```rust
#[test]
fn test_token_stats_records_tool_calls() {
    let stats = TokenStats::new();
    stats.record_tool_call("search_text");
    stats.record_tool_call("search_text");
    stats.record_tool_call("get_file_context");
    let counts = stats.tool_call_counts();
    assert_eq!(counts[0], ("search_text".to_string(), 2));
    assert_eq!(counts[1], ("get_file_context".to_string(), 1));
}
```

- [ ] **Step 6: Run tests, then commit**

```bash
git add src/sidecar/mod.rs src/protocol/mod.rs src/protocol/format.rs
git commit -m "feat: per-tool call counters in health output (U9)

TokenStats tracks invocation count per tool name since daemon start.
Health report shows top tools by call count."
```

---

## Chunk 4: Wave 3 (B2, B3), Wave 4 (U1), Wave 5 (U10)

**Parallelization note:** U1 (Wave 4) depends only on U7 (Wave 1), NOT on B2/B3 (Wave 3). Task 13 (U1) can run in parallel with Tasks 11-12 (B2, B3). Only Task 14 (U10) must wait for B2/B3 to complete.

### Task 11: B2 — `around_symbol` returns full symbol span

**Files:**
- Modify: `src/protocol/tools.rs` (`file_content_options_from_input`, L868-1021)
- Modify: `src/live_index/search.rs` (`FileContentOptions::for_explicit_path_read_around_symbol`)
- Modify: rendering code that handles `around_symbol` in `get_file_content`
- Test: `src/protocol/tools.rs`

**Implement B2 before B3** (they touch the same function).

- [ ] **Step 1: Write failing test — full symbol body returned**

```rust
#[test]
fn test_get_file_content_around_symbol_returns_full_body() {
    // Create index with a multi-line function (20+ lines)
    // Call get_file_content with around_symbol="function_name"
    // Assert ALL lines of the function are in the output (not just 3-7)
}

#[test]
fn test_get_file_content_around_symbol_with_max_lines_truncates() {
    // Same setup, add max_lines=5
    // Assert output is 5 lines + truncation hint
}

#[test]
fn test_get_file_content_around_symbol_not_found_errors() {
    // Call with around_symbol="nonexistent"
    // Assert error: "Symbol 'nonexistent' not found in file"
}

#[test]
fn test_get_file_content_around_symbol_includes_doc_comments() {
    // Function with doc comments that are in the indexed range
    // Assert doc comments appear in output
}

#[test]
fn test_get_file_content_around_symbol_context_lines_extends_range() {
    // Create index with a function at lines 10-15
    // Call with around_symbol="fn_name", context_lines=5
    // Assert output starts at line 5 and ends at line 20
}
```

- [ ] **Step 2: Modify `around_symbol` handling**

The core change is in how `around_symbol` resolves to line ranges. Currently it uses a context window. Change to:

1. Look up the symbol in the index by name (and optional `symbol_line`)
2. Use `sym.line_range` as the output range
3. If `context_lines` is set, extend the range by that many lines before/after
4. If `max_lines` is set, truncate with hint (remove the current rejection of `max_lines` with `around_symbol`)

In `file_content_options_from_input`, the `around_symbol` branch currently rejects `max_lines`. Remove that rejection:

```rust
// REMOVE max_lines from the rejection list:
if input.start_line.is_some()
    || input.end_line.is_some()
    || input.around_line.is_some()
    || input.around_match.is_some()
    || input.chunk_index.is_some()
    // REMOVED: || input.max_lines.is_some()
{
```

Pass `max_lines` through to the `FileContentOptions`:

```rust
search::FileContentOptions::for_explicit_path_read_around_symbol(
    input.path.clone(),
    around_symbol,
    input.symbol_line,
    input.context_lines,
    input.max_lines,  // NEW
)
```

- [ ] **Step 3: Update `ContentContext` to use indexed symbol range**

In the rendering code (likely in `src/live_index/query.rs` or wherever `around_symbol` is resolved to line ranges), change from using a fixed context window to looking up the symbol's `line_range` from the index.

The symbol lookup should use the same `resolve_symbol_selector` function used by edit tools.

- [ ] **Step 4: Run tests**

Run: `cargo test test_get_file_content_around_symbol -- --test-threads=1`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add src/protocol/tools.rs src/live_index/search.rs
git commit -m "fix: around_symbol returns full indexed symbol span (B2)

around_symbol now uses the symbol's indexed line range instead of
a fixed context window. max_lines can optionally truncate with a
hint. Doc comments included when in the indexed range. Symbol not
found returns a clear error."
```

### Task 12: B3 — `show_line_numbers` unrestricted

**Files:**
- Modify: `src/protocol/tools.rs` (`file_content_options_from_input`, L868-1021)
- Test: `src/protocol/tools.rs`

**Must be done after B2** (same function).

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn test_get_file_content_show_line_numbers_with_around_symbol() {
    // Call with around_symbol="fn_name", show_line_numbers=true
    // Assert: no error, output contains line numbers
}

#[test]
fn test_get_file_content_show_line_numbers_with_around_match() {
    // Call with around_match="pattern", show_line_numbers=true
    // Assert: no error, output contains line numbers
}
```

- [ ] **Step 2: Remove `show_line_numbers` rejection**

In `file_content_options_from_input`, remove the two blocks that reject `ordinary_read_formatting_requested` with `around_symbol` and `around_match`:

```rust
// DELETE these blocks (around L901 and L964):
// if ordinary_read_formatting_requested {
//     return Err("Invalid get_file_content request: `show_line_numbers` and `header` ...")
// }
```

Then pass `show_line_numbers` through to `ContentContext` for all modes.

- [ ] **Step 3: Verify line numbers are correct**

```rust
#[test]
fn test_show_line_numbers_correct_with_around_symbol() {
    // around_symbol on a function at line 15
    // show_line_numbers=true
    // Assert first line starts with "15"
}
```

- [ ] **Step 4: Run tests, then commit**

Run: `cargo test test_get_file_content_show_line_numbers -- --test-threads=1`

```bash
git add src/protocol/tools.rs
git commit -m "fix: show_line_numbers works with around_symbol and around_match (B3)

Removed validation rejection. show_line_numbers is now orthogonal
to all selection modes. Line numbers reflect actual file positions."
```

### Task 13: U1 — Explore filters noise by default

**Files:**
- Modify: `src/protocol/tools.rs` (`explore` handler, `ExploreInput`)
- Modify: `src/protocol/explore.rs` (if filtering happens in concept matching)
- Test: `src/protocol/tools.rs`

**Depends on Wave 1 (U7).**

- [ ] **Step 1: Add `include_noise` param to `ExploreInput`**

```rust
/// When true, include vendor/generated/gitignored files in results. Default: false.
#[serde(default, deserialize_with = "lenient_bool")]
pub include_noise: Option<bool>,
```

- [ ] **Step 2: Write failing test**

```rust
#[test]
fn test_explore_filters_vendor_files_by_default() {
    // Create index with vendor/foo.js (classified as Vendor noise) and src/main.rs
    // Call explore(query="functions")
    // Assert vendor/foo.js not in results
}

#[test]
fn test_explore_include_noise_brings_back_vendor() {
    // Same setup, include_noise=true
    // Assert vendor/foo.js IS in results
}

#[test]
fn test_explore_shows_hidden_count_hint() {
    // Explore with noise files present
    // Assert output contains "N results from vendor/generated files hidden"
}
```

- [ ] **Step 3: Apply noise filtering in explore handler**

In the `explore` handler, after collecting symbol and text results, filter out noise-classified files when `include_noise` is false (default):

```rust
let include_noise = params.0.include_noise.unwrap_or(false);
if !include_noise {
    let hidden_count = results.iter().filter(|r| is_noise_file(r.path)).count();
    results.retain(|r| !is_noise_file(r.path));
    if hidden_count > 0 {
        output.push_str(&format!(
            "\n{} results from vendor/generated files hidden. Use include_noise=true to include.\n",
            hidden_count
        ));
    }
}
```

- [ ] **Step 4: Run tests, then commit**

```bash
git add src/protocol/tools.rs src/protocol/explore.rs
git commit -m "feat: explore filters noise by default (U1)

Vendor, generated, and gitignored files are hidden from explore
results by default. include_noise=true overrides. Output shows
count of hidden results."
```

### Task 14: U10 — `get_file_content` mode enum

**Files:**
- Modify: `src/protocol/tools.rs` (`GetFileContentInput`, `file_content_options_from_input`)
- Test: `src/protocol/tools.rs`

**Must be done last** in the get_file_content series (after B2, B3).

- [ ] **Step 1: Add `mode` field to `GetFileContentInput`**

```rust
pub struct GetFileContentInput {
    /// Selection mode: "lines", "symbol", "match", "chunk". Optional — inferred from flags when omitted.
    pub mode: Option<String>,
    // ... existing fields ...
}
```

- [ ] **Step 2: Write failing tests**

```rust
#[test]
fn test_get_file_content_mode_symbol_works() {
    // mode="symbol", around_symbol="fn_name"
    // Assert: works correctly
}

#[test]
fn test_get_file_content_mode_symbol_without_around_symbol_errors() {
    // mode="symbol" but no around_symbol
    // Assert error: "mode=symbol requires around_symbol"
}

#[test]
fn test_get_file_content_mode_lines_with_around_symbol_errors() {
    // mode="lines", around_symbol="foo"
    // Assert error contains "mode=lines conflicts with around_symbol. Use mode=symbol."
}

#[test]
fn test_get_file_content_no_mode_backward_compatible() {
    // No mode + legacy flags
    // Assert: current behavior unchanged
}

#[test]
fn test_get_file_content_mode_search_not_implemented() {
    // mode="search"
    // Assert error: "mode 'search' is not yet implemented"
}

#[test]
fn test_get_file_content_mode_symbol_with_same_mode_flag_overrides() {
    // mode="symbol", around_symbol="foo", context_lines=10
    // Assert: works (context_lines is same-mode override)
}
```

- [ ] **Step 3: Implement mode dispatch in `file_content_options_from_input`**

Add mode validation at the top of the function:

```rust
fn file_content_options_from_input(
    input: &GetFileContentInput,
) -> Result<search::FileContentOptions, String> {
    // Handle mode if present
    if let Some(mode) = &input.mode {
        return match mode.as_str() {
            "lines" => validate_lines_mode(input),
            "symbol" => validate_symbol_mode(input),
            "match" => validate_match_mode(input),
            "chunk" => validate_chunk_mode(input),
            "search" => Err("mode 'search' is not yet implemented".to_string()),
            other => Err(format!("Unknown mode '{other}'. Valid modes: lines, symbol, match, chunk.")),
        };
    }

    // No mode — infer from flags (existing behavior)
    // ... existing code unchanged ...
}

fn validate_symbol_mode(input: &GetFileContentInput) -> Result<search::FileContentOptions, String> {
    // Required: around_symbol
    let around_symbol = input.around_symbol.as_deref()
        .ok_or("mode=symbol requires around_symbol")?;

    // Reject cross-mode flags
    if input.start_line.is_some() || input.end_line.is_some() || input.around_line.is_some()
        || input.around_match.is_some() || input.chunk_index.is_some()
    {
        return Err(format!(
            "mode=symbol conflicts with line/match/chunk flags. Received: {}. Use the appropriate mode instead.",
            describe_received_flags(input),
        ));
    }

    // Same-mode flags allowed: symbol_line, context_lines, max_lines, show_line_numbers
    Ok(search::FileContentOptions::for_explicit_path_read_around_symbol(
        input.path.clone(),
        around_symbol.trim(),
        input.symbol_line,
        input.context_lines,
        input.max_lines,
    ))
}

fn validate_lines_mode(input: &GetFileContentInput) -> Result<search::FileContentOptions, String> {
    // Reject cross-mode flags
    if input.around_symbol.is_some() {
        return Err(format!("mode=lines conflicts with around_symbol. Use mode=symbol. Received: {}", describe_received_flags(input)));
    }
    if input.around_match.is_some() {
        return Err(format!("mode=lines conflicts with around_match. Use mode=match. Received: {}", describe_received_flags(input)));
    }
    if input.chunk_index.is_some() {
        return Err(format!("mode=lines conflicts with chunk_index. Use mode=chunk. Received: {}", describe_received_flags(input)));
    }
    // Same-mode flags: start_line, end_line, around_line, context_lines, show_line_numbers, header
    let show_line_numbers = input.show_line_numbers.unwrap_or(false);
    let header = input.header.unwrap_or(false);
    Ok(match input.around_line {
        Some(around_line) => search::FileContentOptions::for_explicit_path_read_around_line(
            input.path.clone(), around_line, input.context_lines,
        ),
        None => search::FileContentOptions::for_explicit_path_read_with_format(
            input.path.clone(), input.start_line, input.end_line, show_line_numbers, header,
        ),
    })
}

fn validate_match_mode(input: &GetFileContentInput) -> Result<search::FileContentOptions, String> {
    let around_match = input.around_match.as_deref()
        .ok_or("mode=match requires around_match")?;
    if input.start_line.is_some() || input.end_line.is_some() || input.around_line.is_some() {
        return Err(format!("mode=match conflicts with line flags. Use mode=lines. Received: {}", describe_received_flags(input)));
    }
    if input.around_symbol.is_some() {
        return Err(format!("mode=match conflicts with around_symbol. Use mode=symbol. Received: {}", describe_received_flags(input)));
    }
    if input.chunk_index.is_some() {
        return Err(format!("mode=match conflicts with chunk_index. Use mode=chunk. Received: {}", describe_received_flags(input)));
    }
    Ok(search::FileContentOptions::for_explicit_path_read_around_match(
        input.path.clone(), around_match.trim(), input.context_lines,
    ))
}

fn validate_chunk_mode(input: &GetFileContentInput) -> Result<search::FileContentOptions, String> {
    let chunk_index = input.chunk_index.ok_or("mode=chunk requires chunk_index")?;
    let max_lines = input.max_lines.ok_or("mode=chunk requires max_lines")?;
    if chunk_index == 0 { return Err("chunk_index must be 1 or greater".to_string()); }
    if max_lines == 0 { return Err("max_lines must be 1 or greater".to_string()); }
    if input.around_symbol.is_some() || input.around_match.is_some() || input.around_line.is_some() {
        return Err(format!("mode=chunk conflicts with symbol/match/line flags. Received: {}", describe_received_flags(input)));
    }
    Ok(search::FileContentOptions::for_explicit_path_read_chunk(
        input.path.clone(), chunk_index, max_lines,
    ))
}
```

- [ ] **Step 4: Add `describe_received_flags` helper**

```rust
fn describe_received_flags(input: &GetFileContentInput) -> String {
    let mut flags = Vec::new();
    if input.start_line.is_some() { flags.push("start_line"); }
    if input.end_line.is_some() { flags.push("end_line"); }
    if input.around_line.is_some() { flags.push("around_line"); }
    if input.around_symbol.is_some() { flags.push("around_symbol"); }
    if input.around_match.is_some() { flags.push("around_match"); }
    if input.chunk_index.is_some() { flags.push("chunk_index"); }
    if input.max_lines.is_some() { flags.push("max_lines"); }
    flags.join(", ")
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test test_get_file_content_mode -- --test-threads=1`
Expected: All PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: All PASS — no regressions

- [ ] **Step 7: Commit**

```bash
git add src/protocol/tools.rs
git commit -m "feat: get_file_content mode enum for clearer API (U10)

Add optional mode param: lines, symbol, match, chunk. Mode sets
defaults; same-mode flags override; cross-mode flags error with
guidance showing what was received and which mode to use.
mode=search reserved (not yet implemented). No mode = infer from
legacy flags (backward compatible)."
```

---

## Final Steps

- [ ] **Run full test suite**

```bash
cargo test --all-targets -- --test-threads=1
cargo fmt -- --check
```

- [ ] **Update PLAN.md with Sprint 14 trust bugs**

Add Sprint 14 section to `PLAN.md`:

```markdown
## Sprint 14: Trust & Reliability (P0)

1. **batch_rename path-qualified calls** — textual rename misses `Module::symbol` patterns
2. **search_text disk divergence** — FTS index can diverge from disk after partial rename
```

- [ ] **Final commit**

```bash
git add PLAN.md
git commit -m "docs: add Sprint 14 trust bugs to PLAN.md"
```
