# MCP Output Quality Improvements — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve MCP tool output quality across 5 items in 2 PRs — better diff explanations, OR-term annotations, mermaid symbol edges, edit dry_run, and false-positive filtering in "Used by".

**Architecture:** PR 1 touches only the formatting layer (`format.rs`). PR 2 touches input structs (`edit.rs`), query logic (`query.rs`), view structs (`query.rs`), and handler early-returns (`tools.rs`). All changes are additive — no existing behavior changes.

**Tech Stack:** Rust, serde, tree-sitter (index), git2 (temporal data)

**Spec:** `docs/superpowers/specs/2026-03-20-mcp-output-quality-design.md` (v3)

**Coordination:** Another AI agent is working on P0-P1/P3-P10 from a separate bug report. Do NOT modify: `json.rs`, `mod.rs` (protocol), token stats code, `extract_declaration_name`, health stats, explore input struct. The dry_run work (Item 5) is exclusively ours.

---

## PR 1: Output Polish

### Task 1: diff_symbols compact omission note

**Files:**
- Modify: `src/protocol/format.rs` — `diff_symbols_result_view` (L6267)
- Modify: `src/protocol/tools.rs` — add test

- [ ] **Step 1: Write the failing test**

In `src/protocol/tools.rs`, inside `mod tests`, add:

```rust
#[test]
fn test_diff_symbols_compact_shows_omission_note() {
    // File A: has symbol changes. File B: only non-symbol changes.
    // init_git_repo() takes no args and returns TempDir.
    let dir = init_git_repo();

    // Create base commit with two files
    let a_path = dir.path().join("a.rs");
    let b_path = dir.path().join("b.rs");
    std::fs::write(&a_path, "fn old_func() {}\n").unwrap();
    std::fs::write(&b_path, "// comment\n").unwrap();
    run_git(dir.path(), &["add", "."]);
    run_git(dir.path(), &["commit", "-m", "base"]);

    // Create target commit: add symbol to A, change comment in B
    std::fs::write(&a_path, "fn old_func() {}\nfn new_func() {}\n").unwrap();
    std::fs::write(&b_path, "// changed comment\n").unwrap();
    run_git(dir.path(), &["add", "."]);
    run_git(dir.path(), &["commit", "-m", "changes"]);

    // Open GitRepo from the temp dir (init_git_repo returns TempDir, not GitRepo)
    let repo = crate::git::GitRepo::open(dir.path()).expect("open git repo");

    let result = super::format::diff_symbols_result_view(
        "HEAD~1",
        "HEAD",
        &["a.rs", "b.rs"],
        &repo,
        true, // compact
    );

    assert!(
        result.contains("1 file(s) with only non-symbol changes omitted"),
        "compact mode should note omitted files: {result}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_diff_symbols_compact_shows_omission_note -- --test-threads=1`
Expected: FAIL — the omission note is not emitted yet.

- [ ] **Step 3: Implement the change**

In `src/protocol/format.rs`, in `diff_symbols_result_view`, add a `files_with_changes` counter and the omission note. The change goes inside the function body:

1. Add `let mut files_with_changes = 0usize;` before the `for file_path in changed_files` loop.
2. After the `continue;` line inside `if file_added.is_empty() && file_removed.is_empty() && file_modified.is_empty()`, add nothing (the continue skips the counter).
3. Right after the totals accumulation lines (`total_added += ...`, `total_removed += ...`, `total_modified += ...`), add `files_with_changes += 1;`.
4. After the existing `if files_with_symbol_changes == 0` block (at the end of the function, before `lines.join`), add:

```rust
if compact && files_with_changes > 0 && changed_files.len() > files_with_changes {
    let omitted = changed_files.len() - files_with_changes;
    lines.push(format!(
        "({omitted} file(s) with only non-symbol changes omitted)"
    ));
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_diff_symbols_compact_shows_omission_note -- --test-threads=1`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: All existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/protocol/format.rs src/protocol/tools.rs
git commit -m "fix(diff_symbols): show omission note in compact mode when files have no symbol changes"
```

---

### Task 2: search_text OR term annotations

**Files:**
- Modify: `src/protocol/format.rs` — `search_text_result_view` (L302)
- Modify: `src/protocol/tools.rs` — `search_text` handler (L2178) + add test

- [ ] **Step 1: Write the failing test**

In `src/protocol/tools.rs`, inside `mod tests`, add:

```rust
#[tokio::test]
async fn test_search_text_terms_annotates_matched_term() {
    let sym_a = make_symbol("fn_alpha", SymbolKind::Function, 0, 0);
    let sym_b = make_symbol("fn_beta", SymbolKind::Function, 1, 1);
    let file = make_file(
        "src/lib.rs",
        b"fn fn_alpha() { alpha_value }\nfn fn_beta() { beta_value }\n",
        vec![sym_a, sym_b],
    );
    let server = make_server(make_live_index_ready(vec![file]));

    let result = server
        .search_text(Parameters(super::SearchTextInput {
            query: None,
            terms: Some(vec!["alpha_value".to_string(), "beta_value".to_string()]),
            regex: None,
            path_prefix: None,
            language: None,
            limit: None,
            max_per_file: None,
            include_generated: None,
            include_tests: None,
            glob: None,
            exclude_glob: None,
            context: None,
            case_sensitive: None,
            whole_word: None,
            group_by: None,
            follow_refs: None,
            follow_refs_limit: None,
            ranked: None,
        }))
        .await;

    assert!(
        result.contains("[term: alpha_value]"),
        "should annotate alpha term: {result}"
    );
    assert!(
        result.contains("[term: beta_value]"),
        "should annotate beta term: {result}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_search_text_terms_annotates_matched_term -- --test-threads=1`
Expected: FAIL — no term annotations exist yet.

- [ ] **Step 3: Change the formatter signature**

In `src/protocol/format.rs`, change the signature of `search_text_result_view` from:

```rust
pub fn search_text_result_view(
    result: Result<search::TextSearchResult, search::TextSearchError>,
    group_by: Option<&str>,
) -> String {
```

to:

```rust
pub fn search_text_result_view(
    result: Result<search::TextSearchResult, search::TextSearchError>,
    group_by: Option<&str>,
    terms: Option<&[String]>,
) -> String {
```

- [ ] **Step 4: Add term annotation logic in the formatter**

Inside `search_text_result_view`, add a helper closure at the top of the function body (after the error match block, before the empty-check):

```rust
let annotate_term = |line: &str| -> String {
    match &terms {
        Some(ts) if ts.len() > 1 => {
            let lower = line.to_lowercase();
            for term in *ts {
                if lower.contains(&term.to_lowercase()) {
                    return format!("  [term: {term}]");
                }
            }
            String::new()
        }
        _ => String::new(),
    }
};
```

Then, in every place that renders a match line (the `> {line_number}: {line}` patterns), append `{annotate_term(&line_match.line)}`. There are 3 render branches (default/file, usage, and symbol). Only the non-symbol branches render individual lines — append the annotation there.

For the default (`_ =>`) branch and the `usage` branch, change:
```rust
lines.push(format!(
    "    > {}: {}",
    line_match.line_number, line_match.line
));
```
to:
```rust
lines.push(format!(
    "    > {}: {}{}",
    line_match.line_number, line_match.line, annotate_term(&line_match.line)
));
```

For top-level matches (no enclosing symbol), change:
```rust
lines.push(format!("  {}: {}", line_match.line_number, line_match.line));
```
to:
```rust
lines.push(format!("  {}: {}{}", line_match.line_number, line_match.line, annotate_term(&line_match.line)));
```

Do NOT annotate in the `symbol` group_by branch (it shows counts, not individual lines) or in the `context` branch (rendered_lines).

- [ ] **Step 5: Update ALL call sites (there are 4, not 3)**

1. `src/protocol/tools.rs` L2178 (main call):
```rust
// Before:
let output = format::search_text_result_view(result, params.0.group_by.as_deref());
// After:
let output = format::search_text_result_view(result, params.0.group_by.as_deref(), params.0.terms.as_deref());
```

2. `src/protocol/tools.rs` ~L2163 (auto-correct retry path):
```rust
// Before:
let mut output = format::search_text_result_view(
    retry_result,
    params.0.group_by.as_deref(),
);
// After:
let mut output = format::search_text_result_view(
    retry_result,
    params.0.group_by.as_deref(),
    params.0.terms.as_deref(),
);
```

3. `src/protocol/format.rs` L280 (production wrapper — **critical, will cause compile error if missed**):
```rust
// Before:
search_text_result_view(result, None)
// After:
search_text_result_view(result, None, None)
```

4. `src/protocol/format.rs` L3530 and L3613 (two test calls in format.rs):
```rust
// Both need None appended:
search_text_result_view(..., None, None)
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test test_search_text_terms_annotates_matched_term -- --test-threads=1`
Expected: PASS

- [ ] **Step 7: Run full test suite**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: All existing tests pass (they'll pass `None` or the correct terms).

- [ ] **Step 8: Commit**

```bash
git add src/protocol/format.rs src/protocol/tools.rs
git commit -m "feat(search_text): annotate which term matched in OR-term searches"
```

---

### Task 3: Create PR 1

- [ ] **Step 1: Create feature branch and PR**

```bash
git checkout -b feat/output-polish
git push -u origin feat/output-polish
GITHUB_TOKEN= gh pr create --title "feat: MCP output polish — diff compact note, OR term annotations" --body "$(cat <<'EOF'
## Summary
- diff_symbols compact mode now shows omission note for files with only non-symbol changes
- search_text OR-term searches annotate each match with which term matched

## Test plan
- [ ] `cargo test --all-targets -- --test-threads=1` passes
- [ ] `test_diff_symbols_compact_shows_omission_note` verifies omission note
- [ ] `test_search_text_terms_annotates_matched_term` verifies term annotations

Spec: docs/superpowers/specs/2026-03-20-mcp-output-quality-design.md (Items 1, 3)

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)" --base main
```

---

## PR 2: Structural Improvements

### Task 4: Add `name` field to `DependentLineView`

**Files:**
- Modify: `src/live_index/query.rs` — `DependentLineView` (L770), `capture_find_dependents_view` (L1442)
- Modify: `src/protocol/format.rs` — `find_dependents_mermaid` (L2105), `find_dependents_dot` (L2138)
- Modify: `src/protocol/tools.rs` — add test

- [ ] **Step 1: Write the failing test**

In `src/protocol/tools.rs`, inside `mod tests`, add:

```rust
#[tokio::test]
async fn test_find_dependents_mermaid_includes_symbol_names_in_edges() {
    let target_sym = make_symbol("TargetType", SymbolKind::Struct, 0, 2);
    // make_ref signature: (name, qualified_name, kind, line, enclosing_symbol_index)
    let dep_ref = make_ref("TargetType", None, ReferenceKind::TypeUsage, 0, Some(0));
    let dep_sym = make_symbol("consumer", SymbolKind::Function, 0, 1);
    let target_file = make_file("src/target.rs", b"struct TargetType {}\n", vec![target_sym]);
    let dep_file = make_file_with_refs(
        "src/dep.rs",
        b"fn consumer() { TargetType }\n",
        vec![dep_sym],
        vec![dep_ref],
    );
    let server = make_server(make_live_index_ready(vec![target_file, dep_file]));

    let result = server
        .find_dependents(Parameters(super::FindDependentsInput {
            path: "src/target.rs".to_string(),
            compact: None,
            format: Some("mermaid".to_string()),
            limit: None,
            max_per_file: None,
        }))
        .await;

    assert!(
        result.contains("TargetType"),
        "mermaid edge should include symbol name: {result}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_find_dependents_mermaid_includes_symbol_names_in_edges -- --test-threads=1`
Expected: FAIL — mermaid currently only shows ref counts.

- [ ] **Step 3: Add `name` field to `DependentLineView`**

In `src/live_index/query.rs`, change `DependentLineView`:

```rust
pub struct DependentLineView {
    pub line_number: u32,
    pub line_content: String,
    pub kind: String,
    pub name: String,
}
```

- [ ] **Step 4: Populate `name` in `capture_find_dependents_view`**

In `src/live_index/query.rs`, in `capture_find_dependents_view`, change the `DependentLineView` construction:

```rust
.push(DependentLineView {
    line_number,
    line_content,
    kind: reference.kind.to_string(),
    name: reference.name.clone(),
});
```

- [ ] **Step 5: Update mermaid formatter**

In `src/protocol/format.rs`, replace the edge-rendering loop in `find_dependents_mermaid`:

```rust
for file in view.files.iter().take(limits.max_files) {
    let dep_id = mermaid_node_id(&file.file_path);
    let ref_count = file.lines.len();

    // Collect up to 3 distinct symbol names for the edge label
    let mut names: Vec<&str> = Vec::new();
    for line in &file.lines {
        if !names.contains(&line.name.as_str()) {
            names.push(&line.name);
            if names.len() >= 3 {
                break;
            }
        }
    }
    let remaining = ref_count.saturating_sub(names.len());
    let label = if names.is_empty() {
        format!("{ref_count} refs")
    } else if remaining > 0 {
        format!("{} +{remaining}", names.join(", "))
    } else {
        names.join(", ")
    };
    lines.push(format!(
        "    {dep_id}[\"{}\"] -->|\"{label}\"| {target_id}",
        file.file_path
    ));
}
```

- [ ] **Step 6: Update dot formatter**

In `src/protocol/format.rs`, apply the same pattern to `find_dependents_dot`:

```rust
for file in view.files.iter().take(limits.max_files) {
    let mut names: Vec<&str> = Vec::new();
    for line in &file.lines {
        if !names.contains(&line.name.as_str()) {
            names.push(&line.name);
            if names.len() >= 3 {
                break;
            }
        }
    }
    let remaining = file.lines.len().saturating_sub(names.len());
    let label = if names.is_empty() {
        format!("{} refs", file.lines.len())
    } else if remaining > 0 {
        format!("{} +{remaining}", names.join(", "))
    } else {
        names.join(", ")
    };
    lines.push(format!(
        "    \"{}\" -> \"{}\" [label=\"{}\"];",
        dot_escape(&file.file_path),
        dot_escape(path),
        label
    ));
}
```

- [ ] **Step 7: Fix existing tests that construct `DependentLineView`**

Two tests in `src/protocol/format.rs` construct `DependentLineView` directly and will break:

1. `test_find_dependents_mermaid_shows_true_ref_count_not_capped` (L4955): Add `name` field and update assertion.
```rust
// Change DependentLineView construction (inside the .map closure):
DependentLineView {
    line_number: i,
    line_content: format!("use crate::db; // ref {i}"),
    kind: "import".to_string(),
    name: "db".to_string(),  // <-- add this
}
// Change assertion from:
//   result.contains("5 refs")
// To:
//   result.contains("db")
// Because the edge label now shows the symbol name instead of "5 refs".
```

2. `test_find_dependents_dot_shows_true_ref_count_not_capped` (L4981): Same changes — add `name: "db".to_string()` and update assertion from `"5 refs"` to `"db"`.

Also search for any other `DependentLineView {` constructions across the codebase and add the `name` field.

- [ ] **Step 8: Run tests**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: All tests pass, including the new one.

- [ ] **Step 9: Commit**

```bash
git add src/live_index/query.rs src/protocol/format.rs src/protocol/tools.rs
git commit -m "feat(find_dependents): show symbol names in mermaid and dot edge labels"
```

---

### Task 5: dry_run for single edit tools

**Files:**
- Modify: `src/protocol/edit.rs` — 4 input structs
- Modify: `src/protocol/tools.rs` — 4 handlers + 4 tests

- [ ] **Step 1: Add `dry_run` field to all 4 input structs**

In `src/protocol/edit.rs`, add to each of `ReplaceSymbolBodyInput`, `InsertSymbolInput`, `DeleteSymbolInput`, `EditWithinSymbolInput`:

```rust
/// When true, validate and preview but skip the actual write.
#[serde(default, deserialize_with = "super::tools::lenient_bool")]
pub dry_run: Option<bool>,
```

- [ ] **Step 2: Write failing tests for all 4 tools**

In `src/protocol/tools.rs`, inside `mod tests`, add:

```rust
#[tokio::test]
async fn test_replace_symbol_body_dry_run_skips_write() {
    let original = b"fn foo() { old }\n";
    let (_dir, server, file_path) = setup_edit_test(original);

    let result = server
        .replace_symbol_body(Parameters(edit::ReplaceSymbolBodyInput {
            path: "src/lib.rs".to_string(),
            name: "foo".to_string(),
            kind: None,
            symbol_line: None,
            new_body: "fn foo() { new }".to_string(),
            dry_run: Some(true),
        }))
        .await;

    assert!(result.contains("[DRY RUN]"), "should show dry run: {result}");
    let on_disk = std::fs::read_to_string(&file_path).unwrap();
    assert!(on_disk.contains("old"), "file should be unchanged: {on_disk}");
}

#[tokio::test]
async fn test_insert_symbol_dry_run_skips_write() {
    let original = b"fn anchor() {}\n";
    let (_dir, server, file_path) = setup_edit_test(original);

    let result = server
        .insert_symbol(Parameters(edit::InsertSymbolInput {
            path: "src/lib.rs".to_string(),
            name: "anchor".to_string(),
            kind: None,
            symbol_line: None,
            content: "fn new_fn() {}".to_string(),
            position: Some("after".to_string()),
            dry_run: Some(true),
        }))
        .await;

    assert!(result.contains("[DRY RUN]"), "should show dry run: {result}");
    let on_disk = std::fs::read_to_string(&file_path).unwrap();
    assert!(!on_disk.contains("new_fn"), "file should be unchanged: {on_disk}");
}

#[tokio::test]
async fn test_delete_symbol_dry_run_skips_write() {
    let original = b"fn target() {}\nfn keep() {}\n";
    let (_dir, server, file_path) = setup_edit_test(original);

    let result = server
        .delete_symbol(Parameters(edit::DeleteSymbolInput {
            path: "src/lib.rs".to_string(),
            name: "target".to_string(),
            kind: None,
            symbol_line: None,
            dry_run: Some(true),
        }))
        .await;

    assert!(result.contains("[DRY RUN]"), "should show dry run: {result}");
    let on_disk = std::fs::read_to_string(&file_path).unwrap();
    assert!(on_disk.contains("target"), "file should be unchanged: {on_disk}");
}

#[tokio::test]
async fn test_edit_within_symbol_dry_run_skips_write() {
    let original = b"fn foo() { old_text }\n";
    let (_dir, server, file_path) = setup_edit_test(original);

    let result = server
        .edit_within_symbol(Parameters(edit::EditWithinSymbolInput {
            path: "src/lib.rs".to_string(),
            name: "foo".to_string(),
            kind: None,
            symbol_line: None,
            old_text: "old_text".to_string(),
            new_text: "new_text".to_string(),
            replace_all: false,
            dry_run: Some(true),
        }))
        .await;

    assert!(result.contains("[DRY RUN]"), "should show dry run: {result}");
    let on_disk = std::fs::read_to_string(&file_path).unwrap();
    assert!(on_disk.contains("old_text"), "file should be unchanged: {on_disk}");
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test test_replace_symbol_body_dry_run test_insert_symbol_dry_run test_delete_symbol_dry_run test_edit_within_symbol_dry_run -- --test-threads=1`
Expected: FAIL — compile errors (no `dry_run` field yet) or no `[DRY RUN]` output.

- [ ] **Step 4: Add dry_run early-return to `replace_symbol_body`**

In `src/protocol/tools.rs`, in `replace_symbol_body`, after the `resolve_or_error` call succeeds (after `Err(e) => return e,`), add:

```rust
if params.0.dry_run == Some(true) {
    let old_bytes = (sym.byte_range.1 - sym.byte_range.0) as usize;
    return format!(
        "[DRY RUN] Would replace `{}` in {} (old: {} bytes → new: {} bytes)",
        params.0.name, params.0.path, old_bytes, params.0.new_body.len()
    );
}
```

- [ ] **Step 5: Add dry_run early-return to `insert_symbol`**

In `src/protocol/tools.rs`, in `insert_symbol`, after `resolve_or_error` succeeds, add:

```rust
if params.0.dry_run == Some(true) {
    return format!(
        "[DRY RUN] Would insert {} `{}` in {} ({} bytes of content)",
        position, params.0.name, params.0.path, params.0.content.len()
    );
}
```

- [ ] **Step 6: Add dry_run early-return to `delete_symbol`**

In `src/protocol/tools.rs`, in `delete_symbol`, after `resolve_or_error` succeeds, add:

```rust
if params.0.dry_run == Some(true) {
    let deleted_bytes = (sym.byte_range.1 - sym.byte_range.0) as usize;
    return format!(
        "[DRY RUN] Would delete `{}` in {} ({} bytes)",
        params.0.name, params.0.path, deleted_bytes
    );
}
```

- [ ] **Step 7: Add dry_run early-return to `edit_within_symbol`**

In `src/protocol/tools.rs`, in `edit_within_symbol`, after the replacement count is computed (after the `if count == 0` error check), add:

```rust
if params.0.dry_run == Some(true) {
    return format!(
        "[DRY RUN] Would edit within `{}` in {} ({} replacement(s))",
        params.0.name, params.0.path, count
    );
}
```

- [ ] **Step 8: Fix existing tests that construct these input structs**

Search for `ReplaceSymbolBodyInput {`, `InsertSymbolInput {`, `DeleteSymbolInput {`, `EditWithinSymbolInput {` in tests and add `dry_run: None,` (or `dry_run: Some(false),`) to each construction.

- [ ] **Step 9: Run all tests**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: All tests pass.

- [ ] **Step 10: Commit**

```bash
git add src/protocol/edit.rs src/protocol/tools.rs
git commit -m "feat(edit): add dry_run to replace_symbol_body, insert_symbol, delete_symbol, edit_within_symbol"
```

---

### Task 6: Filter false positives in find_dependents_for_file

**Files:**
- Modify: `src/live_index/query.rs` — `find_dependents_for_file` (L2407)
- Modify: `src/protocol/tools.rs` — add test

- [ ] **Step 1: Write the failing test**

In `src/protocol/tools.rs`, inside `mod tests`, add:

```rust
#[tokio::test]
async fn test_find_dependents_excludes_non_pub_name_collision() {
    // target.rs has a non-pub symbol "run"
    let target_sym = make_symbol("run", SymbolKind::Function, 0, 1);
    let target_file = make_file(
        "src/target.rs",
        b"fn run() { internal }\n",
        vec![target_sym],
    );

    // other.rs imports target (so matching_imports is non-empty, triggering the
    // symbol_refs path) but the call to "run" is actually to its own local fn.
    // Without the visibility filter, this would be a false positive.
    let other_sym = make_symbol("main", SymbolKind::Function, 2, 4);
    // make_ref signature: (name, qualified_name, kind, line, enclosing_symbol_index)
    let other_import = make_ref("target", Some("crate::target"), ReferenceKind::Import, 0, None);
    let other_ref = make_ref("run", None, ReferenceKind::Call, 3, Some(0));
    let other_file = make_file_with_refs(
        "src/other.rs",
        b"use crate::target;\nfn run() {}\nfn main() {\n    run();\n}\n",
        vec![other_sym],
        vec![other_import, other_ref],
    );

    let server = make_server(make_live_index_ready(vec![target_file, other_file]));

    let result = server
        .find_dependents(Parameters(super::FindDependentsInput {
            path: "src/target.rs".to_string(),
            compact: None,
            format: None,
            limit: None,
            max_per_file: None,
        }))
        .await;

    assert!(
        !result.contains("src/other.rs"),
        "non-pub symbol name collision should not create false dependent: {result}"
    );
}

#[tokio::test]
async fn test_find_dependents_includes_pub_symbol_references() {
    // target.rs has a pub symbol "PublicApi"
    let target_sym = make_symbol("PublicApi", SymbolKind::Struct, 0, 1);
    let target_file = make_file(
        "src/target.rs",
        b"pub struct PublicApi {}\n",
        vec![target_sym],
    );

    // consumer.rs references PublicApi with a qualified import
    let consumer_sym = make_symbol("use_it", SymbolKind::Function, 1, 3);
    let consumer_import = ReferenceRecord {
        name: "target".to_string(),
        qualified_name: Some("crate::target".to_string()),
        kind: ReferenceKind::Import,
        byte_range: (0, 20),
        line_range: (0, 0),
        enclosing_symbol_index: None,
    };
    // make_ref signature: (name, qualified_name, kind, line, enclosing_symbol_index)
    let consumer_ref = make_ref("PublicApi", None, ReferenceKind::TypeUsage, 2, Some(0));
    let consumer_file = make_file_with_refs(
        "src/consumer.rs",
        b"use crate::target;\nfn use_it() {\n    PublicApi {}\n}\n",
        vec![consumer_sym],
        vec![consumer_import, consumer_ref],
    );

    let server = make_server(make_live_index_ready(vec![target_file, consumer_file]));

    let result = server
        .find_dependents(Parameters(super::FindDependentsInput {
            path: "src/target.rs".to_string(),
            compact: None,
            format: None,
            limit: None,
            max_per_file: None,
        }))
        .await;

    assert!(
        result.contains("src/consumer.rs"),
        "pub symbol with import should be a real dependent: {result}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_find_dependents_excludes_non_pub test_find_dependents_includes_pub -- --test-threads=1`
Expected: The "excludes non-pub" test FAILS (false positive currently appears). The "includes pub" test should PASS already.

- [ ] **Step 3: Implement the visibility heuristic**

In `src/live_index/query.rs`, add a helper function before `find_dependents_for_file`:

```rust
/// Check whether a file exports a public symbol with the given name.
/// Uses a text scan of file content since SymbolRecord has no visibility field.
fn has_pub_symbol(file: &IndexedFile, name: &str) -> bool {
    match file.language {
        LanguageId::Rust => {
            let content = String::from_utf8_lossy(&file.content);
            // Check for pub declarations of the symbol name
            for keyword in &["fn", "struct", "enum", "trait", "type", "const", "static", "mod"] {
                let pattern = format!("pub {keyword} {name}");
                if content.contains(&pattern) {
                    return true;
                }
                // Also check pub(crate) and pub(super)
                let crate_pattern = format!("pub(crate) {keyword} {name}");
                if content.contains(&crate_pattern) {
                    return true;
                }
            }
            false
        }
        LanguageId::JavaScript | LanguageId::TypeScript => {
            let content = String::from_utf8_lossy(&file.content);
            // Check for export declarations
            content.contains(&format!("export {{ {name}"))
                || content.contains(&format!("export {name}"))
                || content.contains(&format!("export default {name}"))
                || content.contains(&format!("export function {name}"))
                || content.contains(&format!("export class {name}"))
                || content.contains(&format!("export const {name}"))
                || content.contains(&format!("export interface {name}"))
                || content.contains(&format!("export type {name}"))
        }
        // Python: all module-level symbols are importable, skip filter
        // Other languages: skip filter to avoid false negatives
        _ => true,
    }
}
```

- [ ] **Step 4: Add filtering in `find_dependents_for_file`**

In `find_dependents_for_file`, in the block that collects `symbol_refs` (the `if !target_symbol_names.is_empty()` section around L2460), wrap the symbol-ref matching with the visibility check.

Change the filter from:
```rust
let symbol_refs: Vec<&ReferenceRecord> = file
    .references
    .iter()
    .filter(|reference| {
        reference.kind != ReferenceKind::Import
            && target_symbol_names.contains(reference.name.as_str())
    })
    .collect();
```

to:
```rust
let symbol_refs: Vec<&ReferenceRecord> = file
    .references
    .iter()
    .filter(|reference| {
        reference.kind != ReferenceKind::Import
            && target_symbol_names.contains(reference.name.as_str())
            && has_pub_symbol(target_file, &reference.name)
    })
    .collect();
```

Apply this same change to BOTH occurrences of this pattern in `find_dependents_for_file` (there's a second one in the re-export chain resolution block around L2540).

- [ ] **Step 5: Run tests**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: All tests pass, including both new tests.

- [ ] **Step 6: Verify the main.rs false positive is fixed**

Run the MCP server and call `get_file_context` on `src/main.rs`. The "Used by" count should drop dramatically from 48 to a much smaller number (only files that genuinely import from main.rs's public API).

- [ ] **Step 7: Commit**

```bash
git add src/live_index/query.rs src/protocol/tools.rs
git commit -m "fix(dependents): filter false positives from non-pub symbol name collisions"
```

---

### Task 7: Create PR 2

- [ ] **Step 1: Create feature branch and PR**

```bash
git checkout -b feat/structural-improvements
git push -u origin feat/structural-improvements
GITHUB_TOKEN= gh pr create --title "feat: MCP structural improvements — mermaid edges, edit dry_run, Used By accuracy" --body "$(cat <<'EOF'
## Summary
- find_dependents mermaid/dot output now shows symbol names in edge labels
- replace_symbol_body, insert_symbol, delete_symbol, edit_within_symbol support dry_run
- find_dependents_for_file filters false positives from non-pub symbol name collisions

## Test plan
- [ ] `cargo test --all-targets -- --test-threads=1` passes
- [ ] `test_find_dependents_mermaid_includes_symbol_names_in_edges` verifies mermaid labels
- [ ] 4 dry_run tests verify preview-without-write for each edit tool
- [ ] `test_find_dependents_excludes_non_pub_name_collision` verifies false positive filtering
- [ ] `test_find_dependents_includes_pub_symbol_references` verifies real dependents are kept

Spec: docs/superpowers/specs/2026-03-20-mcp-output-quality-design.md (Items 4, 5, 6)

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)" --base main
```
