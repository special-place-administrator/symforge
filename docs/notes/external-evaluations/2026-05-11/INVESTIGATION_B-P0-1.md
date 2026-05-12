# INVESTIGATION B-P0-1 — Index Self-Destruction After `index_folder`

**Investigator:** SymForge campaign read-only auditor
**HEAD:** f804d21
**Mode:** Read-only. No source files modified.
**Source of claims under test:** `docs/notes/external-evaluations/2026-05-11/SYMFORGE_EVALUATION_2026-05-11.md` (Kimi Code CLI)

---

## 0. Verdict per mechanism

### Mechanism A — Stale `spawn_blocking` reconcile + shared `SharedIndex` across roots
**Verdict: CONFIRMED** — and the actual mechanism is more severe than the
evaluator described.

Evidence anchors:
- `src/daemon.rs:1070-1107` `ProjectInstance::reload` — calls
  `abort_watcher_task(&mut self.watcher_task);` followed by
  `self.watcher_task = start_project_watcher(canonical_root, Arc::clone(&self.index), ...)`.
- `src/daemon.rs:1120-1124` `abort_watcher_task` — body is literally
  `if let Some(task) = task.take() { task.abort(); }`. No cancellation
  signaling, no awaiting drain, no project-generation bump.
- `src/daemon.rs:1110-1118` `start_project_watcher` — spawns
  `watcher::run_watcher(repo_root, index, watcher_info)` where `index:
  SharedIndex = Arc<SharedIndexHandle>`. The new task captures
  `Arc::clone(&self.index)` — the SAME Arc the old task captured.
- `src/protocol/tools.rs:4394-4458` `index_folder` (the
  `SymForgeServer`-side path, used by per-session non-daemon flows and
  also called from inside the daemon plumbing). After `index.reload(...)`
  it calls `crate::watcher::restart_watcher(root, Arc::clone(&self.index),
  Arc::clone(&self.watcher_info))` — and `restart_watcher`
  (`src/watcher/mod.rs:676-686`) **does not stop or signal the previous
  watcher at all**. It just `tokio::spawn(run_watcher(...))`. So this
  path leaks watchers strictly worse than the daemon path.
- `src/live_index/store.rs:433-448` `SharedIndexHandle` carries
  `live: ArcSwap<LiveIndex>` and write helpers — there is **no
  `repo_root` field, no `project_id`, no generation token**. It is
  pure index state, fully root-agnostic.
- `src/watcher/mod.rs:337-382` `reconcile_stale_files(repo_root, shared)`
  reads `paths` from `shared.read().all_files()` and constructs
  `abs_path = repo_root.join(relative_path)`. The old watcher's
  `repo_root` was captured by value when its task was spawned.
- `src/watcher/mod.rs:216-283` `maybe_reindex` — line 246-251 is the
  `NotFound` arm:
  ```
  Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
      shared.remove_file(relative_path);
      warn!("watcher: file not found, removed from index: {relative_path}");
      return ReindexResult::Removed;
  }
  ```

The sharper mechanism: after `reload()` the `SharedIndex` Arc now contains
files **discovered under root B**, but the doomed task captured root A.
The doomed task's reconcile loop does
`paths = shared.read().all_files()` (returns paths from root B because
both watchers share the Arc), then `abs_path = root_A.join(path_from_B)`,
which is virtually guaranteed to be `NotFound` on disk, and calls
`shared.remove_file(relative_path)` — destroying B's entries one by one.

This is consistent with the observed monotonic collapse 1135 → 4 over
two minutes (30-second reconcile interval, multiple sweeps). The "4 root
files survive" detail also fits: only files whose relative path happens
to exist under both roots (e.g. `Cargo.toml`, `README.md`,
`scripts/coordination.js`) would *not* be removed because
`root_A.join("Cargo.toml")` does in fact exist.

Severity: catastrophic. Triggered every time `index_folder` is called
against a new root for an open session.

### Mechanism B — Buffered events on a dropped debouncer
**Verdict: REFUTED**, with one caveat.

Evidence anchors:
- `src/watcher/mod.rs:391-396` `WatcherHandle` owns
  `_debouncer: Debouncer<...>` and `event_rx: Receiver<DebounceEventResult>`.
  Dropping `WatcherHandle` drops the debouncer, which closes the notify
  callback's sender side.
- `src/watcher/mod.rs:402-422` `start_watcher` constructs the channel
  and returns the `WatcherHandle`. The receiver lives inside the
  watcher task's `handle` local; on inner-loop exit, the local goes
  out of scope along with the entire task frame.
