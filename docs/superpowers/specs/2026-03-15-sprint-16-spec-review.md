# Sprint 16 Design Spec Review

**Reviewer:** Code Review Agent
**Date:** 2026-03-15
**Spec:** `docs/superpowers/specs/2026-03-15-sprint-16-correctness-lifecycle-design.md`
**Verdict:** Solid spec with 3 important gaps and 5 suggestions

---

## Overall Assessment

The spec is well-structured, internally consistent, and accurately describes the
existing code. Problem descriptions match the actual source (verified against
`src/daemon.rs`, `src/protocol/edit.rs`, `src/domain/index.rs`). Execution
ordering rationale is sound. No internal contradictions found between sections.

---

## Important Issues (Should Fix Before Implementation)

### I1. C6: `ProjectInstance::load` spawns watcher + git temporal inside constructor

The spec says (step 3): "call `ProjectInstance::load()` unlocked" and (step 5):
"If another thread inserted it, discard loaded instance, use existing."

But `load()` (daemon.rs:841-879) calls `start_project_watcher()` which spawns a
tokio task AND calls `spawn_git_temporal_computation()`. Discarding the losing
instance does not abort these spawned tasks. The current `ProjectInstance` has no
`Drop` impl that aborts the watcher task -- `abort_watcher_task` is only called
explicitly in `close_session`.

**Recommendation:** The spec's step 5 needs to explicitly require aborting the
watcher task and git temporal task on the discarded instance, OR restructure
`load()` to be a pure construction step that defers watcher/temporal spawn to
after the write-lock re-check (as hinted by the "Key invariants" bullet). The
second approach is cleaner and the spec already gestures at it but does not
mandate it. Make this explicit: split `load()` into `load()` (index only) and
`activate()` (watcher + temporal), calling `activate()` only on the winning
instance under the write lock.

### I2. C1: `build_delete` and `collapse_blank_lines` are CRLF-unaware

The spec lists affected functions for CRLF handling:
- `apply_indentation`
- `build_insert_before` / `build_insert_after`
- `replace_symbol_body` content assembly
- `edit_within_symbol` replacement text
- `batch_edit` / `batch_insert` per-file processing

Missing from that list:
- **`build_delete`** (edit.rs:230-303): scans for `b'\n'` when extending past
  trailing newlines (lines 290-299), misses `\r\n`. A CRLF file will leave
  orphan `\r` bytes after deletion.
- **`collapse_blank_lines`** (edit.rs:306-321): counts only `b'\n'` characters.
  In a CRLF file, `\r\n\r\n\r\n` is 6 bytes with 3 newlines, but `\r` breaks
  the consecutive-newline counter, so the collapse never triggers. This function
  needs to count `\r\n` pairs (or operate post-normalization).

**Recommendation:** Add `build_delete` and `collapse_blank_lines` to the
affected-functions list. For `collapse_blank_lines`, decide whether to normalize
first or make the counter CRLF-aware.

### I3. C3: `tempfile::NamedTempFile::persist()` on Windows does NOT atomically replace

The spec flags this: "Confirm `persist()` atomically replaces an existing target
on all supported platforms. If not, add platform-specific replace step."

