# SymForge Agentic MCP — Full Code Review

**Date:** 2026-03-15
**Scope:** Entire codebase (src/, tests/, npm/)
**Commit:** 29474c2 (main)

---

## Summary

| Severity | Count |
|----------|-------|
| Critical (security/data loss) | 2 |
| Important (bugs/correctness) | 12 |
| Low (code quality) | 3 |

---

## Critical Issues

### C1. Path traversal in `get_file_content` disk fallback
**File:** `src/protocol/tools.rs:2368-2387` | **Confidence: 95**

`params.0.path` is user-supplied and joined directly onto the repo root without normalization or containment check. A path like `../../etc/passwd` bypasses the index and reads arbitrary files.

**Fix:** Canonicalize both `root` and `full_path`, assert `full_path.starts_with(root)` before reading.

---

### C2. `install.js` shell injection via `execSync` with interpolated path
**File:** `npm/scripts/install.js:89` | **Confidence: 83**

```js
const output = execSyncFn(`"${binPath}" --version`, { ... });
```

`execSync` with a string argument runs through the shell. If `SYMFORGE_HOME` or the resolved `binPath` contains shell metacharacters (e.g. `$(evil)`), arbitrary commands execute. The launcher correctly uses `execFileSync` — the install script should too.

**Fix:** Replace with `execFileSyncFn(binPath, ["--version"], ...)`.

---

## Important Issues — Protocol Layer

### I1. Fixed temp filename in `atomic_write_file` — collision + crash leak
**File:** `src/protocol/edit.rs:32-37` | **Confidence: 80**

Uses a fixed `.SYMFORGE_tmp` extension. Concurrent edits to same-basename files can collide. Crash between `write` and `rename` leaves orphan temp files.

**Fix:** Append a random suffix or thread ID to the temp filename.

### I2. `batch_rename` applies splices without overlap validation
**File:** `src/protocol/edit.rs:873-876` | **Confidence: 82**

Ranges are sorted reverse and deduped, but `dedup()` only removes consecutive identical entries — overlapping ranges (e.g. `Foo::Foo`) silently corrupt output. `execute_batch_edit` has overlap checks; `execute_batch_rename` does not.

**Fix:** Add the same overlap validation from `execute_batch_edit` to `execute_batch_rename`.

### I3. `apply_indentation` silently strips `\r` from CRLF files
**File:** `src/protocol/edit.rs:129-144` | **Confidence: 80**

`text.lines()` in Rust strips both `\n` and `\r\n`, but the output only re-joins with `\n`. Windows-format files get their line endings mangled by surgical edits.

**Fix:** Detect the file's line ending style and preserve it.

---

## Important Issues — Infrastructure

### I4. Daemon only handles Ctrl+C, not SIGTERM
**File:** `src/daemon.rs:1020` | **Confidence: 85**

`run_daemon_until_shutdown` only awaits `ctrl_c()`. On Unix, `terminate_process` sends SIGTERM (kill -15), which isn't caught — daemon exits without cleanup, leaving stale `.port` and `.pid` files.

**Fix:** Race `ctrl_c()` against `unix::signal(SignalKind::terminate())` on Unix via `tokio::select!`.

### I5. `spawn_daemon_process` drops `Child` handle — zombie processes on Unix
**File:** `src/daemon.rs:803-806` | **Confidence: 82**

The `Child` handle is dropped without `wait()` or `forget()`. On Unix this creates zombie processes until the parent exits.

**Fix:** `std::mem::forget(child)` or use double-fork.

### I6. `open_project_session` acquires write lock twice — TOCTOU panic
**File:** `src/daemon.rs:195-224` | **Confidence: 80**

Two separate `projects.write()` acquisitions with a gap between them. A concurrent `close_session` between the two can evict the project, causing `.expect("project must exist")` to panic.

**Fix:** Merge into a single lock scope, or use `.ok_or_else()` instead of `.expect()`.

---

## Important Issues — Live Index

### I7. `usize` multiplication overflow in search
**File:** `src/live_index/search.rs:906` | **Confidence: 85**

```rust
total_limit: options.total_limit * normalized_terms.len(),
```

Caller-controlled `total_limit` multiplied by term count can overflow (panic in debug, silent wrap in release).

