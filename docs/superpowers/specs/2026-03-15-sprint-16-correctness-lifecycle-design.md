# Sprint 16: Correctness, Atomicity & Lifecycle

**Date:** 2026-03-15
**Status:** Approved
**Theme:** Trust, atomicity, and lifecycle correctness

## Scope

Six items, ordered by risk. Sprint focused on sealing write-path correctness,
daemon lifecycle, and bounded binary admission hardening.

| # | ID | Summary | Severity | Files |
|---|----|---------|----------|-------|
| 1 | C6 | `open_project_session` TOCTOU panic | CRITICAL | `src/daemon.rs` |
| 2 | C3 | `atomic_write_file` temp filename collision | CRITICAL | `src/protocol/edit.rs` |
| 3 | C1 | CRLF line ending preservation in surgical edits | HIGH | `src/protocol/edit.rs` |
| 4 | C4 | `batch_rename` splice overlap validation | MEDIUM-HIGH | `src/protocol/edit.rs` |
| 5 | C5 | Daemon SIGTERM handling (narrowed) | HIGH | `src/daemon.rs` |
| 6 | C2-lite | Denylist extension hardening | LOW | `src/domain/index.rs` |

**Execution order:** C6 -> C3 -> C1 -> C4 -> C5, with C2-lite parallel or last.

**Rationale:** Highest-risk fixes first. C6 and C3 are critical data-loss/panic
bugs. C1 and C4 are grouped as write-path cluster (settling newline behavior
before splice validation). C5 is daemon lifecycle. C2-lite is isolated hardening.

---

## C6: Fix `open_project_session` TOCTOU Panic

### Problem

`open_project_session` (daemon.rs:188-246) has two separate write-lock scopes
on `self.projects` RwLock:

1. Lines 195-205: Check if project exists, insert if not (calls slow
   `ProjectInstance::load()` under write lock)
2. Lines 213-224: Assume project exists, call `.expect("project must exist")`

**Race conditions:**
- Two concurrent opens for the same project_id both pass the "not loaded"
  check, spawning duplicate `ProjectInstance::load()` calls and duplicate watchers
- A concurrent `close_last_session()` can remove the project between the two
  lock scopes, causing `.expect()` to panic

### Design

**Correctness flow (double-checked locking):**

1. Acquire read lock
2. If project exists: drop read lock, acquire write lock, re-check, add session,
   ensure watcher exists, return
3. If absent: drop read lock, call `ProjectInstance::load()` unlocked
4. Acquire write lock
5. Re-check project existence:
   - If another thread inserted it, discard loaded instance, use existing
   - Else insert new instance
6. Register session under the same write lock
7. Activate the project (start watcher + git temporal) only if this thread
   performed the insertion — never on a discarded instance
8. Return fallible `Result`, never panic

**Key invariants:**
- One `ProjectInstance` per project_id, even under concurrent opens
- One watcher per project
- Session count equals number of successful opens (many sessions per project allowed)
- `ProjectInstance::load()` must be split into two phases:
  1. **Pure construction** — allocate index, parse files, build data structures.
     No spawned tasks, no background work, no OS watchers. Safe to discard.
  2. **Post-commit activation** — start watcher task, launch git temporal
     analysis. Called only after the write-lock re-check confirms this instance
     won insertion. If another thread won, the losing instance is dropped
     without activation — no leaked tasks.
- No `.expect()` on project existence — fallible returns throughout
- The inserted `ProjectInstance` must carry an explicit **activation state**
  (`Inactive` → `Activating` → `Active`) so that two racing opens cannot both
  decide to activate the same project after the write-lock re-check. The thread
  that wins insertion transitions `Inactive` → `Activating` under the write
  lock; the losing thread sees a non-`Inactive` state and skips activation.
  This makes the "one watcher per project" invariant trivially enforceable.

**Close/open race semantics:**
- If close wins before project is inserted, open may still complete and recreate
- If open wins and registers session first, close only removes project if no
  sessions remain
- Closing an already-absent project is a no-op, not an error

**Watcher spawn under write lock:**
- State mutation is atomic under the write lock
- Watcher creation must either be cheap, or represented as a committed
  "needs watcher" state inside the instance with expensive startup outside the lock
- Verify watcher spawn cost before implementation

### Tests

