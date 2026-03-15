# Sprint 16 Implementation Plan Review

**Reviewed:** `docs/superpowers/plans/2026-03-15-sprint-16-correctness-lifecycle.md`
**Against:** `docs/superpowers/specs/2026-03-15-sprint-16-correctness-lifecycle-design.md`
**Date:** 2026-03-15
**Reviewer:** Code Review Agent

---

## Summary

The plan is well-structured and covers all 6 spec items. 17 tasks across 6 chunks with
clear file targets, code snippets, compile-check gates, and test steps. However, there
are several issues ranging from critical (will cause compilation failure or incorrect
behavior) to important (spec coverage gaps, test gaps).

**Verdict:** Solid plan with ~8 issues to fix before execution. None are architectural;
all are localized corrections.

---

## Critical Issues (MUST FIX before execution)

### C1. Chunk 1 (C6): `activate()` runs under write lock -- contradicts spec intent

**Plan lines 136-143:** The plan says "Activate outside the write lock if we inserted"
but immediately re-acquires the write lock:

```rust
if needs_activation {
    let mut projects = self.projects.write().expect("lock poisoned");
    if let Some(project) = projects.get_mut(&project_id) {
        if project.activation_state == ActivationState::Activating {
            project.activate();
        }
    }
}
```

`activate()` calls `start_project_watcher()` and `spawn_git_temporal_computation()`
under the write lock. The spec explicitly says: "Watcher creation must either be cheap,
or represented as a committed 'needs watcher' state inside the instance with expensive
startup outside the lock."

Looking at the actual `start_project_watcher` (line 911-919 in daemon.rs), it spawns
a tokio task -- this is cheap (just scheduling, no blocking). So running under the write
lock is likely acceptable in practice, but the plan's comment "Activate outside the write
lock" is misleading. Either:
- (a) Fix the comment to say "Activate under a second write lock acquisition"
- (b) Actually move activation outside the lock by extracting the needed Arcs first

**Recommendation:** Option (a) -- rename the comment. The current code is functionally
correct because `start_project_watcher` and `spawn_git_temporal_computation` are
non-blocking spawns. But the misleading comment will confuse future readers.

### C2. Chunk 1 (C6): Discarded instance in slow path drops silently but spec requires verification

**Plan lines 124-126:** When another thread wins the race:
```rust
if projects.contains_key(&project_id) {
    // Another thread won the race -- discard our loaded instance (no tasks to clean up).
    false
}
```