- The old task drops its `WatcherHandle` only when the task body
  unwinds — and since `task.abort()` does not actually unwind a
  blocking section, the old task may live on. But the *channel* is
  carried inside that same task's stack — only that task reads
  `event_rx`. There is no second consumer of buffered events
  populating the new index.

So the buffered-events claim does not match the wiring. The doomed
task continues to consume events from its own debouncer (still alive,
because the task's local frame still owns it), and those events refer
to the **old root A**. `normalize_event_path(abs_path, repo_root_A)`
returns paths relative to A. If the user is editing files under B,
those events never even arrive on the doomed channel — they arrive on
the *new* debouncer.

Caveat: the doomed task can still synthesize `Remove` events
internally via `reconcile_stale_files`, which is Mechanism A. So
events do destroy the index, but not via buffered debouncer events.

### Mechanism C — Windows `NotFound` from AV / lock race
**Verdict: PARTIALLY-CONFIRMED for single-watcher case; not the dominant cause**.

Evidence:
- `src/watcher/mod.rs:243-251` does map `ErrorKind::NotFound` to
  `shared.remove_file` immediately, with no retry, no backoff, no
  containment check, no path-existence re-stat.
- `src/watcher/mod.rs:161-187` `normalize_event_path` handles `\\?\`
  on the abs_path side and on the `repo_root` side, but it does
  **not** guarantee the same canonical form between the path that
  comes from `notify` and the path stored under `index.all_files()`.
- The evaluator's external `std::fs::read` test of 119,752 paths with
  zero failures rules out blanket Windows-pathing brokenness.
- The Kimi report attributes the loss to the same `NotFound` → remove
  path regardless of mechanism. Even with no race at all, a single
  AV-locked file would be removed permanently — there is no
  reverification or quarantine.

Mechanism C is a *latent* bug that would cause sporadic single-file
losses without Mechanism A. With Mechanism A present, it is dwarfed by
the mass-removal pattern.

---

## 1. Actual code traced — file:line:fn

### 1. `ProjectInstance::reload` — `src/daemon.rs:1070-1107`

```rust
fn reload(&mut self, canonical_root: &Path) -> anyhow::Result<(usize, usize)> {
    self.index.reload(canonical_root)?;
    let published = self.index.published_state();
    let file_count = published.file_count;
    let symbol_count = published.symbol_count;

    abort_watcher_task(&mut self.watcher_task);
    self.watcher_task = start_project_watcher(
        canonical_root.to_path_buf(),
        Arc::clone(&self.index),
        Arc::clone(&self.watcher_info),
    );
    self.canonical_root = canonical_root.to_path_buf();
    // ... rebuilds server, refreshes git temporal ...
}
```

State mutated: `self.canonical_root`, `self.project_name`,
`self.project_id`, `self.server`, `self.watcher_task`. The
`self.index` (an `Arc<SharedIndexHandle>`) is **mutated in place via
`self.index.reload(canonical_root)`** — the Arc identity is preserved.
The `watcher_info` and `token_stats` Arcs are also preserved.

### 2. `abort_watcher_task` — `src/daemon.rs:1120-1124`

```rust
fn abort_watcher_task(task: &mut Option<tokio::task::JoinHandle<()>>) {
    if let Some(task) = task.take() {
        task.abort();
    }
}
```

Confirmed: only `task.abort()`. No await, no cancellation signal, no
`JoinHandle::is_finished` poll, no grace period. Per tokio semantics,
`abort()` requests cancellation at the next `.await` point; any work
currently inside `tokio::task::spawn_blocking(...)` runs to completion.

### 3. `start_project_watcher` — `src/daemon.rs:1110-1118`

```rust
fn start_project_watcher(
    repo_root: PathBuf,
    index: SharedIndex,
    watcher_info: Arc<Mutex<WatcherInfo>>,
) -> Option<tokio::task::JoinHandle<()>> {
    tokio::runtime::Handle::try_current()
        .ok()
        .map(|handle| handle.spawn(watcher::run_watcher(repo_root, index, watcher_info)))
}
```

Captures `Arc<SharedIndexHandle>` and `Arc<Mutex<WatcherInfo>>`. Both
Arcs are also held by the previous task. There is also a near-twin
spawner in `src/watcher/mod.rs:676-686` `restart_watcher` used by the
`SymForgeServer::index_folder` path (`src/protocol/tools.rs:4438-4444`)
which does not even nominally try to stop the previous watcher.

### 4. `run_watcher` (watcher supervision loop) — `src/watcher/mod.rs:496-670`

Outer loop:
```
loop {
    match start_watcher(&repo_root, debounce_ms) {
        Err(_) => ... consecutive_failures backoff ...
        Ok(handle) => {
            ... inner loop ...
        }
    }
}
```

Inner loop is at `src/watcher/mod.rs:559-660`. The relevant
`spawn_blocking` sites:

a. Periodic reconcile (`src/watcher/mod.rs:564-590`):
```rust
if reconcile_interval_secs > 0
    && last_reconcile.elapsed() >= Duration::from_secs(reconcile_interval_secs)
{
    let shared_clone = shared.clone();
    let root_clone = repo_root.clone();
    let watcher_info_clone = watcher_info.clone();
    tokio::task::spawn_blocking(move || {
        let stale = reconcile_stale_files(&root_clone, &shared_clone);
        let mut info = watcher_info_clone.lock();
        info.stale_files_found += stale as u64;
        info.last_reconcile_at = Some(SystemTime::now());
    });
    let root_for_coupling = repo_root.clone();
    tokio::task::spawn_blocking(move || {
        crate::live_index::coupling::refresh_on_reconcile_tick(&root_for_coupling);
    });
    last_reconcile = Instant::now();
}
```

Two important details:

- The reconcile `spawn_blocking` task is **fire-and-forget** — the
  handle is dropped immediately. No `.await`, no join. Aborting the
  outer task does nothing to this child blocking task.
- `shared.clone()` clones the `Arc<SharedIndexHandle>`. After the
  outer watcher is "aborted", the child blocking task still holds an
  Arc keeping the handle alive and continues to call
  `reconcile_stale_files` against it with the **old `repo_root`**.

b. `process_events` runner (`src/watcher/mod.rs:599-621`):
```rust
match tokio::task::spawn_blocking(move || {
    process_events(events, &root_clone, &shared_clone, &mut trackers, &watcher_info_clone);
    trackers
}).await { ... }
```

This site is `.await`ed, so a parent abort propagates between events,
but only once the in-flight call returns. While the blocking call is
running, abort has no effect.

c. Overflow-triggered reconcile (`src/watcher/mod.rs:631-650`): same
fire-and-forget pattern as (a).

So the doomed task has up to 3 in-flight blocking children that all
hold `Arc<SharedIndexHandle>` and continue mutating it.

### 5. `reconcile_stale_files` — `src/watcher/mod.rs:337-382`

```rust
pub(crate) fn reconcile_stale_files(repo_root: &Path, shared: &SharedIndex) -> usize {
    let paths: Vec<String> = {
        let index = shared.read();
        index.all_files().map(|(p, _)| p.clone()).collect()
    };

    let mut stale_count = 0usize;
    for relative_path in &paths {
        let abs_path = repo_root.join(relative_path);
        if freshen_file_if_stale(relative_path, &abs_path, shared) {
            stale_count += 1;
        }
    }
    ...
}
```

This is the smoking gun: `paths` is read from the shared (now
post-reload-B) index, but `abs_path = repo_root.join(...)` uses the
captured `repo_root` from when the doomed task was spawned (root A).

### 6. `freshen_file_if_stale` and `maybe_reindex` call chain

`src/watcher/mod.rs:293-331` `freshen_file_if_stale`:
- stats `abs_path` for mtime
- if disk mtime differs from indexed mtime, calls
  `maybe_reindex(relative_path, abs_path, shared, language)`

A subtle wrinkle: in the cross-root case, `disk_mtime = 0` because
`fs::metadata(abs_path)` fails on the non-existent A-joined-B-relative
path. Line 322-324:
```
if disk_mtime == 0 && indexed_mtime == 0 {
    return false; // both unknown — treat as fresh to avoid churn
}
```
Indexed mtime is non-zero (B just indexed it), so this guard does NOT
fire. The code falls through to `maybe_reindex` at line 330.

`src/watcher/mod.rs:216-283` `maybe_reindex`:
- `std::fs::read(abs_path)` returns `NotFound` for the bogus path
- hits the arm at line 246-251 → `shared.remove_file(relative_path)`

Confirmed call chain end-to-end.

### 7. `SharedIndexHandle` definition — `src/live_index/store.rs:433-448`

```rust
pub struct SharedIndexHandle {
    live: ArcSwap<LiveIndex>,
    write_mutex: Mutex<()>,
    published_state: ArcSwap<PublishedIndexState>,
    published_repo_outline: ArcSwap<RepoOutlineView>,
    next_generation: AtomicU64,
    git_temporal: ArcSwap<super::git_temporal::GitTemporalIndex>,
    pre_update_symbols: Mutex<HashMap<String, Vec<PreUpdateSymbol>>>,
}
```

Type alias `pub type SharedIndex = Arc<SharedIndexHandle>` at
`src/live_index/store.rs:721`.

`remove_file` is `pub fn remove_file(&self, path: &str)` at
`src/live_index/store.rs:615-637`:
```rust
pub fn remove_file(&self, path: &str) {
    let _wg = self.write_mutex.lock();
    let mut live = (*self.live.load_full()).clone();
    let path_owned = path.to_string();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        live.remove_file(path);
    }));
    match result {
        Ok(()) => self.swap_and_publish(live),
        Err(panic_info) => { tracing::error!(...); }
    }
}
```

Key observation: `remove_file` does **NOT** consult any root or
project identity. It accepts any string path, takes the write mutex,
clone-mutates-swaps, and publishes. There is no way for a caller to
distinguish "I am the current owner" vs "I am a doomed task". The
`next_generation: AtomicU64` exists but is purely an internal
versioning counter for `PublishedIndexState`; it is not used as a
fence.

### 8. `process_events` — `src/watcher/mod.rs:429-488`

Already quoted above. Refutes Mechanism B as written: events are
sourced from the same task's owned channel; there is no second
consumer. The buffered events that *do* sit on the doomed channel
refer to files under root A; even if they were drained, they would
`normalize_event_path` against root A and (in cross-root reload)
likely return `Some(relative_path)` matching A's tree — and then call
`maybe_reindex` against `A.join(rel)`, which still exists on disk for
A's files. That actually *does not destroy B's index*; it just
wastes work.

The dominant destruction path is reconcile, not events.

### 9. `normalize_event_path` — `src/watcher/mod.rs:161-187`

Used only inside `process_events`. Reconciliation does not call it —
`reconcile_stale_files` builds `abs_path` directly via
`repo_root.join(relative_path)` and never normalizes the absolute
side. Asymmetry: if `repo_root` was passed in canonical `\\?\` form
to one watcher and in stripped form to another, `repo_root.join(rel)`
yields different prefixes. The current daemon code resolves through
`canonical_project_root` (`src/daemon.rs:1724-1727`) before passing
in, so both watchers should hold the same string in steady state.

### 10. Cancellation-token patterns in the codebase

`search_text "CancellationToken"` → zero matches.
`search_text "cancel_token"` → zero matches.
`search_text "stop_signal"` → zero matches.
`search_text "generation"` in `src/watcher/` → zero matches.

There is **no existing cancellation infrastructure**.

The frecency reference the prompt called out — `cached_store_for` at
`src/live_index/frecency.rs:382-398` — is a `OnceLock<Mutex<HashMap<PathBuf,
Arc<FrecencyStore>>>>` keyed by repo_root. The store applies an
HEAD-change reset on first open per process under the cache mutex.
It is a similar **per-workspace Arc-share-and-mutate pattern**, but
without any cancellation/generation discipline either: if the
underlying DB file is moved or the workspace is reassigned, the
cache silently holds the stale Arc forever (the comment at line 386
acknowledges this: "All same-process callers for a given workspace
share the same `Arc`").

Also worth noting: there is **no `impl Drop for ProjectInstance`** in
`src/daemon.rs`. When a `ProjectInstance` is removed from the
projects map (e.g. by `index_folder_for_session` at line 504-508), the
struct goes out of scope, and the `Option<JoinHandle<()>>` watcher
task is dropped — which **does not abort** in tokio: a dropped
`JoinHandle` lets the task keep running. The current code defends
against this in only one place: `index_folder_for_session` at
`src/daemon.rs:504-508` does `abort_watcher_task(&mut watcher_task)`
on the removed project before letting it drop. But that is the same
`task.abort()` — still doesn't reach `spawn_blocking` children.

---

## 2. What `f58afbc` fixed and what it missed

`fix(daemon): fix spawn_blocking/governor races and stale PID cleanup`,
13 files changed, 777/+ 237/-.

What it *did*:
1. Added `Governor::execute_non_abortable` at
   `src/sidecar/governor.rs:410-509` and rerouted `index_folder` and
   most tool calls through it. The rationale (quoted from the diff):
   "Tokio timeouts cannot stop an already-running blocking closure,
   so releasing permits/write gates on timeout would let later
   requests overlap with still-running work." This addresses
   permit/write-gate accounting under cancellation.
2. Added PID-validated daemon cleanup (`daemon_health_matches`,
   `daemon_health_matches_recorded_pid`,
   `stop_incompatible_recorded_daemon`,
   `cleanup_daemon_runtime_files`, start-lock handling).
3. Added `pid: Option<u32>` to `DaemonHealth` and threaded it
   through.

What it *missed* (the Mechanism A surface):
- It addressed `spawn_blocking` *cancellation accounting on the
  governor side*, but did nothing for `spawn_blocking` children
  launched from inside `run_watcher` (the reconcile sweep, coupling
  refresh, overflow reconcile).
- It did not introduce any project-generation token, root-identity
  check, or cancellation signal that a doomed watcher could observe.
- It did not change `abort_watcher_task` — still `task.abort()` only.
- It did not change `ProjectInstance::reload` to wait for the prior
  watcher to drain.
- It did not change `restart_watcher` at
  `src/watcher/mod.rs:676-686`, which is the second leak source
  reachable from `SymForgeServer::index_folder`.

Conclusion: `f58afbc` was scoped to daemon lifecycle and governor
correctness. The watcher subsystem is untouched. The catastrophic
mass-removal path remains unguarded.

---

## 3. What `2b18148`'s test covers and misses

`test(live_index): guard publish/lookup atomicity across reload + root-switch`,
added `tests/live_index_publish_atomicity.rs` with 4 tests:

What it covers (from
`tests/live_index_publish_atomicity.rs`):
- `publish_then_immediate_lookup_is_consistent_load` (L103) and
  `publish_then_immediate_lookup_is_consistent_reload` (L131): after
  `LiveIndex::load` / `SharedIndexHandle::reload`, same-thread
  reads of `all_files`, `find_files_by_basename`, and `get_file`
  remain mutually consistent.
- `reload_across_different_roots_purges_prior_path_indices` (L175):
  asymmetric A/B fixtures, asserts that B's basenames resolve and
  A's basenames purge after `shared.reload(dir_b.path())`. Pinpoints
  publish atomicity of the secondary path indices.
- `publish_atomicity_stress_50_iterations` (L275): 50 reloads each
  with unique marker basenames, ensures prior markers are absent.

What it does NOT cover:
- **There is no watcher running in any of these tests.** They drive
  `SharedIndexHandle::reload` synchronously from a single thread.
  No `start_project_watcher` is spawned. No reconcile sweep runs.
- The 2026-04-24 symptom these tests guard against is the
  *publish atomicity* of basename/dir-component maps within a single
  reload. They prove: after `reload(B)`, no A-stale entries linger
  in `files_by_basename`.
- They do **not** model the new bug pattern: a *second actor* (a
  prior watcher task) holding the same `Arc<SharedIndexHandle>` and
  calling `shared.remove_file(path_from_B)` after `reload(B)`
  succeeded. The bug is not inside `LiveIndex` — it is **a
  cross-task post-reload mutation by a doomed background actor**,
  which no LiveIndex-level test can catch.

The 2b18148 author's own disposition (in the commit body) flagged
this exact gap:
> "Disposition: no repro at the LiveIndex layer. ... If the
> 2026-04-24 symptom recurs, the remaining candidates live above
> this layer — daemon session-rebind (`index_folder_for_session` /
> `ProjectInstance::reload`), proxy fallthrough ..., or Cowork-side
> MCP state caching."

The Kimi P0 finding is the materialization of exactly that "above
this layer" candidate.

---

## 4. Proposed fix shape (high-level only)

Two layers of defense are needed. Either alone leaves a hole.

### Layer 1 — Cooperative cancellation of the watcher loop

Replace fire-and-forget `task.abort()` with a cooperative stop
signal.

Concretely:
- Add a `stop_token: Arc<AtomicBool>` (or
  `tokio_util::sync::CancellationToken`, which the codebase doesn't
  currently use) owned by `ProjectInstance` and `cloned` into the
  watcher closure.
- `run_watcher` polls the token between every event batch, before
  every reconcile sweep, and inside `reconcile_stale_files`'s loop
  over paths so the sweep aborts mid-flight when the project is
  reloaded.
- `reload()` (and the `index_folder_for_session` reassignment path)
  first signals the token, then either waits on the JoinHandle with
  a bounded timeout (so the doomed task has a chance to exit its
  next iteration cleanly) or accepts that a still-in-flight
  blocking call must complete before mutation stops — but tags every
  Arc clone passed into that call with the same token so
  `reconcile_stale_files` can also short-circuit.

Where this changes:
- `ProjectInstance` struct (`src/daemon.rs:82-97`): add
  `stop_token`.
- `ProjectInstance::activate` / `::reload` / removal path in
  `index_folder_for_session`: signal before re-spawn.
- `run_watcher` signature: take the token, check at every loop
  boundary.
- `reconcile_stale_files` signature: take a `should_stop: &dyn Fn() -> bool`
  or token clone; check before each file.
- `restart_watcher` (`src/watcher/mod.rs:676-686`) and the
  `SymForgeServer::index_folder` path: also produce/consume a stop
  token, or this leak surface remains.

### Layer 2 — Project-generation fence at the index mutation boundary

Even with Layer 1, a doomed `spawn_blocking` reconcile may have read
`paths` before the cancellation arrives and then start removing.
Defense in depth says the index itself should reject mutations from
a tagged stale producer.

Concretely:
- Add a `project_generation: AtomicU64` field on `SharedIndexHandle`
  (or carry a `project_id: String`).
- Each call to `SharedIndexHandle::reload` bumps the generation.
- The watcher captures the generation at task-spawn time. Every
  `shared.remove_file`, `shared.update_file`, `shared.touch_mtime`
  call from inside the watcher checks the generation against the
  one captured at spawn. Mismatch → no-op + warn-once telemetry.

The simplest API change is to introduce
`SharedIndexHandle::remove_file_at_generation(rel_path, expected_gen)`
that takes the write mutex, re-reads the generation under the lock,
and short-circuits if it has advanced. Same for `update_file` and
`touch_mtime`. Public `remove_file` keeps the existing semantics for
non-watcher callers.

### Layer 3 (minor) — Read-then-existence-recheck before removal

Even in the single-watcher steady-state case, `NotFound` should not
be an irreversible verdict. Before calling `shared.remove_file`,
`maybe_reindex` should:
1. Sleep briefly (e.g. 50ms) and re-stat with `fs::metadata`. AV/IDE
   locks usually clear in tens of milliseconds; a permanently
   deleted file will still be missing.
2. Verify the absolute path is still under the *current* project
   root before removal (which it can do by asking the
   `SharedIndexHandle` for its current generation/root — which only
   works once Layer 2 exists).

This Layer-3 mitigation also addresses Mechanism C.

### Changes in `reload()`

After the fix shape, `ProjectInstance::reload` (`src/daemon.rs:1070`)
should:
1. Signal `self.stop_token` (Layer 1).
2. (Optional) `let _ = handle.await_timeout(small);` to drain the
   doomed task synchronously when possible.
3. `self.index.bump_generation_and_reload(canonical_root)?`
   (Layer 2).
4. Spawn the new watcher with a *new* `stop_token` and the *new*
   generation captured by value.

---

## 5. Risk — false-positive removal even without races

Even with a single, well-behaved watcher and no reload churn,
`maybe_reindex` can wrongly remove a file because:

a. **Antivirus / Indexer / IDE exclusive lock** (Mechanism C): on
Windows, AV scans and `git status` invocations briefly open files
for read with `FILE_SHARE_NONE`. A concurrent `std::fs::read` may
fail with `NotFound` *or* `PermissionDenied` depending on the
specific transient state. The current code at
`src/watcher/mod.rs:243-251` does not distinguish: it only acts on
`NotFound`, but Windows' OS surfaces this state inconsistently.

b. **MAX_PATH (260) without `\\?\` extension**: when the indexed
relative path is constructed via `discovery` and stored as a
forward-slash string, and the watcher then constructs
`abs_path = repo_root.join(relative_path)`, the resulting `PathBuf`
may exceed 260 chars without the `\\?\` prefix.
`std::fs::read` on Windows transparently retries with the extended
prefix in recent stdlib, but older toolchains and some FS targets
(network shares, certain UNC mounts) still return `NotFound`. The
codebase does not opt in long-path support globally; see
`src/watcher/mod.rs:165` `r"\\?\"` strip only on the input side.

