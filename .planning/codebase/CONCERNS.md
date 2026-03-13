# Codebase Concerns

**Analysis Date:** 2026-03-14

## Lock Poisoning

**RwLock and Mutex unwrap calls throughout codebase:**
- Issue: Multiple `.expect("lock poisoned")` calls in `src/live_index/store.rs` (lines 378, 385, 391, 397, 403, 409, 418, 424, 429) and `src/protocol/mod.rs` (lines 108, 112) will panic if a lock holder panics while holding the lock
- Files: `src/live_index/store.rs`, `src/protocol/mod.rs`
- Impact: Any panic in a lock-guarded section will cause the entire server to crash on the next attempted lock acquisition. This is a severe availability risk in production.
- Fix approach: Implement poisoned-lock recovery handlers that gracefully degrade to a degraded state rather than panicking. Consider using custom lock wrappers that reset poison state on recovery. For non-critical locks, consider using `unwrap_or_else()` with state reset logic instead of immediate panic.

## Git Temporal Computation Unbounded Memory

**Git history processing on blocking thread:**
- Issue: `src/live_index/git_temporal.rs` loads commits with bounds (MAX_COMMITS=500, WINDOW_DAYS=90) but then builds HashMaps for every file touched (lines 314-348). For large repositories with many files, the intermediate data structures (`file_commit_indices`, `file_authors`, `file_raw_churn`, `file_last_commit_idx`) accumulate memory linearly with file count × commit count.
- Files: `src/live_index/git_temporal.rs`, lines 314-348 (Phase 1 aggregation)
- Impact: On very large monorepos (10k+ files), computing git temporal metrics could consume significant memory (potentially hundreds of MB) during the background computation window. This doesn't block the server but reduces available system memory.
- Fix approach: Implement streaming aggregation with periodic spill-to-disk if total file count exceeds a threshold (e.g., 5000 files). Alternatively, cap the analysis to the most-modified N files (e.g., top 1000 by churn).

## Snapshot Deserialization Trust

**Postcard binary snapshot loaded without validation:**
- Issue: `src/live_index/persist.rs` deserializes binary snapshots directly without version check or integrity verification (only version number is checked, lines 21). Malformed or corrupted `.tokenizor/index.bin` files can cause panics or incorrect parse states.
- Files: `src/live_index/persist.rs`, lines 94-150 (restore logic)
- Impact: A corrupted or hand-edited snapshot file will cause the index to panic on deserialization, blocking startup. No graceful degradation to empty state.
- Fix approach: Wrap `postcard::from_bytes()` in a try-catch that validates snapshot structure (at least 100 bytes, correct version). On error, log and fall back to empty index. Add checksums to snapshot format in the next version.

## File Watcher Burst Detection Edge Cases

**Burst tracker reset timing:**
- Issue: `src/watcher/mod.rs` lines 87-119: The burst tracker uses `QUIET_SECS=5` to reset the window, but if events are spaced exactly at the boundary (4.99s apart), the tracker can extend the debounce window indefinitely by never reaching the quiet threshold, causing unnecessary delays in index updates.
- Files: `src/watcher/mod.rs`, lines 108-118 (effective_debounce_ms)
- Impact: Edge case where rapid events separated by just under 5 seconds cause the debounce to perpetually stay at 500ms even during natural lulls, potentially delaying file updates by up to 500ms unnecessarily.
- Fix approach: Track absolute time of debounce window start instead of relying on elapsed time between events. Reset unconditionally after 30s of any window, not just quiet periods.

## Cross-Reference Extraction Language Coverage

