# Parsing Subsystem Bug Audit

Date: 2026-03-20
Scope: All 28 files in `src/parsing/`

## Genuine Bugs Found

### Bug 1: Markdown byte_offset overcount for files without trailing newline
**File:** `src/parsing/config_extractors/markdown.rs`, line 70
**Code:**
```rust
byte_offset += raw[i].len() + 1;
```
**What's wrong:** `text.split('\n')` is used to split lines. The `+1` accounts for the `\n` delimiter. But the *last* line in a file with no trailing newline has no `\n` after it, so the `byte_offset` is overcounted by 1 for the last element. While this doesn't affect `lines` entries (only the post-loop `byte_offset` is wrong), it would matter if code ever used `byte_offset` after the loop -- and right now it doesn't. **However**, in the frontmatter section (lines 46-58), the same `+1` pattern is applied. If the frontmatter lacks a trailing newline (e.g., `"---\ntitle: Hello\n---"` where the closing `---` is the last line), the `byte_offset` will be `20` (3+1+13+1+3+1=22, overcounted by 1) vs the actual content length of 21 bytes. This causes all subsequent header `byte_start` values to be off by +1.

**Severity:** Low-medium. Only affects files where frontmatter exists and the file has no trailing newline after the closing `---`. All byte ranges for headers would be shifted by +1.

**Fix:** Use the actual split delimiter length: for the last element in `split('\n')`, don't add +1.

---

### Bug 2: TOML `find_key_value_bytes` searches by raw key but doesn't advance cursor per-key
**File:** `src/parsing/config_extractors/toml_ext.rs`, lines 367-388
**Code:**
```rust
fn find_key_value_bytes(raw: &[u8], key: &str) -> (usize, usize) {
    // ...scans from byte 0 every time...
    let mut i = 0;
```
**What's wrong:** Unlike the JSON extractor which passes `search_from` to advance a cursor, `find_key_value_bytes` always starts scanning from byte 0. If two different TOML sections have keys with the same name (e.g., `[package]` has `name` and `[dependencies]` also has `name`), it will always find the *first* occurrence, producing wrong byte ranges for the second.

**Severity:** Medium. Produces wrong byte ranges for duplicate key names across different TOML sections. This is the `walk_item` code path (line 130), called from `walk_table` (line 103), where each table iterates its own keys but `find_key_value_bytes` has no cursor state.

**Fix:** Thread a `&mut usize` search cursor through `walk_table` -> `walk_item` -> `find_key_value_bytes`, similar to how `find_key_value_range` works in the JSON extractor.

---

### Bug 3: TOML `find_table_header_bytes` uses `key_path` (dot-joined) to search for `[section]`, incorrect for nested sections
**File:** `src/parsing/config_extractors/toml_ext.rs`, lines 390-392
**Code:**
```rust
fn find_table_header_bytes(raw: &[u8], key_path: &str) -> (usize, usize) {
    find_header_pattern(raw, &format!("[{}]", key_path))
}
```
**What's wrong:** When called for a nested table like `dependencies.serde`, it searches for `[dependencies.serde]` which is correct. But `key_path` goes through `join_key_path` which escapes special characters with `~` encoding (dots become `~1`, etc.). If a TOML key contains a literal dot, the escaped path like `a~1b` would be searched as `[a~1b]` in the raw file, but the actual header in the file would be `["a.b"]` or `[a."b"]`. This means the search would fail and fall back to `(0, 0)`.

**Severity:** Low. Only affects TOML keys that contain literal dots (uncommon). The `(0, 0)` fallback produces wrong byte ranges rather than a crash.

---

### Bug 4: TOML `find_key_value_bytes` doesn't handle quoted TOML keys
**File:** `src/parsing/config_extractors/toml_ext.rs`, lines 447-459
**Code:**
```rust
fn line_starts_with_key(line: &[u8], key: &[u8]) -> bool {
    if line.len() < key.len() { return false; }
    if !line.starts_with(key) { return false; }
    // ...
}
```
**What's wrong:** TOML allows quoted keys: `"my.key" = "value"`. The `walk_table` function receives the raw key from `toml_edit` (which is `my.key` unquoted), but the raw bytes in the file have `"my.key"`. The `line_starts_with_key` function checks if the line starts with `my.key` (unquoted), which won't match `"my.key"`. This produces `(0, 0)` byte ranges.

**Severity:** Low-medium. Affects any TOML file using quoted keys. Byte ranges default to `(0, 0)` instead of the correct position.

---

### Bug 5: JSON `find_key_value_range` doesn't handle escaped quotes in key names
**File:** `src/parsing/config_extractors/json.rs`, lines 304-338
**Code:**
```rust
let needle = format!("\"{}\"", key);
```
**What's wrong:** If a JSON key contains a character that would be escaped in JSON (e.g., a backslash or quote), the `needle` is built from the *unescaped* serde key string, but the raw bytes contain the *escaped* version. For example, a key `foo\"bar` in serde would be `foo"bar`, but the raw JSON has `"foo\"bar"`. The needle would be `"foo"bar"` which won't match. This produces `(0, content.len())` fallback ranges.