The `new_project` variable (of type `ProjectInstance`) is dropped here. The plan's
`load()` refactoring ensures no watcher_task is spawned (it's `None`). However, `load()`
still calls `LiveIndex::load()` which allocates an index with internal data. The plan
does NOT explicitly verify that dropping a `ProjectInstance` with `activation_state: Inactive`
is truly side-effect-free.

**Recommendation:** Add a comment documenting this invariant. The spec's test matrix
includes "Discarded instance cleanup" -- the plan's Task 3 tests do NOT include this
test. Add it.

### C3. Chunk 2 (C3): `tempfile` is already in Cargo.toml -- Step 1 is a no-op

The plan says to run `cargo add tempfile`, but `tempfile = "3"` already exists in
Cargo.toml (line 58). This is not harmful (cargo add is idempotent) but the plan's
Step 2 verification note about checking tempfile persist behavior should be the actual
first step.

**Recommendation:** Note this as already-present. No action needed.

### C4. Chunk 3 (C1): `apply_indentation` trailing newline check is incomplete for CRLF input

**Plan line 688:**
```rust
if text.ends_with('\n') || text.ends_with("\r\n") {
    result.extend_from_slice(newline);
}
```

The `text` parameter is `&str`. If the input `text` already has `\r\n` endings,
`text.ends_with('\n')` is `true` (since `\r\n` ends with `\n`), so the `||` branch
is redundant but harmless. However, the real issue is that `.lines()` on line 679
strips both `\n` and `\r\n`, so the function reconstructs all line breaks using the
`line_ending` parameter. If `text` contains `\r\n` and `line_ending` is `CrLf`, the
output is correct. If `text` contains `\r\n` but `line_ending` is `Lf`, the `\r` is
stripped by `.lines()` and reconstructed as `\n` -- also correct. So this is fine.

**No action needed.**

### C5. Chunk 3 (C1): Missing callers list -- `execute_batch_edit` has 4 call sites that need LineEnding

**Plan Task 7 Step 2** lists 6 callers of `apply_indentation` / insert helpers, but
the actual `execute_batch_edit` function at lines 600-700 calls:
- `apply_indentation` (line 613)
- `build_insert_before` (line 624)
- `build_insert_after` (line 633)
- `build_delete` (line 643)

All 4 of these calls are inside `execute_batch_edit` and need the `LineEnding` parameter.
The plan mentions "execute_batch_edit" as one caller but does not mention that
`build_delete` is also called from `execute_batch_edit` (in addition to being called
from `tools.rs:3177`). When the plan updates `build_delete` in Task 8, it must also
update this call site in `execute_batch_edit`.

**Additionally**, `tools.rs:3177` calls `build_delete` directly for the `delete_symbol`
tool handler. The plan's Task 8 Step 3 says "Both are called from tool handlers" but
doesn't enumerate all sites. The full list of `build_delete` callers is:
1. `execute_batch_edit` (edit.rs:643)
2. `delete_symbol` handler (tools.rs:3177)

**Recommendation:** Enumerate all call sites explicitly in Task 8 Step 3 to prevent
missed updates during execution.

### C6. Chunk 3 (C1): `build_delete` has CRLF bugs beyond trailing newline

Looking at the actual `build_delete` implementation (lines 229-302), it uses:
- `b == b'\n'` for line scanning (line 234, 282, 285, 289)
- `.split(|&b| b == b'\n')` for upward doc-comment scanning (line 247)

On CRLF files, splitting on `\n` leaves trailing `\r` on each line element. The
doc-comment detection at line 254 does `trimmed.starts_with("///")`, but if the line
has a trailing `\r`, `trim_start()` won't strip it (it strips leading whitespace only).
This means the upward-scanning logic for orphaned doc comments will work correctly
because `\r` is at the END of lines, not the start. However, the `is_empty()` and
`is_ascii_whitespace()` checks at line 260 will fail because `\r` is ASCII whitespace,
so blank-line detection still works.

The trailing newline extension at lines 281-289 uses bare `\n` checks:
```rust
while pos < file_content.len() && file_content[pos] != b'\n' { pos += 1; }
if pos < file_content.len() { pos += 1; } // skip \n
if pos < file_content.len() && file_content[pos] == b'\n' { pos += 1; }
```

On CRLF files, this skips only the `\n` and leaves the next `\r` as a non-newline
byte, breaking the "consume up to one blank line" logic. The plan's Task 8 Step 2
says to update this but provides no code snippet. The implementer needs clear guidance
on the CRLF-aware version of this trailing-newline extension.

**Recommendation:** Add explicit code for the CRLF-aware trailing newline extension
in Task 8 Step 2. Something like:

```rust
let newline_len = match line_ending {
    LineEnding::CrLf => 2,
    LineEnding::Lf => 1,
};
// Skip to end of current line
while pos < file_content.len() && file_content[pos] != b'\n' { pos += 1; }
if pos < file_content.len() { pos += 1; } // skip \n
// On CRLF: also skip \r before the next \n for blank line detection
if line_ending == LineEnding::CrLf {
    if pos + 1 < file_content.len()
        && file_content[pos] == b'\r'
        && file_content[pos + 1] == b'\n'
    {
        pos += 2;
    }
} else if pos < file_content.len() && file_content[pos] == b'\n' {
    pos += 1;
}
```

### C7. Chunk 4 (C4): validate_rename_ranges reads file content TWICE

**Plan Task 11 Step 1:**
```rust
for (path, ranges) in by_file.iter_mut() {
    let file = {
        let guard = index.read().expect("lock poisoned");
        guard.capture_shared_file(path)...
    };
    validate_rename_ranges(ranges, &file.content, &input.name, path)?;
}
```

Then in Phase 4 (the apply loop at line ~890+), the code reads the file content AGAIN:
```rust
let file = { guard.capture_shared_file(path)... };
let original = file.content.clone();
```

This creates a TOCTOU window: if the file changes between validation and application,
the validated ranges may be stale. The spec says "All overlap is planner error. Never
silently merge." -- but the plan doesn't address this TOCTOU between validation and
application.

**Recommendation:** Either:
(a) Capture the content once during validation, store it, and reuse during apply (preferred)
(b) Document this as an accepted limitation (the existing code already has this issue)

Option (b) is acceptable since the existing code has the same race, and concurrent
file modification during rename is inherently unsafe.

### C8. Chunk 5 (C5): Missing `libc` dependency for Unix signal handling

**Plan Task 13 Step 2** says "Check: `grep 'libc' Cargo.toml` / If not present:
`cargo add libc`". My grep confirms `libc` is NOT in Cargo.toml. The plan correctly
identifies this needs to be added, but Task 12 (SIGTERM handling) does NOT use libc
directly -- it uses `tokio::signal::unix::signal(SignalKind::terminate())` which is
pure tokio. The libc dependency is only needed for Task 13's `terminate_process` rewrite
which calls `libc::kill()` and `libc::ESRCH`.

This ordering is correct. No issue here.

**However:** On Windows, the `#[cfg(not(windows))]` block in `terminate_process` uses
`libc::kill` and `libc::SIGTERM` and `libc::ESRCH`. The `libc` crate on Windows does
NOT provide `kill()`, `SIGTERM`, or `ESRCH`. The plan uses `#[cfg(not(windows))]` which
is correct for gating, but `libc` must be added as a target-specific dependency:

```toml
[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

Or alternatively as a general dependency since libc compiles on all platforms but the
plan's cfg gates prevent calling unix-only APIs. Either way works, but this should be
noted.

**Recommendation:** Use `cargo add libc` (unconditional) since the cfg gates handle
platform safety. The plan is correct as-is.

---

## Important Issues (SHOULD FIX)

### I1. Spec test coverage gaps

The spec defines the following tests that are MISSING from the plan:

**C6 (TOCTOU):**
- "Open/close race (barrier)" -- no test in the plan for concurrent open + close
- "Load dedupe sanity" -- no instrumented load() call count test
- "Discarded instance cleanup" -- no test verifying dropped instance has no leaked tasks

**C3 (atomic_write):**
- "Error path cleanup" -- no test for write failure (e.g., read-only directory)

**C1 (CRLF):**
- "Multiple same-file edits on CRLF" -- no test combining insert + replace on CRLF
- "batch_edit on CRLF file" -- no test

**C4 (splice overlap):**
- "Naive offset corruption case" -- no test demonstrating corruption without validation
- "Dry-run site count" -- no test verifying post-validation counts

**C5 (SIGTERM):**
- "Spawn daemon, send SIGTERM, wait" -- no integration test (only idempotent dead-PID test)
- "SIGINT still works" -- no regression test
- "Stop command on recorded PID" -- no test

**Recommendation:** Add stub tests for the missing items, particularly the C6 open/close
race test and C3 error path test. The C5 integration tests may require process spawning
which is harder to test; documenting them as manual verification is acceptable.

### I2. Chunk 3 (C1): `build_insert_before` separator logic needs CRLF update

The current `build_insert_before` (lines 175-203) uses hardcoded `b"\n"` and `b"\n\n"`
for separators. The plan's Task 7 Step 3 shows the updated signature with `line_ending`
but the separator logic still uses:

```rust
let separator: &[u8] = if sym.doc_byte_range.is_some() {
    b"\n"
} else { ... b"\n\n" }
```

These must become `newline` (single) and `[newline, newline]` (double) using the
detected line ending. The plan's pseudocode at line 737-739 shows `newline` being used
for the double-newline case, which is correct. But the plan doesn't show the full
updated separator logic including the `already_has_blank` detection, which scans for
`\n\n` and would need to scan for `\r\n\r\n` on CRLF files.

**Recommendation:** Add explicit code for the `already_has_blank` check in CRLF mode:
```rust
let already_has_blank = match line_ending {
    LineEnding::CrLf => prefix.len() >= 4
        && prefix[prefix.len()-2..] == *b"\r\n"
        && prefix[prefix.len()-4..prefix.len()-2] == *b"\r\n",
    LineEnding::Lf => prefix.len() >= 2
        && prefix[prefix.len()-1] == b'\n'
        && prefix[prefix.len()-2] == b'\n',
};
```

### I3. Chunk 1 (C6): `register_session_for_existing_project` has a race with close

If `close_session` removes the last session and then removes the project from the map
between the `open_project_session` fast-path check (`projects.contains_key`) and the
`register_session_for_existing_project` call, the helper will return an error:
"project was removed between check and session registration."

The spec says "Closing an already-absent project is a no-op, not an error" but the
plan's open path returns an error in this case. This is arguably correct (the open
fails, the client retries), but the spec's "Close/open race semantics" section implies
open should still succeed by recreating the project.

**Recommendation:** Document this as accepted behavior. The open returning an error
and the client retrying is a valid interpretation. Alternatively, fall through to the
slow path when the fast path fails.

### I4. Chunk 3 (C1): Test `test_apply_indentation_preserves_crlf` has confusing assertion

**Plan line 839:**
```rust
assert!(!result_str.contains("\r\n").then(|| true).is_none() || result_str.contains("\r\n"));
```

This assertion is a tautology -- it's always true. The `.contains("\r\n").then(|| true)`
returns `Some(true)` if CRLF present, `None` if not. `.is_none()` is false if present,
true if not. `!false || contains` = `true || anything` = `true`. And `!true || contains`
= `false || contains` which only tests the second condition.

The second assertion (lines 841-845) using the byte-level scan is the correct one.
The first assertion should be removed or replaced.

**Recommendation:** Remove the confusing assertion at line 839. The byte-level scan
at lines 841-845 is sufficient and correct.

---

## Suggestions (NICE TO HAVE)

### S1. Plan does not specify `use` imports for new types

The plan introduces `LineEnding`, `detect_line_ending`, `normalize_line_endings` in
`edit.rs` and uses them from `tools.rs` (line 3023, 3109, 3111, 3177). The plan should
note that `tools.rs` imports need updating:

```rust
use super::edit::{detect_line_ending, LineEnding, ...};
```

### S2. Chunk 4 test uses `apply_splice` directly

Plan Task 11 Step 5 test `test_batch_rename_length_change_close_refs` calls
`apply_splice` directly in a loop. This doesn't actually test the `execute_batch_rename`
integration. Consider adding a test that goes through the full rename pipeline with
validated ranges.

### S3. Commit strategy has redundant squash steps

Tasks 1-3 (C6) produce 3 commits then suggest "squash if desired" (Step 7). Tasks 12-14
(C5) similarly produce 3 commits then squash. The plan should pick one strategy: either
commit once at the end of each chunk, or commit per task without squashing.

---

## Cross-Chunk Interaction Analysis

The plan correctly identifies no cross-chunk dependencies. Verified:

- C6 modifies `daemon.rs` (struct + methods) -- no edit.rs changes
- C3 modifies `edit.rs:51-56` only -- function body replacement
- C1 modifies `edit.rs` signatures + `tools.rs` callers -- large surface area
- C4 modifies `edit.rs` batch_rename section -- separate from C1's affected functions
- C5 modifies `daemon.rs` -- different functions than C6
- C2-lite modifies `domain/index.rs` -- completely isolated

**One concern:** C1 and C4 both modify `edit.rs`. C1 adds `LineEnding` parameters to
many functions. C4 adds `validate_rename_ranges` and modifies `execute_batch_rename`.
If C4's `execute_batch_rename` also needs CRLF awareness (it writes files via
`atomic_write_file`), this would be a cross-chunk dependency. However, `batch_rename`
writes complete file content and doesn't generate text, so CRLF preservation is handled
implicitly by the fact that rename only substitutes exact byte ranges. No issue.

---

## Final Assessment

| Category | Count |
|----------|-------|
| Critical | 2 real issues (C2: missing spec test, C6: build_delete CRLF code missing) |
| Important | 4 issues (spec test gaps, separator logic, race semantics, bad assertion) |
| Suggestions | 3 items |

The plan is ready for execution with the fixes noted above. The most important
pre-execution fix is adding explicit CRLF-aware code for `build_delete`'s trailing
newline extension (C6 above) and adding the missing spec tests (I1).