This is a known issue. On Windows, `persist()` calls `std::fs::rename()` which
fails if the target exists. The `tempfile` crate provides
`NamedTempFile::persist_noclobber()` (which is the opposite of what's needed) but
the standard `persist()` uses `rename` which returns `ERROR_ALREADY_EXISTS` on
Windows.

The solution is `tempfile::NamedTempFile::persist()` combined with enabling the
Windows `FILE_RENAME_INFO` with `ReplaceIfExists`, OR simply calling
`std::fs::rename` after `std::fs::remove_file` on Windows, OR using the
`persist` method from the `tempfile` crate version 3.x which on Windows calls
`MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`.

**Recommendation:** Verify which version of `tempfile` is depended upon (the
`Cargo.toml` says `tempfile = "3"`) and confirm that the `persist()` path for
v3 uses `MoveFileExW` with replace semantics on Windows. If it does, document
this in the spec. If not, add an explicit `#[cfg(windows)]` fallback path.
Current `std::fs::rename` in `atomic_write_file` also fails on Windows if target
exists, but the current code works because the `with_extension` path is
different from the target -- confirm the new flow handles the already-exists
case.

---

## Suggestions (Nice to Have)

### S1. C6: Test coverage for watcher/temporal task leaks

The test table has "Load dedupe sanity: Instrument `load()` call count; allow
>=1, verify only 1 instance inserted." This is good but should also assert that
no watcher tasks are leaked (i.e., exactly 1 watcher task is alive per project
after concurrent opens resolve). Consider asserting on `watcher_task.is_some()`
count or instrumenting `start_project_watcher` with a counter.

### S2. C4: The `dedup()` call on tuples already does exact-dedup

The spec says (step 2): "Exact-dedup only (current behavior)." The current code
calls `ranges.dedup()` after `ranges.sort_by(|a, b| b.0.cmp(&a.0))`. Since
`(u32, u32)` implements `PartialEq`, `dedup()` already removes exact duplicates.
However, `sort_by` only sorts by `.0` descending -- two ranges with the same
start but different end would not be adjacent after sort unless end is also in
the sort key. The spec's step 1 says "Sort by `(start desc, end desc)`" which
fixes this, but call it out as a behavior change from current code which only
sorts by start.

### S3. C1: Missing test for `build_delete` on CRLF file

Related to I2. If `build_delete` and `collapse_blank_lines` are added to the
affected-functions list, add a corresponding test: "Delete symbol from CRLF file
-- no orphan `\r` bytes, blank line collapse works correctly."

### S4. C5: Windows `terminate_process` is not addressed

The spec's "terminate_process improvement" section focuses on Unix (use
`libc::kill` instead of shell-out). The Windows path (lines 1539-1544) shells
out to `taskkill /T /F` which sends a forceful kill, not a graceful termination.
Since C5 is about graceful shutdown, consider whether the Windows
`terminate_process` should also be improved (e.g., using
`GenerateConsoleCtrlEvent` for graceful shutdown). If out of scope, state it
explicitly.

### S5. C2-lite: `is_denylisted_extension` already lowercases

The spec says "Make extension comparison explicitly case-insensitive: Lowercase
the extracted extension once." Looking at the code (index.rs:528-529),
`is_denylisted_extension` already calls `.to_lowercase()` on the input. The
caller `classify_admission` (discovery/mod.rs:380) passes the raw
`OsStr::to_str()` result. So case-insensitivity is already handled. The spec
should acknowledge this is already implemented and state that the change is just
adding the 5 new extensions, not adding case-insensitivity (which is a no-op).

---

## Verified Correct

- C6 problem description: two separate write-lock scopes at lines 195-205 and
  213-224, `.expect("project must exist")` at line 217 -- all match.
- C3 problem description: deterministic temp name at line 53 -- matches.
- C1 problem description: `.lines()` stripping `\r\n` at line 152, hardcoded
  `\n` at lines 153-155 -- matches.
- C4 problem description: `.dedup()` at line 841, descending sort at line 840 --
  matches.
- C5 problem description: only `ctrl_c()` at line 1020, no SIGTERM -- matches.
- C2-lite problem description: 44 extensions missing exe/dll/so/dylib/class --
  count is 46 (not 44, counted from source lines 480-526), but the missing
  extensions are confirmed absent.
- Cross-cutting: `tempfile` already in `Cargo.toml` -- confirmed.
- No cross-item dependency conflicts found.
- Commit strategy uses `fix:` prefix for all items -- correct for release-please.

---

## Minor Accuracy Note

The spec says "Existing 44-extension denylist." Counting the actual array from
source (lines 480-526), there are 46 extensions (including "bin" at line 525).
Trivial but worth correcting for accuracy.
