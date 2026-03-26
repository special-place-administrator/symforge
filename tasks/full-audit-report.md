# SymForge Full Codebase Audit Report

**Date:** 2026-03-20
**Scope:** All source files under `src/`
**Agents:** 4 parallel reviewers (protocol, live_index, daemon/sidecar/watcher, parsing/domain)

---

## Summary

| Module | Critical | Important | Minor | Total |
|--------|----------|-----------|-------|-------|
| Protocol (tools, edit, format, explore) | 4 | 7 | 5 | 16 |
| Live Index (search, query, store) | 2 | 5 | 3 | 10 |
| Daemon / Sidecar / Watcher | 4 | 10 | 3 | 17 |
| Parsing / Domain | 3 | 9 | 5 | 17 |
| **TOTAL** | **13** | **31** | **16** | **60** |

---

## CRITICAL (13)

### Protocol Module

| # | File | Issue | Confidence |
|---|------|-------|------------|
| P1 | tools.rs:3594 | `insert_symbol` capability check after symbol resolution (inconsistent with peers) | 95% |
| P2 | tools.rs:3776 | `edit_within_symbol` replace_all + dry_run=true + missing text returns false success | 92% |
| P3 | edit.rs:790 | `execute_batch_edit` TOCTOU: byte ranges from snapshot A applied to content snapshot B | 88% |
| P4 | edit.rs:19 | `safe_repo_path` requires path to exist on disk; undocumented; breaks pre-write validation | 85% |

### Live Index Module

| # | File | Issue | Confidence |
|---|------|-------|------------|
| L1 | store.rs:417 | Undocumented lock ordering across 4 RwLocks (live→pre_update→published_state→published_outline) | 95% |
| L2 | search.rs:404 | `NoiseClass::Ignored` maps to `include_vendor` flag instead of its own flag | 92% |

### Daemon / Sidecar / Watcher

| # | File | Issue | Confidence |
|---|------|-------|------------|
| D1 | daemon.rs:308 | Double write-lock in `open_project_session` — activation race window | 90% |
| D2 | daemon.rs:382 | `close_session` None-branch: project cleaned up but session removal can return None | 85% |
| D3 | daemon.rs:734 | `connect_or_spawn_session` uses bare `Client::new()` — no timeout (hangs indefinitely) | 95% |
| D4 | daemon.rs:820 | `daemon_health` uses bare `Client::new()` — no timeout | 95% |

### Parsing / Domain

| # | File | Issue | Confidence |
|---|------|-------|------------|
| X1 | config_extractors/json.rs:263 | JSON array element byte ranges are `(0, file_len)` placeholders | 100% |
| X2 | config_extractors/yaml.rs:316 | YAML array element byte ranges are `(0, file_len)` placeholders | 100% |
| X3 | xref.rs:637 | Language object discarded after OnceLock init — fragile if grammar versions diverge | 95% |

---

## IMPORTANT (31)

### Protocol Module

| # | File | Issue | Confidence |
|---|------|-------|------------|
| P5 | edit.rs:412 | `extend_past_orphaned_docs` CRLF byte offset sum is fragile | 82% |
| P6 | explore.rs:22 | Duplicate CONCEPT_MAP entries ("file watching"/"watcher", "parsing"/"parser") | 98% |
| P7 | tools.rs:1916 | `get_symbol_context` silently picks alphabetically first match when ambiguous | 87% |
| P8 | format.rs:1117 | `what_changed_timestamp_view` misleading "No changes" when index is empty | 82% |
| P9 | edit.rs:264 | `apply_indentation` loses trailing blank lines via `str::lines()` stripping | 85% |
| P10 | tools.rs:2851 | `find_references` impls mode `is_concrete` misses Enum and other non-trait kinds | 83% |
| P11 | tools.rs:719 | `parse_language_filter` returns Err for unknown language (inconsistent validation) | 83% |

### Live Index Module

| # | File | Issue | Confidence |
|---|------|-------|------------|
| L3 | store.rs:1088 | `build_reload_data` does not populate `skipped_files` — health stats wrong after reload | 88% |
| L4 | query.rs:1777 | `capture_context_bundle_view` returns entire file when byte range exceeds content len | 88% |
| L5 | query.rs:1056 | `capture_shared_file_for_scope` with Prefix silently returns None for multiple matches | 85% |
| L6 | search.rs:1109 | `compute_importance_score` hardcoded normalizer of 20 for caller count | 82% |
| L7 | search.rs:1144 | `collect_text_matches` duplicates test_ranges computation between passes | 82% |

### Daemon / Sidecar / Watcher

| # | File | Issue | Confidence |
|---|------|-------|------------|
| D5 | daemon.rs:355 | Lock ordering inconsistency in `close_session` (projects write → sessions write) | 88% |
| D6 | daemon.rs:336 | Atomic mutation through read guard — intentional but undocumented | 92% |
| D7 | handlers.rs:584 | `handle_edit_impact` blocking disk I/O + tree-sitter parse without `spawn_blocking` | 90% |
| D8 | handlers.rs:467 | `handle_new_file_impact` same blocking I/O problem | 90% |
| D9 | server.rs:55 | `spawn_sidecar` uses `current_dir()` as repo root — wrong if run from subdirectory | 88% |
| D10 | watcher/mod.rs:301 | mtime=0 treated as always-stale, causes repeated re-index on stat failure | 85% |
| D11 | daemon.rs:131 | Truncating `as u64` cast from `u128` millis (safe but non-idiomatic) | 85% |
| D12 | cli/init.rs:42 | Dead `from_home` function silenced with `#[allow(dead_code)]` | 88% |
| D13 | daemon.rs:380 | Stale `project_id` returned from close_session None branch | 83% |
| D14 | sidecar/mod.rs:186 | `build_with_budget` misses truncation suffix when first item exceeds budget | 85% |

