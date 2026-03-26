# SymForge Codebase Review — 2026-03-20

5 parallel review agents, 98 Rust source files analyzed, ~400K tokens consumed.

---

## CRITICAL (2) — Silent wrong behavior, data invisible to users

### C1. TypeScript `namespace`/`module` declarations never extracted
**File:** `src/parsing/languages/typescript.rs:26-39`
**Category:** missing-kind | **Confidence:** 95

The tree-sitter TS grammar emits `module` and `internal_module` nodes for `namespace Foo {}` / `module Foo {}`. Neither parser matches them. `SymbolKind::Module` exists in the domain but is never produced. Any TypeScript namespace is completely invisible to the indexer.

**Fix:** Add `"module" | "internal_module"` → `Some(SymbolKind::Module)` to `walk_node`.

---

### C2. TypeScript `abstract class` declarations silently dropped
**File:** `src/parsing/languages/typescript.rs:26-39`
**Category:** silent-failure | **Confidence:** 95

`abstract class Foo {}` parses as `abstract_class_declaration`, not `class_declaration`. The match arm only covers `"class_declaration"`. Abstract classes produce zero symbol records.

**Fix:** Add `"abstract_class_declaration"` → `Some(SymbolKind::Class)` to the match.

---

## HIGH (10) — Bugs that cause wrong behavior, data loss, or security issues

### H1. `mtime_secs` type mismatch: `i64` in snapshot vs `u64` in runtime
**File:** `src/live_index/persist.rs:55,184` vs `src/live_index/store.rs:54`
**Category:** correctness | **Confidence:** 88

`IndexedFileSnapshot::mtime_secs` is `i64`, `IndexedFile::mtime_secs` is `u64`. The `as u64` cast at persist.rs:184 silently wraps negative values to huge u64, making the freshness guard permanently treat stale files as fresh — the watcher will never re-index them.

**Fix:** Unify to `u64` throughout, or use `snap_file.mtime_secs.max(0) as u64`.

---

### H2. IPv6 bind host silently deletes live sidecar port files
**File:** `src/sidecar/port_file.rs:101-103`
**Category:** panic / data-loss | **Confidence:** 90

When `bind_host` is IPv6 (e.g. `"::1"`), `addr.parse()` fails (missing brackets). The fallback substitutes `127.0.0.1:0` — port 0 connect always fails, so `check_stale` returns `true` and calls `cleanup_files()` on a live sidecar.

**Fix:** `let addr_str = if bind_host.contains(':') { format!("[{bind_host}]:{port}") } else { format!("{bind_host}:{port}") };`

---

### H3. TOCTOU: content read and mtime stat are not atomic
**File:** `src/watcher/mod.rs:235-240`
**Category:** race-condition | **Confidence:** 82

File bytes are read first, then `std::fs::metadata` is called separately to get mtime. If the file is written between these calls, the index stores a newer mtime with older content. The freshness guard then considers the file "fresh" permanently until the next write.

**Fix:** Read metadata before content, or use a single `File` handle for both stat and read.

---

### H4. `index_folder` accepts arbitrary absolute paths — no containment check
**File:** `src/protocol/tools.rs:2560-2598`
**Category:** security | **Confidence:** 88

Unlike file-read operations that go through `edit::safe_repo_path`, `index_folder` passes the `path` parameter directly to `self.index.reload()` with no validation. A malicious MCP client can re-root the entire index to any filesystem location (e.g., `/etc`, `C:\Windows`).

**Fix:** Validate the path exists as a directory. Consider an `allowed_roots` allowlist for untrusted clients.

---

### H5. `safe_repo_path` fails open for non-existent paths
**File:** `src/protocol/edit.rs:21-33`
**Category:** security / correctness | **Confidence:** 82

`canonicalize()` requires the path to exist on disk. For non-existent paths, it returns an error that callers treat as "not found" — indistinguishable from a legitimate missing file. The path traversal guard is non-functional for non-existent targets.

**Fix:** Add a lexical containment check before calling `canonicalize`:
```rust
let normalized = full_path.components().collect::<PathBuf>();
if !normalized.starts_with(repo_root) { return Err(...); }
```

---

### H6. `export default` arrow/function expression produces no symbol
**File:** `src/parsing/languages/typescript.rs:33`, `javascript.rs:31`
**Category:** incorrect-extraction | **Confidence:** 85

`export_statement` passes `kind = None` and recurses. For `export default (x) => x * 2`, the arrow function is not a `variable_declarator`, so nothing is extracted. Default-exported anonymous functions are invisible.

**Fix:** When the child of `export_statement` is a direct arrow/function expression (not a declaration), emit a symbol named `"default"` with `SymbolKind::Function`.

---

### H7. Class `field_definition` / static fields silently skipped
**File:** `src/parsing/languages/typescript.rs:26-39`, `javascript.rs:26-36`
**Category:** missing-kind | **Confidence:** 85

`walk_node` only matches `"method_definition"` inside class bodies. `"field_definition"`, `"public_field_definition"`, and `"static_block"` fall to `_ => None` and are silently dropped.

**Fix:** Add match arms for `"field_definition" | "public_field_definition"` → extract name as `SymbolKind::Variable` or `SymbolKind::Constant`.

---

### H8. Angular control-flow symbols all share the same span
**File:** `src/parsing/languages/html.rs:183-219`
**Category:** incorrect-extraction | **Confidence:** 90