c. **Case mismatch / unicode normalization**: the indexed
`relative_path` is in whatever case the original walk yielded. On a
case-insensitive FS, a later `Modify` event may carry a different
case; `normalize_event_path` strips and slash-normalizes but does
not lower-case. Subsequent `reconcile_stale_files` builds
`repo_root.join("Path/With/Mixed/Case.rs")`; on case-insensitive
NTFS this works, but on a network mount or remote case-sensitive
filesystem it can fail.

d. **TOCTOU between `discover_all_files` and first `maybe_reindex`**:
a file present at index time, then deleted before the first
reconcile tick 30 seconds later, will be removed. This is the
"correct" behavior — but if the user undid the deletion in the
meantime, the watcher's removal is sticky because the watcher only
re-indexes files already in the index; it does not actively
discover new files between reconciles.

e. **`disk_mtime == 0` short-circuit interaction**
(`src/watcher/mod.rs:322`): when `fs::metadata` itself fails (not
just `NotFound`), `disk_mtime` becomes 0 — but `indexed_mtime` is
not 0, so the guard at line 322 doesn't short-circuit. The code
falls through to `maybe_reindex` and *will* attempt the read; if
`fs::metadata` failed because of a transient lock, `fs::read`
likely fails the same way. The result is still a `remove_file`
call.