### Parsing / Domain

| # | File | Issue | Confidence |
|---|------|-------|------------|
| X4 | config_extractors/json.rs:317 | `find_key_value_range` naive substring match can match keys inside string values | 90% |
| X5 | languages/dart.rs:20 | `function_signature` may not capture top-level Dart functions; no test for `main()` | 85% |
| X6 | languages/kotlin.rs:25 | Kotlin interface and enum silently mapped to `Class` | 85% |
| X7 | languages/swift.rs:25 | Missing `extension_declaration` — Swift extensions not indexed at all | 83% |
| X8 | languages/c.rs:5 | DOC_SPEC has Rust-specific prefixes (`//!`) that are not valid C comments | 83% |
| X9 | languages/ruby.rs:26 | `method` mapped to Function, `singleton_method` to Method — inverted | 82% |
| X10 | xref.rs:510 | `push_import_reference` JS/TS path splitting uses `::` then `.` — wrong for file paths | 82% |
| X11 | languages/elixir.rs:11 | `&str` byte-index panics on non-ASCII source before `@doc` attribute | 88% |
| X12 | Multiple files | `build_line_starts` / `byte_to_line` duplicated identically in json.rs, toml_ext.rs, yaml.rs | 85% |

---

## MINOR (16)

### Protocol Module

| # | File | Issue |
|---|------|-------|
| P12 | edit_format.rs:50 | Emoji `⚠` in `format_stale_warnings` output |
| P13 | format.rs:155 | `render_symbol_detail` out-of-bounds fallback dumps entire file |
| P14 | format.rs:845 | Dead code `repo_outline_display_labels` suppressed with `#[allow(dead_code)]` |
| P15 | edit.rs:1752 | `find_qualified_usages` single-quoted strings not handled; false matches in JS/TS |
| P16 | tools.rs:3574 | `insert_symbol` position validation before proxy call (inconsistent ordering) |

### Live Index Module

| # | File | Issue |
|---|------|-------|
| L8 | query.rs:201 | `parse_declared_scope` splits on `//` before checking if inside string literal |
| L9 | query.rs:2556 | `find_dependents_for_file` BFS doesn't dedup transitive hits against initial results |
| L10 | query.rs:145 | `is_pub_use_import` lookback boundary check has redundant condition |

### Daemon / Sidecar / Watcher

| # | File | Issue |
|---|------|-------|
| D15 | handlers.rs:910 | `repo_map_text` acquires index read lock twice unnecessarily |
| D16 | watcher/mod.rs:437 | Burst tracker map not evicted during overflow-only reconciliation |
| D17 | handlers.rs:871 | Misleading "use file to narrow" hint when file filter already active |

### Parsing / Domain

| # | File | Issue |
|---|------|-------|
| X13 | domain/index.rs | `optional_u32` returns None for value 0 — undocumented convention |
| X14 | languages/cpp.rs:31 | `template_declaration` match arm is dead code (both arms return None) |
| X15 | languages/go.rs | No tests for struct, interface, const, var extraction |
| X16 | languages/perl.rs:24 | `package_statement` to Module mapping is untested |
| X17 | languages/html.rs:225 | `scan_angular_text` offset calculation can overflow for huge text nodes |

---

## Priority Fix Order (Recommended)

### Wave 1 — Safety & Correctness (12 items)
1. D3+D4: Add timeout to bare `reqwest::Client::new()` calls (daemon.rs)
2. X11: Elixir `&str` byte-index panic on non-ASCII (elixir.rs)
3. P2: `edit_within_symbol` replace_all dry_run false success (tools.rs)
4. P3: `execute_batch_edit` TOCTOU byte range mismatch (edit.rs)
5. X1+X2: JSON/YAML array element placeholder byte ranges (json.rs, yaml.rs)
6. D1: Double write-lock activation race in `open_project_session` (daemon.rs)
7. D7+D8: Blocking I/O in async handlers without spawn_blocking (handlers.rs)
8. X9: Ruby method/function kind inversion (ruby.rs)
9. L1: Document lock ordering (store.rs)
10. P1: Move `insert_symbol` capability check before resolution (tools.rs)

### Wave 2 — Reliability & Consistency (10 items)
11. D2+D13: close_session None-branch fixes (daemon.rs)
12. L4+P13: Clamp byte range instead of returning entire file (query.rs, format.rs)
13. X7: Add Swift `extension_declaration` support (swift.rs)
14. X8: Fix C DOC_SPEC prefixes (c.rs)
15. X10: Fix JS/TS import path splitting in xref (xref.rs)
16. P6: Remove duplicate CONCEPT_MAP entries (explore.rs)
17. L2: Add `include_ignored` flag to NoisePolicy (search.rs)
18. D9: Pass repo root explicitly to spawn_sidecar (server.rs)
19. P10: Expand `is_concrete` to include Enum (tools.rs)
20. D10: Handle mtime=0 == indexed_mtime=0 as "skip" (watcher/mod.rs)

### Wave 3 — Polish & Cleanup (remaining)
21-60: Remaining important and minor items from tables above