**Severity:** Low. Only affects JSON keys containing characters that need escaping (very uncommon in practice).

---

### Bug 6: `scan_doc_range` skips doc-prefix check for custom_doc_check nodes
**File:** `src/parsing/languages/mod.rs`, lines 86-98
**Code:**
```rust
if is_comment_node {
    if let Some(prefixes) = spec.doc_prefixes {
        // ... prefix check ...
    }
}
```
**What's wrong:** When a sibling matches via `custom_doc_check` (i.e., `is_custom_doc == true`) but is NOT a comment node (`is_comment_node == false`), the doc prefix check is skipped entirely. This is actually *correct* for the current Elixir case (where `@doc` is an attribute, not a comment). However, the blank-line check on lines 74-83 works correctly for both paths.

**Verdict:** NOT a bug. The code is correct -- custom doc nodes bypass the prefix check by design.

---

### Bug 7: `env.rs` line_starts index out of bounds when file has no trailing newline and ends with a comment-only last "line"
**File:** `src/parsing/config_extractors/env.rs`, line 59
**Code:**
```rust
let line_start = line_starts[line_idx];
```
**What's wrong:** `line_starts` is built from `\n` bytes in `content`. `text.lines()` enumerates lines. If the file is `"A=1\n# comment"` (no trailing newline), `line_starts = [0, 4]` (2 entries). `text.lines()` yields `["A=1", "# comment"]` (2 items, indices 0 and 1). `line_starts[0]` and `line_starts[1]` both exist. This works correctly.

But consider: `"A=1\n\n\nB=2"`. `line_starts = [0, 4, 5, 6]` (4 entries). `text.lines()` -> `["A=1", "", "", "B=2"]` (4 items). `line_starts[3] = 6`. Correct. This works.

**Verdict:** NOT a bug after careful analysis. `text.lines()` and the `\n`-based `line_starts` are aligned in count.

---

### Bug 8: Kotlin `walk_node` classification checks child TEXT content, not node KIND
**File:** `src/parsing/languages/kotlin.rs`, lines 32-43
**Code:**
```rust
for child in node.children(&mut cursor) {
    match child.utf8_text(source.as_bytes()).unwrap_or("") {
        "enum" => {
            refined = SymbolKind::Enum;
            break;
        }
        "interface" => {
            refined = SymbolKind::Interface;
            break;
        }
        _ => {}
    }
}
```
**What's wrong:** This checks the *text content* of every child node, not its *kind*. For a class like `class EnumFactory { ... }`, the class body might contain identifiers or string literals with the text "enum" or "interface", potentially causing misclassification. However, the direct children of `class_declaration` are typically keyword tokens and the class name, not arbitrary body content -- the body is wrapped in a `class_body` node. So in practice, this matching is scanning keyword tokens which happen to have their text == their kind.

**Verdict:** Fragile but not currently buggy in practice. The tree-sitter-kotlin-sg grammar wraps the body in a `class_body` node, so body content doesn't leak to direct children. The keyword tokens `enum`, `interface`, `class` appear as direct children with their text matching. This works correctly but is brittle.

---

## Confirmed Bugs Summary

| # | File | Severity | Description |
|---|------|----------|-------------|
| 1 | `config_extractors/markdown.rs:70` | Low | Byte offset overcount by +1 for last line in frontmatter without trailing newline |
| 2 | `config_extractors/toml_ext.rs:370` | Medium | `find_key_value_bytes` always starts from byte 0, wrong ranges for duplicate key names across sections |
| 3 | `config_extractors/toml_ext.rs:390` | Low | `find_table_header_bytes` uses escaped key_path to search raw file; fails for keys with dots |
| 4 | `config_extractors/toml_ext.rs:447` | Low-Medium | `line_starts_with_key` doesn't handle quoted TOML keys |
| 5 | `config_extractors/json.rs:306` | Low | `find_key_value_range` needle doesn't handle JSON-escaped key names |

## Non-Bugs Investigated and Cleared

- `scan_doc_range` byte arithmetic: uses `usize` from tree-sitter consistently, casts to `u32` only when building result. No off-by-one.
- `env.rs` CRLF handling: `text.lines()` and `\n`-based line_starts are aligned.
- `push_symbol` sets `item_byte_range: Some(byte_range)` where `byte_range` is the tree-sitter node range. This is correct.
- All language extractors' `walk_children` depth logic is correct via `next_child_depth`.
- HTML angular text scanner's `offset` calculation correctly adds `+1` for the `\n` consumed by `split('\n')`.
- All tree-sitter query strings compile at init time via `OnceLock`; invalid queries would panic at first use, not silently produce wrong results.