Layer 3 above (re-stat with brief sleep) is the only durable defense
against most of these.

---

## 6. Reproduction scenario for a regression test

The bug has two distinct repro shapes.

### Shape A — Single-watcher false-positive removal
Goal: catch Mechanism C (and case (e) above) without reload churn.

Test outline (Rust integration test):
1. Tempdir with N >= 50 files across multiple subdirs.
2. `let index = LiveIndex::load(tmp).unwrap();`
   `let info = Arc::new(Mutex::new(WatcherInfo::default()));`
   spawn `run_watcher(tmp.clone(), Arc::clone(&index), info)`.
3. Trigger a synthetic locking event on one file: open it with
   `OpenOptions::new().share_mode(0).open(...)` on Windows (using
   the `winapi`/`windows-sys` crate gated to `#[cfg(windows)]`).
   Briefly modify-and-touch a different file under the same dir to
   force a debounce event that brings that file into the reconcile
   set.
4. Wait > `SYMFORGE_RECONCILE_INTERVAL` (env-set to 1 second for
   the test). The reconcile sweep will run; the locked file may
   resolve `NotFound` transiently.
5. Release the lock, then re-read the index. Assert all original
   files are still present.

This test would *currently fail* if the AV-lock pattern triggered
removal; it would *pass* once Layer 3 is implemented.

