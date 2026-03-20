# Reviewer Feedback Remediation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 9 issues identified by external reviewers testing SymForge on real TypeScript/Angular/NestJS and C#/.NET codebases.

**Architecture:** Each task is a self-contained fix targeting a specific module. No task depends on another. Wave 1 (P0, P1) addresses high-impact issues; Wave 2 (P3-P5) medium; Wave 3 (P6-P9) low. P2 (dry_run for single-edit tools) is handled by a separate agent and excluded from this plan.

**Tech Stack:** Rust, serde_json, tree-sitter, axum (sidecar)

**Cross-agent coordination:** Another agent is modifying `diff_symbols_result_view`, `search_text_result_view`, `find_dependents_mermaid`, `find_dependents_dot` in `format.rs`, all 4 edit input structs in `edit.rs`, `search_text` handler in `tools.rs` (terms parameter), and `DependentLineView`/`find_dependents_for_file` in `query.rs`. Avoid modifying those symbols. If unavoidable, coordinate via commit message.

---

## Wave 1 — High Priority

### Task 1: JSONC Comment Support (P0)

**Files:**
- Modify: `Cargo.toml:13-52` (add dependency)
- Modify: `src/parsing/config_extractors/json.rs:11-66` (`extract` method)
- Test: `src/parsing/config_extractors/json.rs:360-463` (inline tests)
- Test: `tests/config_files.rs` (integration tests)

**Problem:** `serde_json::from_slice` cannot parse JSON with `//` or `/* */` comments. Every `tsconfig.json` in TypeScript projects fails with `"serde_json: expected value at line 1 column 1"`.

**Approach:** Strip comments before parsing using a minimal inline function. Avoids adding a dependency — comment stripping for JSON is simple enough to inline (no nested strings edge cases beyond what we already handle).

- [ ] **Step 1: Write failing unit test for JSONC with line comments**

In `src/parsing/config_extractors/json.rs`, add inside `mod tests`:

```rust
#[test]
fn test_jsonc_line_comments() {
    let input = br#"{
        // This is a line comment
        "compilerOptions": {
            "target": "es2022", // inline comment
            "module": "commonjs"
        }
    }"#;
    let result = JsonExtractor.extract(input);
    assert!(
        matches!(result.outcome, ExtractionOutcome::Ok),
        "JSONC with line comments should parse successfully, got: {:?}",
        result.outcome
    );
    assert!(!result.symbols.is_empty(), "should extract symbols from JSONC");
    let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"compilerOptions"), "should find compilerOptions key");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib config_extractors::json::tests::test_jsonc_line_comments -- --test-threads=1`
Expected: FAIL — `serde_json` cannot parse comments

- [ ] **Step 3: Write failing unit test for JSONC with block comments**

In `src/parsing/config_extractors/json.rs`, add inside `mod tests`:

```rust
#[test]
fn test_jsonc_block_comments() {
    let input = br#"/* tsconfig for the app */
{
    "extends": "./tsconfig.base.json",
    /* compiler settings */
    "include": ["src/**/*.ts"]
}"#;
    let result = JsonExtractor.extract(input);
    assert!(
        matches!(result.outcome, ExtractionOutcome::Ok),
        "JSONC with block comments should parse successfully, got: {:?}",
        result.outcome
    );
    assert!(!result.symbols.is_empty());
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo test --lib config_extractors::json::tests::test_jsonc_block_comments -- --test-threads=1`
Expected: FAIL

- [ ] **Step 5: Implement `strip_json_comments` helper**

Add this function in `src/parsing/config_extractors/json.rs` before the `impl ConfigExtractor for JsonExtractor` block (before line 10):