**Missing language implementations:**
- Issue: `src/parsing/xref.rs` implements cross-reference extraction (xref queries) for only 5 languages: Rust, Python, JavaScript, TypeScript, and Go. Other supported languages (C, C++, C#, Java, Ruby, PHP, Swift, Perl, Kotlin, Dart, Elixir) fall back to no-op extraction, returning empty reference lists.
- Files: `src/parsing/xref.rs`, lines 13-85 (query definitions)
- Impact: Tools like `find_dependents` and co-change analysis will not work for projects in unsupported languages. Users will see empty results without clear indication that xref is unavailable.
- Fix approach: Add xref queries for high-value languages (Java, C++, C# at minimum). For unsupported languages, surface a clear message in query results: "Cross-references not available for [Language]". Consider marking language support in tool output.

## Term Search Regex Validation

**No validation on user-provided regex patterns:**
- Issue: `src/protocol/tools.rs` search_text handler accepts `regex` parameter (line 746-768) but does not pre-validate the regex pattern before passing to `search_text_result_with_options()`. Invalid regex (e.g., `"(?P<incomplete"`) will cause tree-sitter regex compilation to fail, returning an error string to the user instead of structured error.
- Files: `src/protocol/tools.rs`, lines 746-768
- Impact: User receives unhelpful error message. Malicious input cannot cause code execution but makes the tool appear fragile.
- Fix approach: Validate regex with `regex::Regex::new()` before passing to search. Return structured error with the regex error message highlighted.

## Concurrent File Modification Race

**File content cache without file-watch debounce coordination:**
- Issue: `src/live_index/store.rs` and `src/protocol/edit.rs` work with file content that is cached in memory. If a file is modified externally (e.g., by another process) while the watcher is debouncing events (up to 500ms), the in-memory content can become stale. Edits computed against this stale content will apply to the wrong byte ranges.
- Files: `src/protocol/edit.rs` (lines 44-53, reindex_after_write), `src/watcher/mod.rs` (debounce logic)
- Impact: Race condition where external file modifications that occur during the debounce window cause edits to apply at wrong positions. Very rare in practice but possible in CI/shared-editor scenarios.
- Fix approach: Before applying any edit, re-read the file from disk and verify content_hash matches the cached version. If mismatch, return error asking user to retry after sync. Consider reducing debounce window for edit operations.

## ParseStatus Not Propagated in Errors

**Partial parse warnings lost in some workflows:**
- Issue: Files with `ParseStatus::PartialParse` are indexed successfully with symbols extracted, but tools don't consistently surface the parse warning. `search_symbols` results include parse status but `symbol_context` results don't, leading to inconsistent user experience.
- Files: `src/protocol/tools.rs` (symbol_context handler, line ~2100-2200 estimated), `src/protocol/format.rs`
- Impact: Users may not realize they're working with incomplete symbol data for files with syntax errors.
- Fix approach: Ensure all symbol-returning tools include parse_status in output. Add a "⚠️ This file has parsing warnings" banner to context output.

## Tight Loop in Trigram Index

**Trigram generation unbounded for large files:**
- Issue: `src/live_index/trigram.rs` generates all 3-character substrings from file content without any length limit. For very large files (e.g., 10MB minified bundle), this creates millions of trigram entries, consuming O(file_size) memory and slowing down trigram-based queries.
- Files: `src/live_index/trigram.rs`, entire module
- Impact: Indexing very large files (>1MB) becomes noticeably slower. No functional correctness issue but performance degrades.
- Fix approach: Skip trigram indexing for files over a threshold (e.g., 500KB). Mark such files as "not text search enabled" and fall back to full-file scan for those.

## Daemon Degradation State Persistence

**Once-degraded daemon never attempts reconnection:**
- Issue: `src/protocol/mod.rs` lines 138-139: Once `daemon_degraded` flag is set to `true` after reconnect failure, all subsequent tool calls bypass the daemon entirely and fall back to local execution. The flag is never reset during the session.
- Files: `src/protocol/mod.rs`, lines 138-139, 174-175, 195-196
- Impact: If daemon has a transient failure and recovers, the local client will never know and will continue using potentially stale local index. Only a full process restart will re-enable daemon access.
- Fix approach: Implement exponential backoff reconnect attempts every N minutes (e.g., 5 min with jitter). Reset the `daemon_degraded` flag on successful reconnect attempt, not just on initial failure.

## Index Reload During Active Queries

**No guard preventing queries during reload:**
- Issue: `SharedIndexHandle::reload()` acquires a write lock to rebuild the entire index. If a query tool acquires a read lock during this brief window, it could see a partially-built state, or the reload could block waiting for reader completion, causing tool latency spikes.
- Files: `src/live_index/store.rs`, line 353 (write method)
- Impact: Rare race condition: reload-triggered queries may see old index state or experience 100-500ms latency spikes during reload. No data corruption but user-visible jank.
- Fix approach: Implement a "publishing" pattern where reload builds into a shadow index, then atomically swaps `live` to point to the new data. This allows readers to continue using the old index during rebuild.

## Test Coverage of Error Paths

**Limited error handling tests:**
- Issue: Codebase has strong test coverage for happy paths but minimal coverage of error scenarios. For example, no tests for parse failures, missing file handling, lock poisoning recovery, or git temporal computation failures.
- Files: `tests/` directory (all test files)
- Impact: Bugs in error handling paths go undetected and surface in production. Error messages may be unhelpful or incorrect.
- Fix approach: Add error scenario tests for each major module: parsing failures, file-not-found, corrupted snapshots, git command failures, and lock poisoning.

## Unbounded Result Sets in Search

**No pagination or result limiting in some search paths:**
- Issue: `search_text` tool has `result_limit` parameter capped at 100, but the actual matching logic in `src/live_index/search.rs` may still process thousands of candidate files before applying the limit. For very broad patterns (e.g., `.` or `[a-z]`), this could cause query latency to spike to multiple seconds.
- Files: `src/live_index/search.rs`, lines 1000-1200 (matching logic)
- Impact: Pathological regex patterns can cause tool timeout or 10+ second response latency, blocking the user.
- Fix approach: Add a hard cutoff in the search loop: if matches exceed result_limit × 10, stop scanning further files and return early with a "truncated due to too many matches" message.

## Dependency Version Pinning

**Loose version constraints on critical deps:**
- Issue: Cargo.toml specifies versions like `tokio = "1.48"`, `axum = "0.8"`, `rayon = "1.10"` without patch pinning. Semver-minor version bumps could introduce breaking changes or behavioral shifts.
- Files: `Cargo.toml`, lines 6-46
- Impact: Very low risk (semver should prevent breakage) but CI/binary reproducibility could vary across builds. No security known risk but drift is possible.
- Fix approach: Use `cargo update --aggressive` in CI to validate all patches work. Consider pinning to full versions (e.g., `1.48.0`) in production releases.

## Performance Bottleneck: Reverse Index Rebuilds

**Full reverse index rebuilt on every file mutation:**
- Issue: Every time a file is updated, added, or removed, the entire reverse index (HashMap<String, Vec<ReferenceLocation>>) is rebuilt from scratch in `src/live_index/store.rs` (lines ~390-410). For large indices (10k+ files), this O(N) operation runs synchronously.
- Files: `src/live_index/store.rs`, lines 391, 397, 403
- Impact: File edits via `index_folder` tool (which triggers `update_file`) cause 50-200ms latency on large repos during the rebuild. Accumulates if multiple files are edited in sequence.
- Fix approach: Implement incremental reverse index updates: instead of rebuilding, only remove old reference entries for the changed file and insert new ones. This reduces O(N) to O(file_references).

---

*Concerns audit: 2026-03-14*