Note: this is platform-sensitive. On Linux a flock isn't observable
the same way; on Windows the test should use `FILE_SHARE_NONE`. Mark
the test `#[cfg(windows)]` or write the broader version using a
shimmed filesystem.

### Shape B — Reload-induced cross-root destruction
Goal: catch Mechanism A directly.

Test outline:
1. Build two tempdirs A and B with disjoint relative-path sets
   (e.g. A has `crate_a/src/lib.rs`, B has `crate_b/src/lib.rs`).
2. Construct a single `Arc<SharedIndexHandle>` and a single
   `WatcherInfo`.
3. Spawn watcher with `repo_root = A`. Drive a `LiveIndex::load(A)`
   into the shared handle first.
4. Set `SYMFORGE_RECONCILE_INTERVAL=1` to make the reconcile loop
   sweep frequently.
5. Call the equivalent of `ProjectInstance::reload(B)`:
   `index.reload(B)` followed by `abort_watcher_task(handle); spawn
   run_watcher(B, ...)`. (Or use the high-level
   `index_folder_for_session` path against a daemon harness — but a
   pure-unit test is cheaper.)
6. Wait > 2 seconds (so the doomed reconcile sweep can run).
7. Assert `index.published_state().file_count` equals the count B
   was loaded with, **and** assert every B file is still reachable
   via `index.get_file(path)`.