| Test | Assertions |
|------|------------|
| Same-project race (barrier, 50x loop) | No panic, 1 project instance, 1 watcher, session count == callers |
| Different-project concurrent opens | Both succeed, 2 watchers, 2 project instances |
| Open/close race (barrier) | No panic, no leaked watcher, final state matches close semantics |
| Load dedupe sanity | Instrument `load()` call count; allow >=1, verify only 1 instance inserted |
| Discarded instance cleanup | If 2 threads race load, the losing instance is dropped without spawning watcher/git tasks — no leaked tasks |

---

## C3: Fix `atomic_write_file` Temp Filename Collision

### Problem

`atomic_write_file` (edit.rs:52-57) uses deterministic temp name
`path.with_extension("tokenizor_tmp")`. Two concurrent edits to the same file
both write to the same temp path — race condition causes data loss.

### Design

**Use `tempfile` crate** (`NamedTempFile::new_in()`) instead of hand-rolled names:

1. `NamedTempFile::new_in(parent_dir)` — unique temp file in same directory
2. Write full contents to temp file
3. `flush()` then `sync_all()` on the temp file (durability, not just collision safety)
4. `temp.persist(target_path)` — atomic rename (same filesystem)
5. On error: temp file auto-deleted by `NamedTempFile` drop

**Semantics:**
- Last-writer-wins is correct — not serializing concurrent writes
- Invariant: final file contains exactly one complete payload, never mixed/truncated
- No orphan temp files remain on any code path

**Platform-specific replace behavior:**

- **Linux/macOS:** `persist()` uses `rename(2)` which atomically replaces an
  existing target. No additional work needed.
- **Windows:** `std::fs::rename` fails with `ERROR_ALREADY_EXISTS` when the
  target exists. `tempfile` v3.x provides `NamedTempFile::persist()` which
  internally uses `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING` on Windows,
  handling this correctly. **Verify this at implementation time** by checking
  the `tempfile` crate source.
  - **If verified:** Use `persist()` directly — atomic replace on all platforms.
  - **If NOT verified:** Do NOT fall back to `remove_file` + `persist()` — that
    breaks the atomic-write contract during the brief delete/recreate window.
    Instead, implement a platform-specific atomic replace using
    `MoveFileExW(MOVEFILE_REPLACE_EXISTING)` directly via `windows-sys` or
    `winapi` crate. The non-atomic `remove + persist` path is a **last resort
    only**, acceptable only if no atomic alternative exists on the platform.

### Tests

| Test | Assertions |
|------|------------|
| Concurrent writes (barrier, distinct 1MB payloads) | Final file == exactly payload A or exactly payload B, never hybrid |
| Orphan temp cleanup | No temp files remain in directory after concurrent writes |
| Error path cleanup | Target unchanged, no orphan temp files on write/persist failure |

---

## C1: CRLF Line Ending Preservation in Surgical Edits

### Problem

`apply_indentation()` (edit.rs ~line 147) uses `.lines()` which strips `\r\n`,
then reconstructs with hardcoded `\n`. Any surgical edit on a CRLF file silently
converts the affected region to LF.

### Policy Decisions

1. Detect the file's native line ending style before editing
2. Preserve that style throughout the edited output
3. Inserted text containing `\n` is normalized to the file's native style
4. No mixed line endings introduced unintentionally
5. Unedited file bytes are preserved exactly — only generated/inserted text is
   normalized

### Design

**Detection — dominant-count, not first-match:**

```
count \r\n occurrences
count lone \n occurrences
if \r\n > \n → CRLF
else → LF
empty/no-newline → LF default
```

**Normalization helper (`normalize_line_endings`):**

1. Convert `\r\n` -> `\n`
2. Convert lone `\r` -> `\n`
3. If target is CRLF, convert `\n` -> `\r\n`

Applied only to generated/replacement text, never to untouched file regions.

**Trailing newline tracking:**

Detect whether the original edited region ends with a newline. Do not
accidentally append one during text reconstruction. Track separately from
line ending style via `EditTextPolicy { line_ending, had_trailing_newline }`.

**Affected functions (all receive `LineEnding` from detection):**

- `apply_indentation` — use detected ending instead of hardcoded `\n`
- `build_insert_before` / `build_insert_after` — normalize inserted text
- `build_delete` — scans for bare `\n` when trimming trailing newlines; must
  handle `\r\n` to avoid leaving orphan `\r` bytes