**Fix:** `options.total_limit.saturating_mul(normalized_terms.len())`

### I8. `linear_scan` allocates full lowercased copy of every file for short queries
**File:** `src/live_index/trigram.rs:158-163` | **Confidence: 83**

For queries < 3 bytes, every file's content is `.to_ascii_lowercase().collect()` — allocating and discarding O(total_bytes) per call.

**Fix:** Compare inline with `to_ascii_lowercase()` per byte using `windows()`, avoiding the allocation.

---

## Important Issues — Parsing

### I9. Python parser misses decorated and async functions
**File:** `src/parsing/languages/python.rs:19-23` | **Confidence: 88**

Only matches `"function_definition"` and `"class_definition"`. Tree-sitter-python produces `"decorated_definition"` for `@decorator` and may use different node kinds for `async def`. These are silently dropped from symbol outlines.

**Fix:** Handle `"decorated_definition"` (recurse into body) and verify async function node kinds.

### I10. JS/TS arrow functions indexed as `Constant` instead of `Function`
**File:** `src/parsing/languages/javascript.rs:51-75`, `typescript.rs:53-75` | **Confidence: 85**

`const handler = (req, res) => { ... }` is indexed as `Constant`. Arrow functions assigned to variables are the dominant export pattern in modern JS/TS — searching by `kind=fn` misses all of them.

**Fix:** Check if the variable declarator's initializer is an arrow/function expression; if so, emit `Function` kind.

### I11. YAML/JSON array elements have full-file byte ranges
**File:** `src/parsing/config_extractors/yaml.rs:295-317` | **Confidence: 90**

Every sequence element is emitted with `byte_start=0, byte_end=content.len()` — the entire file. Any feature using byte ranges to navigate to array elements shows the wrong location. The test only checks `byte_range.1 <= content.len()`, which trivially passes.

**Fix:** Compute actual byte offsets for each array element from the YAML event stream.

### I12. `find_yaml_key_range` advances cursor to wrong position
**File:** `src/parsing/config_extractors/yaml.rs:110-146` | **Confidence: 85**

After matching, `search_from` is set to `key_end` (after the colon) instead of after `range_end`. Sibling key searches start inside the previous value block, causing incorrect matches when key names are substrings of later content.

**Fix:** Set `*search_from = range_end` after a successful match.

---

## Low Severity

### L1. Duplicate query params silently dropped in `parse_resource_uri`
**File:** `src/protocol/resources.rs:319` | **Confidence: 80**

`url.query_pairs().collect::<HashMap>()` keeps only the last value for duplicate keys.

### L2. `toml_ext.rs` double-parses on failure path
**File:** `src/parsing/config_extractors/toml_ext.rs:56-72` | **Confidence: 80**

On parse failure, the TOML document is re-parsed just to inspect the error message.

### L3. Test `test_ts_builtin_type_filter` doesn't assert core invariant
**File:** `tests/xref_integration.rs:168-179` | **Confidence: 80**

Asserts `filtered.len() < 10` but never asserts `unfiltered.len() > filtered.len()` — test passes equally whether filtering works or xref is completely broken.

---

## Architectural Notes (not bugs, but worth awareness)

- **Python docstrings** (`python.rs:4`): Uses `NO_DOC_SPEC`, so Python triple-quote docstrings are never captured. This is a known limitation of the `DocCommentSpec` mechanism.
- **Angular text offset** (`html.rs:169-225`): `offset += line.len() + 1` overshoots by 1 for non-newline-terminated final lines in text nodes.
- **Sidecar port collision** (`sidecar/port_file.rs`): TCP-connect stale check would falsely accept a non-SymForge process reusing the port.

---

## Recommended Priority

1. **C1** — Path traversal. Security vulnerability. Fix immediately.
2. **C2** — Shell injection in install script. Fix before next npm publish.
3. **I9, I10** — Python/JS/TS parsing gaps affect core value proposition.
4. **I4, I5** — Daemon lifecycle issues on Unix.
5. **I11, I12** — YAML/JSON config parsing correctness.
6. **I2, I3** — Edit operation correctness (rename overlap, CRLF).
7. **I1, I6, I7, I8** — Robustness improvements.