Today this test would observe the file count drop toward zero and
fail. Once Layer 1 + Layer 2 are implemented, the doomed task either
exits before its sweep starts or is rejected by the
`expected_gen` check.

### Bonus: deterministic variant without spawn_blocking timing

Because `spawn_blocking` timing is hard to make deterministic in
unit tests, a more focused unit test for Layer 2 would be:
1. Construct `SharedIndexHandle`, load A.
2. Capture `gen_a = shared.current_generation()`.
3. Reload to B: `shared.reload(B)` (which bumps generation to
   `gen_b`).
4. Call (directly, on the test thread)
   `shared.remove_file_at_generation("crate_a/src/lib.rs", gen_a)`
   — the doomed-task removal API.
5. Assert the call was a no-op and `shared.get_file(...)` for B's
   files is unchanged.

This isolates the API contract from any timing or task-scheduling
variable.

---

## 7. Related code paths worth refactoring

Any code path that holds an `Arc<SharedIndexHandle>` and can mutate
it after a project root change deserves audit. Candidates already in
the codebase:

1. **`src/protocol/tools.rs:4438-4444`
   `SymForgeServer::index_folder`** — calls
   `crate::watcher::restart_watcher(...)` without aborting the
   prior watcher at all. Worse than `ProjectInstance::reload`. Same
   shared Arc semantics. Same fix shape needed here.