In `scan_angular_text`, every `push_symbol` call passes `node` (the enclosing text node) as the node argument. All `@if`, `@for`, `@let` found within the same text node report identical `byte_range` and `line_range` — the full extent of the text node, not the individual construct.

**Fix:** Add a `push_symbol_at_bytes` variant that accepts explicit `(start_byte, end_byte)` instead of reading from `node`.

---

### H9. CSS: `@import`, `@layer`, `@container` produce no symbols
**File:** `src/parsing/languages/css.rs:104-108`
**Category:** missing-kind | **Confidence:** 85

The CSS `walk_node` only matches `rule_set`, `declaration`, `media_statement`, and `keyframes_statement`. `@import` (`import_statement`), `@layer`, and `@container` hit the `_` wildcard, recurse into children with no symbol-bearing nodes, and are silently dropped.

**Fix:** Add explicit match arms for `import_statement`, `layer_statement`, `container_query`.

---

### H10. Multi-lock acquisition order in `update_file` lacks enforcement
**File:** `src/live_index/store.rs:430-448`
**Category:** race-condition | **Confidence:** 85

`update_file` acquires `live.write()`, then `pre_update_symbols.write()`, then `published_state.write()` and `published_repo_outline.write()` via `publish_locked`. The ordering is consistent today but has no compile-time enforcement. Any future caller that acquires `published_state` before `live` will deadlock.

**Fix:** Document the full lock ordering on `publish_locked`. Consider a typed lock-guard wrapper to enforce ordering at compile time.

---

## MEDIUM (9) — Correctness gaps, dead code, inconsistencies

| # | File | Category | Description |
|---|------|----------|-------------|
| M1 | `typescript.rs` | angular-gap | Decorators break `scan_doc_range` — JSDoc not attached to decorated classes |
| M2 | `typescript.rs:73` / `javascript.rs:70` | incorrect-extraction | Multi-declarator `const a=1, b=2` assigns whole-statement byte span to each symbol |
| M3 | `typescript.rs` / `javascript.rs` | missing-kind | `generator_function_declaration` not matched at top level |
| M4 | `scss.rs:100,133` | scss-gap | SCSS control-flow `@each`/`@for`/`@while`/`@if` not emitted as symbols |
| M5 | `tools.rs:936` | error-handling | `fix_common_double_escapes` compiles regex per call via `.unwrap()` — should use `LazyLock` |
| M6 | `tools.rs:2173` | input-validation | `search_text` does no early validation when both `query` and `terms` are empty |
| M7 | `domain/index.rs:158` | dead-code | `SupportTier::Unsupported` variant declared but never constructed |
| M8 | `domain/index.rs:65` | dead-code | `LanguageId::extensions()` is public but never called |
| M9 | `domain/index.rs:242,246` | correctness | `FileClassification::is_text()`/`is_binary()` always return `false` — `FileClass::Text`/`Binary` never produced |

---

## LOW (4) — Cleanup, documentation, minor inconsistencies

| # | File | Category | Description |
|---|------|----------|-------------|
| L1 | `tools.rs:2517` | error-handling | `health` tool panics on poisoned mutex — use `.unwrap_or_else(\|e\| e.into_inner())` |
| L2 | `Cargo.toml:66` | dead-code | `v1` feature declared with zero `cfg(feature = "v1")` gates in source |
| L3 | `daemon.rs:209,213` | dead-code | `SessionRuntime` fields `project_name` and `watcher_info` stored but never read |
| L4 | `discovery/mod.rs:43` | dead-code | `discover_files` / `DiscoveredFile` superseded by `discover_all_files` but still exported |

---

## Fix Status (all applied, verified)

All fixes below compile cleanly (`cargo check` passes) and all 1,398 tests pass.

| ID | Fix Applied | File(s) Modified |
|----|------------|-----------------|
| C1 | Added `"module" \| "internal_module"` → Module | `typescript.rs` |
| C2 | Added `"abstract_class_declaration"` → Class | `typescript.rs` |
| H1 | Unified `mtime_secs` to `u64` throughout | `persist.rs`, `main.rs` |
| H2 | Bracket IPv6 addresses before SocketAddr parse | `port_file.rs` |
| H3 | Read metadata before content (TOCTOU fix) | `watcher/mod.rs` |
| H4 | Added exists + is_dir validation to `index_folder` | `tools.rs` |
| H5 | Added lexical parent-traversal check before canonicalize | `edit.rs` |
| H6 | — (requires design decision on `export default` naming) | — |
| H7 | Added `"public_field_definition" \| "field_definition"` → Variable | `typescript.rs`, `javascript.rs` |
| H8 | Construct SymbolRecord directly with per-line byte/line range | `html.rs` |
| H9 | Added `import_statement \| layer_statement \| container_query_statement` arms | `css.rs` |
| H10 | — (documentation-only, deferred) | — |
| M2 | Pass `&child` instead of `node` to fix multi-declarator spans | `typescript.rs`, `javascript.rs` |
| M3 | Added `"generator_function_declaration"` → Function | `typescript.rs`, `javascript.rs` |
| M4 | Added `extend_statement` to silenced list with comment | `scss.rs` |
| M5 | Replaced per-call regex with `LazyLock` static | `tools.rs` |
| L1 | Handle poisoned mutex with `unwrap_or_else` | `tools.rs` |
