# Protocol Subsystem Bug Audit

Auditor: Claude Opus 4.6 (1M context)
Date: 2026-03-20
Scope: `src/protocol/` (8 files, 826 symbols)

---

## BUG 1: `edit_within_symbol` tool handler does not include doc comments in body scan (tools.rs)

**File:** `src/protocol/tools.rs`, lines 3781-3783
**Buggy code:**
```rust
let sym_start = sym.byte_range.0 as usize;
let sym_end = sym.byte_range.1 as usize;
let body = &file.content[sym_start..sym_end];
```

**What it does wrong:** The tool handler uses `sym.byte_range.0` to define the body to search within, but then splices the replacement using `sym.byte_range` (line 3831):
```rust
let new_content = edit::apply_splice(&file.content, sym.byte_range, new_body.as_bytes());
```

Meanwhile, the `build_edit_within` helper in `edit.rs` (line 538) uses `sym.effective_start()` for both the body scan AND the splice range (line 565):
```rust
let sym_start = sym.effective_start() as usize;  // includes doc comments
// ...
let effective_range = (sym.effective_start(), sym.byte_range.1);
let new_content = apply_splice(file_content, effective_range, new_body.as_bytes());
```

This means the inline handler and the shared helper have different behavior. The inline handler:
1. Reads body from `byte_range.0..byte_range.1` (excludes doc comments)
2. Does find-and-replace on that body
3. Splices the result over `byte_range.0..byte_range.1` (excludes doc comments)

This is actually internally consistent for the handler, BUT it means if the user's `old_text` appears in a doc comment that is part of the symbol's `effective_start()` range but not `byte_range.0`, it won't be found. The handler also does NOT normalize the `old_text` line endings, only the `new_text` -- so on CRLF files, if the user sends LF-terminated `old_text`, the match will fail silently.

However, the more significant issue is that the handler does NOT call `build_edit_within` at all and instead duplicates its logic with a different byte range definition. The batch_edit path (edit.rs line 897) DOES call `build_edit_within`, creating inconsistent behavior between:
- `edit_within_symbol` tool (uses `byte_range.0`)
- `batch_edit` with `edit_within` operation (uses `effective_start()`)

**Correct behavior:** The `edit_within_symbol` tool handler should call `edit::build_edit_within()` like the batch_edit path does, or at minimum use `effective_start()` consistently.