```rust
/// Strip `//` line comments and `/* ... */` block comments from JSON content.
/// Preserves byte positions by replacing comment characters with spaces,
/// so line numbers and column offsets in error messages remain accurate.
fn strip_json_comments(input: &[u8]) -> Vec<u8> {
    let mut output = input.to_vec();
    let len = output.len();
    let mut i = 0;
    while i < len {
        match output[i] {
            b'"' => {
                // Skip string literals — don't strip inside strings.
                i += 1;
                while i < len {
                    if output[i] == b'\\' {
                        i += 2; // skip escaped character
                    } else if output[i] == b'"' {
                        i += 1;
                        break;
                    } else {
                        i += 1;
                    }
                }
            }
            b'/' if i + 1 < len && output[i + 1] == b'/' => {
                // Line comment: replace until newline with spaces.
                while i < len && output[i] != b'\n' {
                    output[i] = b' ';
                    i += 1;
                }
            }
            b'/' if i + 1 < len && output[i + 1] == b'*' => {
                // Block comment: replace until */ with spaces (preserve newlines).
                output[i] = b' ';
                i += 1;
                output[i] = b' ';
                i += 1;
                while i < len {
                    if output[i] == b'*' && i + 1 < len && output[i + 1] == b'/' {
                        output[i] = b' ';
                        i += 1;
                        output[i] = b' ';
                        i += 1;
                        break;
                    }
                    if output[i] != b'\n' {
                        output[i] = b' ';
                    }
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    output
}
```

- [ ] **Step 6: Update `extract` to strip comments before parsing**

In `JsonExtractor::extract()` (line 12), change:

```rust
// OLD:
let value: serde_json::Value = match serde_json::from_slice(content) {
```

to:

```rust
// NEW:
let cleaned = strip_json_comments(content);
let value: serde_json::Value = match serde_json::from_slice(&cleaned) {
```

- [ ] **Step 7: Run both JSONC tests to verify they pass**

Run: `cargo test --lib config_extractors::json::tests::test_jsonc -- --test-threads=1`
Expected: PASS (both `test_jsonc_line_comments` and `test_jsonc_block_comments`)

- [ ] **Step 8: Write test for trailing commas (should still fail gracefully)**

```rust
#[test]
fn test_jsonc_trailing_commas_still_fail() {
    // Trailing commas are NOT valid JSON or JSONC — only JSON5 supports them.
    // Ensure we don't accidentally break valid error reporting.
    let input = br#"{"a": 1, "b": 2,}"#;
    let result = JsonExtractor.extract(input);
    assert!(
        matches!(result.outcome, ExtractionOutcome::Failed(_)),
        "trailing commas should still produce Failed outcome"
    );
}
```

- [ ] **Step 9: Write test that comment stripping preserves string contents**

```rust
#[test]
fn test_jsonc_comments_inside_strings_preserved() {
    let input = br#"{"url": "https://example.com", "pattern": "// not a comment"}"#;
    let result = JsonExtractor.extract(input);
    assert!(matches!(result.outcome, ExtractionOutcome::Ok));
    let names: Vec<&str> = result.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"url"));
    assert!(names.contains(&"pattern"));
}
```

- [ ] **Step 10: Run all JSON extractor tests**

Run: `cargo test --lib config_extractors::json -- --test-threads=1`
Expected: ALL PASS

- [ ] **Step 11: Run integration tests**

Run: `cargo test --test config_files -- --test-threads=1`
Expected: ALL PASS (existing tests unaffected)

- [ ] **Step 12: Commit**

```bash
git add src/parsing/config_extractors/json.rs
git commit -m "fix: add JSONC comment support for tsconfig and similar files

Strip // and /* */ comments from JSON before parsing with serde_json.
Every tsconfig.json in TypeScript projects uses comments, causing 100%
parse failure rate. Comment characters are replaced with spaces to
preserve line/column positions in error messages.

Fixes reviewer feedback P0."
```

---

### Task 2: Token Savings Aggregation (P1)

**Files:**
- Modify: `src/protocol/mod.rs:127-136` (`record_read_savings`)
- Modify: `src/protocol/tools.rs` (multiple tool handlers — NOT `search_text` handler internals, just add savings call at the end)
- Test: manual via `health` tool after calling tools

**Problem:** Only `get_file_context` and `get_symbol_context` call `record_read_savings`. The other 22 tools save tokens but don't report savings, so `health` shows "~0 tokens saved" for most workflows.

**Approach:** Add a generic `record_tool_savings` method and call it from tool handlers that replace raw file reads. Avoid touching the `search_text` handler body (other agent scope) — instead, add the call at the format/return point.

**Note:** The `search_text` handler returns at multiple points. The other agent is modifying this handler (passing terms to the formatter). To avoid conflicts, we will NOT modify the `search_text` handler. We will add savings recording to: `search_symbols`, `get_symbol` (single and batch), `find_references`, `explore`, `diff_symbols`, `get_file_content`. The `search_text`, `find_dependents`, and `inspect_match` savings can be added after the other agent's work is merged or in a follow-up.

- [ ] **Step 1: Add `record_tool_savings` method to `SymForgeServer`**

In `src/protocol/mod.rs`, after the existing `record_read_savings` method (line 136), add:

```rust
    /// Record token savings from any MCP tool that replaces raw file reads.
    /// `estimated_raw_tokens`: approximate tokens the user would have consumed via raw reads.
    /// `output_tokens`: tokens in the actual tool response.
    pub(crate) fn record_tool_savings(&self, estimated_raw_tokens: u64, output_tokens: u64) {
        if let Some(ref stats) = self.token_stats {
            let saved = estimated_raw_tokens.saturating_sub(output_tokens);
            stats
                .read_saved_tokens
                .fetch_add(saved, std::sync::atomic::Ordering::Relaxed);
            stats
                .read_fires
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
```

- [ ] **Step 2: Add savings to `get_symbol` handler**

In `src/protocol/tools.rs`, in the `get_symbol` function, find the final return that produces the formatted output. Before returning, estimate savings. The symbol body is typically 5-20% of the file. Estimate raw cost as `output.len() * 5` (the user would have read the whole file to find the symbol).

Locate the end of `get_symbol` (around line 1593). Just before the final formatted string is returned, add:

```rust
self.record_tool_savings((output.len() * 5 / 4) as u64, (output.len() / 4) as u64);
```

Apply this pattern to each handler listed below — estimate raw cost conservatively based on the tool's purpose:

- [ ] **Step 3: Add savings to `search_symbols` handler**

In the `search_symbols` handler, before the return, add:
```rust
// Symbol search replaces scanning all files; estimate 10x savings
self.record_tool_savings((output.len() * 10 / 4) as u64, (output.len() / 4) as u64);
```

- [ ] **Step 4: Add savings to `find_references` handler**

In the `find_references` handler, before each return of successful results, add:
```rust
self.record_tool_savings((output.len() * 8 / 4) as u64, (output.len() / 4) as u64);
```

Note: the `find_references` handler has four return paths: proxy result (early return, skip), implementations mode (skip — different logic), compact view, and full view. Add the savings call before the compact and full view returns only. Do NOT modify `find_implementations_result_view` — just add before the `format::find_references_result_view` and `format::find_references_compact_view` calls.

- [ ] **Step 5: Add savings to `explore` handler**

At the end of the `explore` handler, before the final return of `output`, add:
```rust
self.record_tool_savings((output.len() * 10 / 4) as u64, (output.len() / 4) as u64);
```

- [ ] **Step 6: Add savings to `diff_symbols` handler**

Before the return of `format::diff_symbols_result_view(...)`, capture the result and add savings:
```rust
let output = format::diff_symbols_result_view(base, target, &changed_files, &repo, params.0.compact.unwrap_or(false));
self.record_tool_savings((output.len() * 5 / 4) as u64, (output.len() / 4) as u64);
output
```

- [ ] **Step 7: Add savings to `get_file_content` handler**

In the `get_file_content` handler, the tool already has access to `raw_chars` (the full file size). Find where the successful response is returned and add:
```rust
self.record_tool_savings((raw_chars / 4) as u64, (output.len() / 4) as u64);
```

- [ ] **Step 8: Run full test suite**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: ALL PASS (savings recording is purely additive)

- [ ] **Step 9: Commit**

```bash
git add src/protocol/mod.rs src/protocol/tools.rs
git commit -m "feat: aggregate token savings across all major tool handlers

Previously only get_file_context and get_symbol_context reported savings
to the health endpoint. Now search_symbols, get_symbol, find_references,
explore, diff_symbols, and get_file_content also contribute, giving
accurate session-wide savings totals.

Fixes reviewer feedback P1."
```

---

## Wave 2 — Medium Priority

### Task 3: Fix `diff_symbols` String Constant False Positives (P3)

**Files:**
- Modify: `src/protocol/format.rs:6422-6472` (`extract_declaration_name`)
- Test: `src/protocol/format.rs` (add test near existing tests for this function, or in a tests module)

**Problem:** In C#, `const string Foo = "bar"` causes `extract_declaration_name` to capture `string` (the type) as the symbol name. The function matches `const ` then grabs the next word, but in C# the next word after `const` is the type, not the name.

**Approach:** After matching `const `, check if the captured word is a known C#/TypeScript/Rust primitive type. If so, skip it and capture the next identifier.

- [ ] **Step 1: Write failing test**

Add a test in `src/protocol/format.rs` (find the `#[cfg(test)]` module nearest to `extract_declaration_name`):

```rust
#[test]
fn test_extract_declaration_name_csharp_const() {
    // C# const: `const string Foo = "bar"` — name is Foo, not string
    assert_eq!(
        extract_declaration_name("const string ConnectionString = \"...\";"),
        Some("ConnectionString".to_string())
    );
    assert_eq!(
        extract_declaration_name("const int MaxRetries = 3;"),
        Some("MaxRetries".to_string())
    );
    // Rust const should still work (type after colon, not before name)
    assert_eq!(
        extract_declaration_name("const MAX_SIZE: usize = 100;"),
        Some("MAX_SIZE".to_string())
    );
}
```

Note: We do NOT test `"public const bool IsEnabled = true;"` here because the
existing `strip_prefix("pub")` logic incorrectly matches the `pub` in `public`,
producing `"lic const bool IsEnabled..."`. That is a pre-existing bug in C#
access modifier handling, out of scope for this fix. A separate task could add
proper `public`/`protected`/`private`/`internal` stripping for C#/Java.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib protocol::format::tests::test_extract_declaration_name_csharp_const -- --test-threads=1`
Expected: FAIL — `ConnectionString` assertion fails, gets `Some("string")` instead

- [ ] **Step 3: Fix `extract_declaration_name` to skip type keywords after `const`**

In `src/protocol/format.rs`, in `extract_declaration_name`, replace the `const ` arm of the keywords loop. Currently the function loops over keywords and extracts the first alphanumeric word after the keyword. Change the logic for `const `:

After the existing keyword loop (line ~6455-6465), add special handling. Replace the keyword match block with:

```rust
    for kw in &keywords {
        if let Some(rest) = stripped.strip_prefix(kw) {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if name.is_empty() {
                continue;
            }
            // For `const`, the first word might be a type name (C#: `const string Foo`).
            // If it looks like a well-known type, skip it and take the next identifier.
            if *kw == "const " && is_likely_type_keyword(&name) {
                let after_type = &rest[name.len()..].trim_start();
                let real_name: String = after_type
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !real_name.is_empty() {
                    return Some(real_name);
                }
            }
            return Some(name);
        }
    }
    None
```

And add this helper right above `extract_declaration_name`:

```rust
/// Check if a word is a well-known type keyword that would appear between
/// `const` and the actual variable name in C#, Java, or TypeScript.
fn is_likely_type_keyword(word: &str) -> bool {
    matches!(
        word,
        "string" | "String" | "int" | "Int32" | "Int64"
            | "bool" | "Boolean" | "float" | "double" | "decimal"
            | "char" | "byte" | "long" | "short" | "uint"
            | "object" | "var" | "number" | "bigint" | "any"
    )
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib protocol::format::tests::test_extract_declaration_name_csharp_const -- --test-threads=1`
Expected: PASS

- [ ] **Step 5: Run full test suite for regressions**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add src/protocol/format.rs
git commit -m "fix: diff_symbols no longer reports type keywords as symbol names

In C#, 'const string Foo' was extracting 'string' as the symbol name.
Now extract_declaration_name skips known type keywords after 'const'
and captures the actual variable name.

Fixes reviewer feedback P3."
```

---

### Task 4: Improve `get_symbol_context` Default Mode (P4)

**Files:**
- Modify: `src/protocol/tools.rs:1755-1925` (`get_symbol_context` handler)
- Modify: `src/sidecar/handlers.rs:828-842` (`symbol_context_text` output)

**Problem:** Two sub-issues:
1. Without `path`, `file_path_hint` is `None` → no definition body shown
2. When zero references exist, output is silent instead of showing "0 callers, 0 callees"

- [ ] **Step 1: Fix default mode to auto-resolve path when omitted**

In `src/protocol/tools.rs`, in the `get_symbol_context` handler, find the default mode section (after the `sections` check, around line 1862). Replace the `file_path_hint` binding:

```rust
// OLD:
let file_path_hint = params.0.path.as_deref().or(params.0.file.as_deref());

// NEW:
let file_path_hint = params.0.path.as_deref().or(params.0.file.as_deref());
// Auto-resolve path from index when not provided
let resolved_path: Option<String>;
let file_path_hint = if file_path_hint.is_some() {
    file_path_hint
} else {
    let guard = self.index.read();
    let candidates: Vec<String> = guard
        .all_files()
        .filter_map(|(path, file)| {
            if file.symbols.iter().any(|s| s.name == params.0.name) {
                Some(path.to_string())
            } else {
                None
            }
        })
        .take(5)
        .collect();
    drop(guard);
    if candidates.len() == 1 {
        resolved_path = Some(candidates.into_iter().next().unwrap());
        resolved_path.as_deref()
    } else if candidates.len() > 1 {
        resolved_path = Some(candidates[0].clone());
        // Will append disambiguation note later
        resolved_path.as_deref()
    } else {
        None
    }
};
```

Note: `resolved_path` must be declared before the `if` block so it lives long enough. Also add a disambiguation note after the output is built (before the return), when `candidates.len() > 1` and `params.0.path.is_none()`:

After the output is assembled but before returning, if the path was auto-resolved from multiple candidates:

```rust
// After building output, before return:
if params.0.path.is_none() && params.0.file.is_none() {
    if let Some(ref resolved) = resolved_path {
        // Check if there were multiple candidates
        let guard = self.index.read();
        let count = guard.all_files()
            .filter(|(_, file)| file.symbols.iter().any(|s| s.name == params.0.name))
            .count();
        drop(guard);
        if count > 1 {
            output.push_str(&format!(
                "\n\nNote: {} symbols named \"{}\" found — showing from {}. Specify path for precision.",
                count, params.0.name, resolved
            ));
        }
    }
}
```

- [ ] **Step 2: Fix empty references display**

In `src/sidecar/handlers.rs`, in `symbol_context_text`, after the file loop (around line 842), when `lines` is empty, add explicit zero-count message:

```rust
// After the file loop ends (line 842):
if lines.is_empty() {
    lines.push("No references found in the index.".to_string());
    lines.push("Tip: this symbol may only be used via dynamic dispatch, reflection, or external entry points.".to_string());
}
```

- [ ] **Step 3: Run test suite**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
git add src/protocol/tools.rs src/sidecar/handlers.rs
git commit -m "fix: get_symbol_context auto-resolves path and shows empty reference counts

When called without path, now searches the index for the symbol and uses
the first match. Shows disambiguation note when multiple files contain
the same symbol name. When zero references exist, explicitly shows
'No references found' instead of empty output.

Fixes reviewer feedback P4."
```

---

### Task 5: List Failed Files in Health Report (P5)

**Files:**
- Modify: `src/live_index/query.rs:647-674` (`HealthStats` struct)
- Modify: `src/live_index/query.rs:2223-2265` (`health_stats` method)
- Modify: `src/protocol/format.rs:992-1064` (`health_report_from_stats`)

**Problem:** Health shows `"N failed"` count but never lists which files failed or why. `partial_parse_files` is listed (up to 10) but `failed_files` is not.

- [ ] **Step 1: Add `failed_files` field to `HealthStats`**

In `src/live_index/query.rs`, in the `HealthStats` struct, after `partial_parse_files` (around line 670), add:

```rust
    /// Sorted, deduplicated list of files with failed parse status and their error messages.
    pub failed_files: Vec<(String, String)>,
```

- [ ] **Step 2: Populate `failed_files` in `health_stats()`**

In the `health_stats()` method, after the `partial_parse_files` collection (around line 2258), add:

```rust
        let mut failed_files: Vec<(String, String)> = self
            .files
            .iter()
            .filter_map(|(path, f)| {
                if let ParseStatus::Failed { error } = &f.parse_status {
                    Some((path.clone(), error.clone()))
                } else {
                    None
                }
            })
            .collect();
        failed_files.sort_by(|a, b| a.0.cmp(&b.0));
```

Then add `failed_files` to the `HealthStats` initializer:

```rust
        HealthStats {
            // ... existing fields ...
            partial_parse_files,
            failed_files,  // ADD THIS
            tier_counts: self.tier_counts(),
        }
```

- [ ] **Step 3: Display failed files in health report**

In `src/protocol/format.rs`, in `health_report_from_stats`, after the `partial_parse_files` display block (around line 1062), add:

```rust
    if !stats.failed_files.is_empty() {
        output.push_str(&format!(
            "\nFailed files ({}):\n",
            stats.failed_files.len()
        ));
        for (i, (path, error)) in stats.failed_files.iter().take(10).enumerate() {
            output.push_str(&format!("  {}. {} — {}\n", i + 1, path, error));
        }
        if stats.failed_files.len() > 10 {
            output.push_str(&format!(
                "  ... and {} more failed files\n",
                stats.failed_files.len() - 10
            ));
        }
    }
```

- [ ] **Step 4: Run test suite**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: ALL PASS

- [ ] **Step 5: Commit**

```bash
git add src/live_index/query.rs src/protocol/format.rs
git commit -m "feat: list failed files with error messages in health report

Health previously showed only a count of failed files. Now lists up to
10 failed files with their parse error messages, matching the existing
pattern for partial-parse files.

Fixes reviewer feedback P5."
```

---

## Wave 3 — Low Priority

### Task 6: Improve `find_references` Implementations Messaging (P6)

**Files:**
- Modify: `src/protocol/tools.rs:2696-2758` (`find_references` handler, implementations branch)

**Problem:** `"No implementations found for WorkflowsService"` doesn't explain that classes can't have implementations — only interfaces/traits can.

**Approach:** After getting an empty implementations result, check the symbol's kind in the index. If it's a class/struct, return a more helpful message. Do NOT modify `find_implementations_result_view` in `format.rs` (other agent's file). Instead, handle this in the tool handler before calling the formatter.

- [ ] **Step 1: Add kind-aware messaging in the handler**

In `src/protocol/tools.rs`, in the `find_references` handler, in the `mode == "implementations"` branch, after `format::find_implementations_result_view(...)` is called, check if the result indicates empty and enrich:

Replace the implementations return (around line 2728):

```rust
// OLD:
return format::find_implementations_result_view(&view, &input.name, &limits);

// NEW:
let result = format::find_implementations_result_view(&view, &input.name, &limits);
if view.entries.is_empty() {
    // Check if the symbol is a class/struct (not an interface/trait)
    let guard = self.index.read();
    let is_concrete = guard.all_files().any(|(_, file)| {
        file.symbols.iter().any(|s| {
            s.name == input.name
                && matches!(
                    s.kind,
                    crate::domain::SymbolKind::Class | crate::domain::SymbolKind::Struct
                )
        })
    });
    drop(guard);
    if is_concrete {
        return format!(
            "No implementations found for \"{}\" — it is a class/struct, not an interface/trait.\n\
             Use find_references with mode=\"references\" to find callers and usages instead.",
            input.name
        );
    }
}
return result;
```

- [ ] **Step 2: Run test suite**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: ALL PASS

- [ ] **Step 3: Commit**

```bash
git add src/protocol/tools.rs
git commit -m "fix: find_references implementations mode explains class vs interface

When searching for implementations of a class/struct (which can't have
implementations), now returns a helpful message explaining the
distinction and suggesting mode=references instead.

Fixes reviewer feedback P6."
```

---

### Task 7: Paginate `get_repo_map` detail=full (P7)

**Files:**
- Modify: `src/protocol/tools.rs:478-486` (`GetRepoMapInput`)
- Modify: `src/protocol/tools.rs:1603-1693` (`get_repo_map` handler, full branch)

**Problem:** `detail=full` dumps every file with symbol counts, unbounded. Large repos (1000+ files) produce overwhelming output.

- [ ] **Step 1: Add `max_files` parameter to `GetRepoMapInput`**

In `src/protocol/tools.rs`, add to `GetRepoMapInput`:

```rust
    /// Maximum number of files to include in the output (only used when detail="full", default: 200).
    #[serde(default, deserialize_with = "lenient_u32")]
    pub max_files: Option<u32>,
```

- [ ] **Step 2: Apply limit in the full mode handler**

In the `get_repo_map` handler, in the `"full"` branch, after `format::repo_outline_view(...)` is called, truncate the output. Better approach: truncate the file list before formatting.

Find the `"full"` branch (around line 1604). After the view is obtained but before formatting, apply the limit:

For the unfiltered path (no `path` prefix), change:

```rust
// OLD:
format::repo_outline_view(&view, &self.project_name)

// NEW:
let max_files = params.0.max_files.unwrap_or(200) as usize;
if view.files.len() > max_files {
    let truncated_files: Vec<_> = view.files.iter().take(max_files).cloned().collect();
    let remaining = view.files.len() - max_files;
    let truncated_view = crate::live_index::query::RepoOutlineView {
        total_files: view.total_files,
        total_symbols: view.total_symbols,
        files: truncated_files,
    };
    let mut output = format::repo_outline_view(&truncated_view, &self.project_name);
    output.push_str(&format!(
        "\n\n... and {} more files (use path= to scope or increase max_files=)",
        remaining
    ));
    output
} else {
    format::repo_outline_view(&view, &self.project_name)
}
```

Apply the same truncation to the filtered path (with `path` prefix). Replace:

```rust
// OLD:
format::repo_outline_view(&filtered_view, &self.project_name)

// NEW:
let max_files = params.0.max_files.unwrap_or(200) as usize;
if filtered_view.files.len() > max_files {
    let remaining = filtered_view.files.len() - max_files;
    let truncated_files: Vec<_> = filtered_view.files.iter().take(max_files).cloned().collect();
    let truncated_view = crate::live_index::query::RepoOutlineView {
        total_files: filtered_view.total_files,
        total_symbols: filtered_view.total_symbols,
        files: truncated_files,
    };
    let mut output = format::repo_outline_view(&truncated_view, &self.project_name);
    output.push_str(&format!(
        "\n\n... and {} more files (increase max_files= to see more)",
        remaining
    ));
    output
} else {
    format::repo_outline_view(&filtered_view, &self.project_name)
}
```

- [ ] **Step 3: Run test suite**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
git add src/protocol/tools.rs
git commit -m "feat: add max_files pagination to get_repo_map detail=full

Defaults to 200 files. Shows total count and a hint to use path= or
increase max_files= when truncated. Prevents context overflow in
large repos.

Fixes reviewer feedback P7."
```

---

### Task 8: Improve `get_file_content around_symbol` Zero-Symbol Message (P8)

**Files:**
- Modify: `src/protocol/format.rs:1762-1799` (`not_found_symbol_names`)

**Problem:** When a file has zero indexed symbols (e.g., C# top-level statements), the message `"No symbols in that file"` doesn't explain WHY.

- [ ] **Step 1: Update the empty symbols message**

In `src/protocol/format.rs`, in `not_found_symbol_names`, replace the empty check (line 1764-1766):

```rust
// OLD:
if symbol_names.is_empty() {
    return format!("No symbol {name} in {relative_path}. No symbols in that file.");
}

// NEW:
if symbol_names.is_empty() {
    return format!(
        "No symbol {name} in {relative_path}. \
         This file has no indexed symbols — it may use top-level statements, \
         expression-bodied code, or a syntax not extracted by the parser. \
         Use get_file_content without around_symbol to read the raw file."
    );
}
```

- [ ] **Step 2: Run test suite**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: ALL PASS

- [ ] **Step 3: Commit**

```bash
git add src/protocol/format.rs
git commit -m "fix: improve error message when around_symbol targets a zero-symbol file

Explains that the file may use top-level statements or unsupported syntax
and suggests using get_file_content without around_symbol instead.

Fixes reviewer feedback P8."
```

---

### Task 9: Add Language Filter to `explore` (P9)

**Files:**
- Modify: `src/protocol/tools.rs:614-629` (`ExploreInput`)
- Modify: `src/protocol/tools.rs:2818-3133` (`explore` handler)

**Problem:** `explore` has no `language` or `path_prefix` filter, causing noise from config files when searching for code concepts.

- [ ] **Step 1: Add filter params to `ExploreInput`**

In `src/protocol/tools.rs`, add to `ExploreInput`:

```rust
    /// Optional canonical language name filter (e.g., "Rust", "TypeScript", "C#").
    pub language: Option<String>,
    /// Optional relative path prefix scope (e.g., "src/", "backend/").
    pub path_prefix: Option<String>,
```

- [ ] **Step 2: Apply filters in the explore handler**

In the `explore` handler, the symbol and text search phases need to respect the filters. There are two integration points:

**Early in the handler:** Parse the language filter once at the top of the `explore` handler (after the proxy check, around line 2823), using the existing `parse_language_filter` helper:

```rust
let lang_filter = match parse_language_filter(params.0.language.as_deref()) {
    Ok(f) => f,
    Err(e) => return e,
};
```

**Phase 1 (symbol search):** After the symbol search loop, filter `match_counts` by language and path_prefix:

```rust
// After: for sq in &all_symbol_queries { ... }
// Filter match_counts by language and path_prefix
if lang_filter.is_some() || params.0.path_prefix.is_some() {
    match_counts.retain(|(_, _, path), _| {
        if let Some(ref prefix) = params.0.path_prefix {
            if !path.starts_with(prefix.as_str()) {
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
    });
}
```

**Phase 2 (text search):** Set `path_scope` on `TextSearchOptions` (NOT `path_prefix` — that field doesn't exist). `TextSearchOptions` uses `path_scope: PathScope` where `PathScope` is an enum at `src/live_index/search.rs:31-35`:

Before the text search loop, configure the path scope:
```rust
if let Some(ref prefix) = params.0.path_prefix {
    options.path_scope = search::PathScope::Prefix(prefix.clone());
}
if let Some(ref lang) = lang_filter {
    options.language_filter = Some(lang.clone());
}
```

Also filter `text_hits` by path_prefix after collection (language is already handled by `TextSearchOptions.language_filter`):
```rust
if let Some(ref prefix) = params.0.path_prefix {
    text_hits.retain(|(path, _, _)| path.starts_with(prefix.as_str()));
}
```

- [ ] **Step 3: Run test suite**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
git add src/protocol/tools.rs
git commit -m "feat: add language and path_prefix filters to explore tool

Allows scoping concept exploration to specific languages or directory
subtrees, reducing noise from config files and non-code matches.

Fixes reviewer feedback P9."
```

---

## Deferred

### P10: Angular Template Partial Parse

Tree-sitter grammar limitation. Symbols from class body ARE extracted correctly. The partial-parse health listing (already shows up to 10 files) provides visibility. No action unless the other agent's work creates an opening for template pre-processing.

---

## Verification Checklist

After all tasks are complete:

- [ ] `cargo test --all-targets -- --test-threads=1` — all green
- [ ] `cargo fmt -- --check` — no formatting issues
- [ ] `cargo check` — clean compilation
- [ ] Manual test: call `health` after 10+ tool calls — verify token savings > 0
- [ ] Manual test: parse a `tsconfig.json` with comments — verify `Processed` status
- [ ] Manual test: call `find_references mode=implementations` on a class — verify helpful message
- [ ] Manual test: call `get_repo_map detail=full` on a large repo — verify truncation at 200 files