2. **`src/watcher/mod.rs:676-686` `restart_watcher`** — by design
   leaks any prior watcher task. Either remove the function in
   favor of a single supervised reload path or have it accept a
   `stop_token` and signal the previous one.

3. **`src/protocol/tools.rs:1659-1673`
   `freshen_exact_path_for_targeted_retrieval`,
   `src/protocol/tools.rs:1687-1703` `prepare_exact_path_for_edit`,
   `src/protocol/tools.rs:1705-1728` `prepare_batch_paths_for_edit`,
   `src/sidecar/handlers.rs:180-195`
   `freshen_sidecar_path_if_stale`** — all call
   `watcher::freshen_file_if_stale(rel, abs, &server.index)` with
   `abs` constructed from a server-/state-held `repo_root`. If a
   session's `repo_root` is updated by a concurrent
   `index_folder` (via `set_repo_root` in
   `src/protocol/mod.rs:158`) but a prior request still holds the
   old captured root, this path triggers the same
   `NotFound → shared.remove_file` chain *per-request*. Less
   catastrophic than the watcher loop because it removes one file
   per stale request, but the same root cause.

4. **`src/live_index/coupling/mod.rs`
   `refresh_on_reconcile_tick`** — invoked from the watcher in the
   same fire-and-forget `spawn_blocking` style. Less directly
   destructive (it does not call `remove_file`), but the same
   doomed-task lifetime applies and the per-workspace guard
   mentioned in the run_watcher comment should be audited for
   correct behavior across generation bumps.