**Severity:** Medium. Most symbols have `effective_start() == byte_range.0` (when there's no doc_byte_range). But for symbols with doc comments attached, the two codepaths behave differently.

---

## BUG 2: `edit_within_symbol` handler does not normalize `old_text` line endings (tools.rs)

**File:** `src/protocol/tools.rs`, lines 3790-3793
**Buggy code:**
```rust
let normalized_new =
    edit::normalize_line_endings(params.0.new_text.as_bytes(), line_ending);
let normalized_new_str =
    String::from_utf8(normalized_new).unwrap_or_else(|_| params.0.new_text.clone());
```

**What it does wrong:** Only `new_text` is normalized to match the file's line endings. The `old_text` used for matching (line 3795-3801) is used as-is. On CRLF files, the file content will have `\r\n` line endings, but the LLM client typically sends `\n`-only text. The `old_text` with `\n` won't match the body content with `\r\n`, causing a silent "not found" error.

**Correct behavior:** The `old_text` should also be normalized to the file's line ending style before matching, OR the body should be normalized to LF before matching (and then the splice should account for the difference).

Note: The `build_edit_within` helper in `edit.rs` also does NOT normalize `old_text`, so both paths have the same bug. However, MCP clients overwhelmingly send LF-only text, and most codebases use LF, so this primarily affects Windows-style CRLF files.

**Severity:** Medium. Causes silent failures on CRLF files when `old_text` contains newlines.

---

## BUG 3: `is_noise_line` false positive on Python decorators and Rust attributes (format.rs)

**File:** `src/protocol/format.rs`, line 293
**Buggy code:**
```rust
|| trimmed.starts_with('#')
```

**What it does wrong:** This marks any line starting with `#` as "noise" (import/comment). But in Python, `#` is a comment prefix, while in Rust `#[derive(...)]` and `#[cfg(...)]` are attributes (not noise). More critically, in Markdown, `# Heading` is a heading line. The function explicitly exempts `#[` by including `#include` as a separate check, but `#[` is NOT exempted -- any line starting with `#` is treated as noise.

This means:
- Python lines like `# TODO: important` are correctly filtered as comments
- BUT Rust attribute lines like `#[derive(Debug)]`, `#[test]`, `#[cfg(test)]` are incorrectly filtered as "noise"
- Markdown headings `# Section Title` are incorrectly filtered as "noise"
- YAML comments `# some config note` are correctly filtered

**Where this matters:** The `is_noise_line` function is used in:
1. `search_text` with `group_by="usage"` -- filters out matched lines
2. `explore` tool -- filters text search hits

So a `search_text` query for `"derive"` with `group_by="usage"` would silently hide all `#[derive(...)]` lines. Similarly, searching for `"test"` with usage grouping would hide `#[test]` attribute lines.

**Correct behavior:** The `#` check should be more specific. At minimum, `#[` (Rust/Python decorators) and `# ` followed by uppercase (Markdown headings) should be exempted.

**Severity:** Medium. Causes legitimate search results to be silently hidden in the usage/explore filter paths.

---

## BUG 4: `is_noise_line` false positive on lines starting with `*` (format.rs)

**File:** `src/protocol/format.rs`, line 295
**Buggy code:**
```rust
|| trimmed.starts_with('*')
```

**What it does wrong:** This is intended to catch block comment continuation lines (`* some doc text`), but it also matches:
- Pointer dereference: `*ptr = value;`
- Glob patterns: `*.rs`
- Multiplication: `*factor`
- CSS selectors: `* { margin: 0; }`
- Markdown list items: `* item one`

Any code line starting with `*` after trimming whitespace would be silently filtered as noise by the `usage` group_by mode and the `explore` tool.

**Correct behavior:** Should check for `* ` (star followed by space, typical of block comment continuation) rather than bare `*`. Even better, check `trimmed.starts_with("* ")` or `trimmed == "*"` (closing of `/* ... */` blocks uses `*/`).

**Severity:** Low-Medium. Relatively uncommon for search results to match lines starting with `*` in real code, but when they do, results are silently hidden.

---

## BUG 5: `is_noise_line` catches `require()` anywhere in the line, not just at start (format.rs)

**File:** `src/protocol/format.rs`, line 297
**Buggy code:**
```rust
|| line.contains("require(")
```

**What it does wrong:** Unlike all other checks which use `trimmed.starts_with(...)`, this one uses `line.contains(...)`, meaning ANY line containing the substring `require(` anywhere is marked as noise. This would incorrectly filter lines like:
- `if (module.require(dependency))` -- a method call named `require`
- `// This function will require(...)` -- within a comment, yes, but also within code
- `const isRequired = require("module")` -- this IS an import, but so is `const x = require("foo"); doSomething(x);` on the same line

The key issue is that `line.contains` (using the raw line, not trimmed) matches `require(` at any position, making it far broader than intended.

**Correct behavior:** Should use `trimmed.starts_with("require(")` or `trimmed.starts_with("const ") && line.contains("require(")` to be consistent with the other import-detection patterns.

**Severity:** Low-Medium. False positives in JavaScript/TypeScript code where `require(` appears mid-line.

---

## BUG 6: `replace_symbol_body` dry run reports `new_body.len()` as byte count (tools.rs)

**File:** `src/protocol/tools.rs`, lines 3511-3518
**Buggy code:**
```rust
if params.0.dry_run == Some(true) {
    let old_bytes = (sym.byte_range.1 - sym.byte_range.0) as usize;
    return format!(
        "[DRY RUN] Would replace `{}` in {} (old: {} bytes -> new: {} bytes)",
        params.0.name,
        params.0.path,
        old_bytes,
        params.0.new_body.len()   // <-- raw input length
    );
}
```

And the actual replacement path (lines 3520-3572) applies indentation, line ending normalization, and orphaned doc extension, which changes the actual number of bytes written. The dry run reports `new_body.len()` (the raw user input length) while the actual write path uses the indented+normalized length.

Similarly, the final summary at line 3572 also uses `params.0.new_body.len()` instead of the actual `indented.len()`:
```rust
edit_format::format_replace(
    &params.0.path,
    &params.0.name,
    &sym.kind.to_string(),
    old_bytes,
    params.0.new_body.len(),  // <-- raw input, not indented
);
```

**What it does wrong:** Reports the wrong "new bytes" count. The actual written content includes indentation prefix on each line and normalized line endings, which can be significantly different from the raw input length. On CRLF files, every `\n` becomes `\r\n`, so the actual byte count is larger. With indentation, each line gets prefix bytes added.

**Correct behavior:** The summary should report the actual byte count of the indented+normalized content that was written.

**Severity:** Low. Cosmetic -- the byte counts in the output message are wrong, but the actual edit is correct.

---

## BUG 7: `batch_edit` applies edits using stale byte ranges after earlier edits in same file (edit.rs)

**File:** `src/protocol/edit.rs`, lines 838-913

**What it does wrong:** The batch_edit function sorts edits in reverse byte offset order within each file (phase 2, lines 790-797), which is correct for non-overlapping edits. However, for operations like `InsertBefore`, `InsertAfter`, and `Delete`, the helper functions (`build_insert_before`, `build_insert_after`, `build_delete`) operate on the ENTIRE file content, not just a splice range. They compute line starts, indentation, and blank line collapsing by scanning the full file.

When multiple edits target the same file, the code at line 856 does:
```rust
content = apply_splice(&content, (line_start, r.sym.byte_range.1), &indented);
```

After this splice, the `content` buffer has changed size, but subsequent edits still use the original `resolved[ri].sym.byte_range` values that were captured from the pre-edit index snapshot. For reverse-offset-ordered non-overlapping edits, the basic splice ranges still work because earlier (higher offset) edits don't shift lower offsets. BUT:

For `InsertBefore` (line 866):
```rust
content = build_insert_before(&content, &r.sym, code, line_ending);
```

`build_insert_before` internally computes `line_start` from `sym.effective_start()` by scanning backward in the content. After a previous edit changed the content, the `sym.byte_range` is stale and may point to wrong content. The function then scans `content[..sym_start]` for the preceding newline -- but `sym_start` is from the original index, not the updated content.

Wait -- since edits are applied in reverse offset order, a higher-offset edit changing the content doesn't affect the byte positions of lower-offset symbols. So the byte ranges remain valid for subsequent (lower-offset) edits. This is actually correct for reverse-offset processing.

**Revised assessment:** Upon closer examination, the reverse-offset ordering ensures that earlier (higher-offset) splices don't shift the byte positions of later (lower-offset) edits. This is the standard approach. NOT A BUG.

---

## CONFIRMED BUGS SUMMARY

| # | File | Severity | Description |
|---|------|----------|-------------|
| 1 | tools.rs:3781 | Medium | `edit_within_symbol` handler uses `byte_range.0` instead of `effective_start()`, inconsistent with `build_edit_within` and `batch_edit` |
| 2 | tools.rs:3790 | Medium | `edit_within_symbol` does not normalize `old_text` line endings -- fails silently on CRLF files |
| 3 | format.rs:293 | Medium | `is_noise_line` treats all `#`-prefixed lines as noise, hiding Rust attributes like `#[derive]`, `#[test]` |
| 4 | format.rs:295 | Low-Medium | `is_noise_line` treats all `*`-prefixed lines as noise, hiding dereference/glob/CSS lines |
| 5 | format.rs:297 | Low-Medium | `is_noise_line` uses `line.contains("require(")` instead of `starts_with`, catching mid-line occurrences |
| 6 | tools.rs:3511,3572 | Low | `replace_symbol_body` reports raw input length instead of actual indented+normalized byte count |