- `collapse_blank_lines` — counts consecutive `\n` to detect 3+ blank lines;
  must count `\r\n` pairs on CRLF files or the threshold never triggers
- `replace_symbol_body` content assembly
- `edit_within_symbol` replacement text
- `batch_edit` / `batch_insert` per-file processing (detect once per file)

**Scope boundary:** Fixes the edit output path only. Indexer/parser already
handles both endings correctly.

### Tests

| Test | Assertions |
|------|------------|
| CRLF round-trip after surgical edit | `\r\n` preserved throughout entire file |
| CRLF with no trailing newline | No trailing newline added |
| Multi-line LF inserted into CRLF file | Normalized to `\r\n` |
| Indentation-sensitive edit (doc comments) | CRLF preserved |
| LF file stays LF | No false conversion to CRLF |
| No accidental mixed endings | Invariant: output has no mixed `\r\n`/`\n` unless input was mixed |
| batch_edit on CRLF file | Multiple replacements preserve CRLF |
| Multiple same-file edits on CRLF | insert + replace on same CRLF file preserves endings |
| Delete symbol in CRLF file | No orphan `\r` bytes left after trailing newline trim |
| collapse_blank_lines on CRLF | 3+ consecutive `\r\n` blank lines correctly detected and collapsed |

---

## C4: `batch_rename` Splice Overlap Validation

### Problem

`execute_batch_rename` (edit.rs:747-1008) collects rename ranges from two
sources (indexed refs + qualified scan), deduplicates with `.dedup()`, and
applies splices in descending offset order. Two bugs:

1. `.dedup()` only removes adjacent identical elements — overlapping but
   non-identical ranges pass through and corrupt the file
2. No validation that ranges actually contain the expected old name text —
   stale index data or wrong qualified-scan matches apply silently

### Design

**All overlap is planner error. Never silently merge contained ranges.**

**Per-file planning (after sort + dedup):**

1. Sort by `(start desc, end desc)` — note: current code sorts only by start;
   update comparator to `|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1))`
2. Exact-dedup only (current behavior)
3. Validate each range:
   - `start < end`
   - `end <= original.len()`
   - `&original[start..end] == old_name.as_bytes()` (range-text validation)
4. Validate no overlaps between adjacent ranges:
   ```
   // Ranges sorted descending. For adjacent pair (prev=higher, curr=lower):
   // Overlap exists if curr.end > prev.start
   // Adjacency (curr.end == prev.start) is allowed
   ```
5. Any validation failure → return error with diagnostic info

**Apply loop (unchanged):**

Descending-offset application is correct for validated non-overlapping ranges.
Each splice at offset N only affects bytes at position >= N. The next splice
targets a lower offset, which is unaffected.

**Defense-in-depth assertion:**

Debug assertion during apply loop that each range's start is strictly below the
previous range's start offset.

**Dry-run reporting (enhanced):**

Report post-dedup, post-validation site counts. Include:
- Total indexed candidates
- Total qualified-scan candidates
- Exact duplicates removed
- Final applied site count

### Tests

| Test | Assertions |
|------|------------|
| Exact duplicate ranges | Deduped to one |
| Contained ranges (e.g., (10,20) and (12,15)) | Rejected with error, not merged |
| Partially overlapping ranges | Rejected with error |
| Adjacent non-overlapping ranges | Allowed, both applied correctly |
| Length-changing rename with close references | Exact expected output |
| Range-text mismatch (stale index) | Fails with clear error message |
| Naive offset corruption case | Test where skipping validation would corrupt 2nd replacement |
| Dry-run site count | Reflects post-dedup, post-validation count |

---

## C5: Daemon SIGTERM Handling (Narrowed)

### Problem

`run_daemon_until_shutdown` (daemon.rs:1017-1023) only listens for
`tokio::signal::ctrl_c()` (SIGINT). SIGTERM from systemd, containers, or `kill`
is ignored — daemon stays alive until SIGKILL.

### Scope (narrowed for this sprint)

**In scope:**
- SIGTERM handling in the daemon process
- Graceful shutdown path runs on SIGTERM (same as SIGINT)
- Cleanup files (port, pid) are removed
- Idempotent shutdown
- Direct signal API for `terminate_process` (replace shell-out)