5. **`src/live_index/git_temporal::spawn_git_temporal_computation`**
   — called from `ProjectInstance::reload` (`src/daemon.rs:1102`)
   on each reload. A prior in-flight git-temporal computation
   holding `Arc::clone(&self.index)` may publish into
   `SharedIndexHandle::git_temporal` *after* the next reload
   completes, with stale data for the wrong root. The publish API
   for `git_temporal` is an `ArcSwap::store`, so the failure mode
   is "stale temporal data in the index" rather than "files
   removed" — less severe, but the same architectural pattern.

6. **`src/live_index/frecency.rs:382-398` `cached_store_for`**
   keeps `Arc<FrecencyStore>` per repo_root forever. If a daemon
   ever migrates or moves a workspace, the cache holds a stale
   Arc. Lower severity but worth a TODO marker once a
   generation/cleanup pattern lands.

A consistent project-generation token spanning watcher, coupling,
git-temporal, and frecency caches would address all of these in one
move.

---

## Notes and uncertainties

- I could not exercise the bug at runtime to confirm exact event
  timing; the path-of-destruction analysis is from static reading
  alone. **Needs runtime evidence:** whether `tokio::task::abort()`
  on the parent prevents the parent from `await`ing the next
  `spawn_blocking` reconcile cycle (in which case fewer reconcile
  sweeps run by the doomed task) vs. whether the parent reaches the
  `last_reconcile.elapsed() >= ...` check before being cancelled. In
  practice the 30-second default reconcile interval and the
  observed ~30s collapse cadence strongly suggest at least one
  doomed reconcile sweep ran post-abort, but a runtime confirmation
  via tracing-level logs in a controlled repro would close this
  gap.
- The evaluator's quoted Rust snippet for the `NotFound` arm
  matches the actual `src/watcher/mod.rs:243-251` arm exactly,
  including comment-free form. They quoted the real code.
- The evaluator's wording around Mechanism B ("After
  `spawn_blocking` returns, `try_recv` may pull buffered `Remove`
  events processed against the live `SharedIndex`") is consistent
  with the actual loop structure but does not match a real failure
  mode because the buffered events live on the doomed task's own
  channel, not a shared one. Refuted with file:line evidence at
  `src/watcher/mod.rs:391-396` (handle owned per-task) and the
  channel construction at `src/watcher/mod.rs:411`.
- The function name `reconcile_stale_files` matches the
  evaluator's. The names `freshen_file_if_stale` and `maybe_reindex`
  also match. No naming discrepancies found.