**Out of scope (deferred):**
- Full daemonization redesign (setsid, double-fork)
- Process reaping for non-child daemons (cannot `waitpid()` a non-child)
- Zombie lifecycle guarantees for spawn path

### Design

**SIGTERM handling:**

```rust
#[cfg(unix)]
{
    let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = sigterm.recv() => {},
    }
}
#[cfg(not(unix))]
{
    tokio::signal::ctrl_c().await?;
}
```

After signal, existing `shutdown_tx.send(())` triggers graceful shutdown.
`cleanup_daemon_files()` runs in the server task exit path (line 1007).

**Shutdown contract:**
> SIGTERM and SIGINT both trigger the same graceful shutdown path and cleanup.

**Idempotent termination contract:**
> When `terminate_process` targets an already-dead PID, that is a **clean,
> idempotent outcome** — not an operational failure. The function must:
> 1. Attempt SIGTERM (or platform equivalent)
> 2. If the process is already gone, treat it as success
> 3. Clean up stale pid/port files regardless
> 4. Return Ok, not Err
>
> This prevents stale daemon state from blocking new daemon startup.

**`terminate_process` improvement (Unix):**
- Use `libc::kill(pid, SIGTERM)` or `nix::sys::signal::kill` directly
- Poll for process disappearance with short timeout
- Do not pretend to reap non-child processes
- Return useful result

### Tests

| Test | Assertions |
|------|------------|
| Spawn daemon, send SIGTERM, wait | Process exits within timeout, pid/port files removed |
| SIGINT still works | Existing behavior preserved |
| Repeated shutdown (SIGTERM to dead process) | Handled cleanly, no error |
| Stop command on recorded PID | SIGTERM sent, timeout/error messaging is sane |

---

## C2-lite: Denylist Extension Hardening

### Problem

Existing 46-extension denylist in `DENYLISTED_EXTENSIONS` (domain/index.rs)
misses common executable/library formats found in real repositories.

### Design

**Add 5 extensions:**

| Extension | Category |
|-----------|----------|
| `.exe` | Windows executable |
| `.dll` | Windows dynamic library |
| `.so` | Linux shared library |
| `.dylib` | macOS shared library |
| `.class` | Java compiled bytecode |

**Case-insensitive matching:**

Ensure extension comparison is case-insensitive (already implemented in
`is_denylisted_extension`; verify preserved after adding new entries):
- Lowercase the extracted extension once
- Compare against lowercase denylist

**Precedence rule:**

Denylisted extension wins over size-based Tier 1 eligibility. A tiny (100-byte)
`.exe` file still gets `MetadataOnly`, not `Normal`.

**Scope boundary:**

No magic-byte detection, no node_modules policy, no admission redesign. Those
are deferred to a future sprint.

### Tests

| Test | Assertions |
|------|------------|
| `.exe`, `.dll`, `.so`, `.dylib`, `.class` | All → MetadataOnly tier |
| Tiny denylisted file (100 bytes) | Still MetadataOnly (denylist wins over size) |
| Non-denylisted small text file | Normal (Tier 1) |
| Mixed-case: `.DLL`, `.So`, `.EXE` | Case-insensitive → MetadataOnly |

---

## Cross-Cutting Concerns

### Test categorization

| Category | Items |
|----------|-------|
| Unit tests | C1, C2-lite, C4 |
| Filesystem tests | C3 |
| Concurrency/integration tests | C6 |
| Integration/process tests (Unix) | C5 |

### Dependencies

- C3 adds `tempfile` crate dependency
- C1 introduces `LineEnding` enum and `EditTextPolicy` struct in edit.rs
- C5 adds `#[cfg(unix)]` conditional compilation for signal handling
- No cross-item dependencies — each can be implemented and tested independently

### Commit strategy

One commit per item, conventional commit prefixes:
- `fix: resolve open_project_session TOCTOU panic (C6)`
- `fix: use unique temp files in atomic_write_file (C3)`
- `fix: preserve CRLF line endings in surgical edits (C1)`
- `fix: validate splice overlap in batch_rename (C4)`
- `fix: handle SIGTERM for daemon graceful shutdown (C5)`
- `fix: add exe/dll/so/dylib/class to denylist (C2-lite)`
