---
type: design
title: Tentacle 3 Phase 3.1 — Coupling Store Lifecycle
created: 2026-04-18
revised: 2026-04-18
tentacle: T3
phase: 3.1
parent_plan: docs/plans/cochange-coupling-execution.md
adr: docs/decisions/0013-coupling-signal-contract.md
status: revised per reviewer feedback — per-workspace guard, guard covers boot-init, non-git vs no-HEAD split, synchronous test helper
---

# Phase 3.1 — Coupling Store Lifecycle (design)

Strict scope: make the per-workspace coupling store open, cold-build, and stay current with HEAD. **Nothing reads the store yet** — no `RankCtx`, no `capture_search_files_view`, no `SearchFilesHit` population, no user-visible ordering change. Enabling `SYMFORGE_COUPLING=1` must produce only on-disk state (`.symforge/coupling.db`) and log lines; disabling it must produce exactly the behaviour of today's `main` branch.

## Reuse vs new

| Concern | Frecency precedent | Phase 3.1 choice |
|---|---|---|
| Per-workspace DB path constant | `SYMFORGE_FRECENCY_DB_PATH` in `src/paths.rs` | Add `SYMFORGE_COUPLING_DB_PATH` sibling |
| Same-process cache | `cached_store_for` in `frecency.rs` using `OnceLock<Mutex<HashMap<PathBuf, Arc<FrecencyStore>>>>` | Mirror shape, but store is already `Clone` (wraps `Arc<Mutex<Connection>>`) so use `HashMap<PathBuf, CouplingStore>` and return a cloned handle |
| Boot-path init | `init_frecency_store(project_root)` in `persist.rs:370`, called once from `LiveIndex::load` at `store.rs:1090`. Runs HEAD check synchronously; cheap | `init_coupling_store(project_root)` callsite identical, but the **work** spawns onto a background thread — calibration shows 2–7s cold-build, unacceptable on boot path |
| HEAD-change-across-sessions | Frecency reads HEAD at boot, applies graduated reset | Coupling opens store at boot, branches on `cold_built_at`: `None` → `cold_build`; `Some` → `apply_head_delta` with pre-HEAD-check |
| HEAD-change-within-session | **None exists.** Frecency is boot-only | **New lifecycle work.** Piggyback on the watcher's existing 30 s reconcile tick at `watcher/mod.rs:566-578`. Add one `spawn_blocking` alongside `reconcile_stale_files` that calls a coupling refresh. Inherits the `SYMFORGE_RECONCILE_INTERVAL=0` kill-switch for free |
| Error handling | Drop all errors silently — "must never crash the live-index boot path" | Same policy. Every entry point returns `()` or `Option`; the store being missing, git failing, or SQLite rolling back must not propagate |
| Env gate | `SYMFORGE_FRECENCY=1` | `SYMFORGE_COUPLING=1`. Hook-adoption telemetry pattern mirrors frecency — gate at call time, not at registration time |

**What is genuinely new (not a frecency copy):**

1. **Background boot thread** — frecency doesn't need one. Cold-build can take seconds; init returns immediately after spawning.
2. **Reconcile-tick piggyback** — frecency has no mid-session HEAD handling at all. We add one.
3. **Per-workspace in-flight guard covering BOTH entry points.** The daemon runs a watcher per project (`src/daemon.rs:280, :1020`); a single global guard would let workspace A suppress workspace B's refresh. Boot-init spawns work that can still be running when the watcher at `src/daemon.rs:973` starts its first 30-second reconcile tick at `src/watcher/mod.rs:544`, so the same guard must protect both `init_coupling_store`'s background thread **and** every `refresh_on_reconcile_tick` call. Shape: `HashMap<PathBuf, Arc<AtomicBool>>` keyed by canonical project root, lazily populated.
4. **Cheap HEAD-unchanged pre-check** — `apply_head_delta` currently walks up to 500 commits via `compute_window` **before** its NoOp fast path. The tick calls it every 30 s, so we must short-circuit with a `git::head_sha(root)? == store.last_head()?` check before invoking delta. Otherwise every idle tick walks the git history to confirm nothing changed.
5. **Synchronous internal helper** for testability — `run_init(db_path, repo_root)` mirroring frecency's `run_frecency_init` pattern. Tests invoke it directly; production callers spawn it on a thread. Avoids thread-join dances in tests.

## Placement

### File layout

Create `src/live_index/coupling/lifecycle.rs` for the new surface. Rationale: `persist.rs` is already 1850 lines with a partial-parse diagnostic; the coupling lifecycle is richer than frecency's (background thread + tick hook + guard), so colocating it with the coupling module keeps the blast radius local.

Public exports from `src/live_index/coupling/mod.rs`:

```rust
pub mod lifecycle;
pub use lifecycle::{init_coupling_store, refresh_on_reconcile_tick};
```

**Store caching deferred to 3.2.** The design originally proposed a same-process `cached_store_for(project_root) -> Option<CouplingStore>` mirroring frecency's `OnceLock<HashMap<PathBuf, _>>` pattern. Phase 3.1 has no external consumers (nothing reads the store yet), so the cache would be dead code. `run_init` opens the store each call; the per-workspace in-flight guard already serialises access. Add the cache in 3.2 when the first reader (store query helpers) appears.

### Boot-path callsite

`src/live_index/store.rs:1090` — one new line adjacent to the frecency init:

```rust
super::persist::init_frecency_store(root);
super::coupling::lifecycle::init_coupling_store(root);
```

Order matters: frecency is synchronous and cheap; coupling spawns a thread. Running frecency first keeps the established boot ordering intact for any frecency tests that assume synchronous completion.

### Reconcile-tick hook

`src/watcher/mod.rs:566-578` — inside `run_watcher`'s reconcile branch, add one `spawn_blocking` alongside the existing `reconcile_stale_files` call:

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

    // NEW: coupling refresh — separate task so a slow coupling delta
    // never delays stale-file reconciliation.
    let root_for_coupling = repo_root.clone();
    tokio::task::spawn_blocking(move || {
        crate::live_index::coupling::lifecycle::refresh_on_reconcile_tick(&root_for_coupling);
    });

    last_reconcile = Instant::now();
}
```

Separate `spawn_blocking` so the two tasks don't serialise. Each is independently best-effort.

`refresh_on_reconcile_tick` gates on `SYMFORGE_COUPLING=1` internally — with the flag off, the body is a bare `return` and the new spawn costs one tokio task allocation every 30 s. Acceptable overhead for keeping the wiring in place by default.

## API shape

```rust
// src/live_index/coupling/lifecycle.rs

/// Boot-path entry point. Idempotent.
///
/// With `SYMFORGE_COUPLING=1` AND project_root is inside a git repo:
///   1. Acquire the per-workspace in-flight guard (try-lock). If another
///      refresh is already running for this workspace, return immediately.
///   2. Spawn a background thread that:
///        a. Opens (or creates) the per-workspace coupling store.
///        b. Calls the synchronous helper `run_init(db_path, repo_root)`.
///        c. Releases the per-workspace guard on completion / panic.
///   3. Return immediately.
///
/// With the flag unset OR no git repo at project_root: no-op, no thread,
/// no DB touch, no guard acquired.
pub fn init_coupling_store(project_root: &Path);

/// Reconcile-tick entry point. Called every `SYMFORGE_RECONCILE_INTERVAL`
/// seconds from `run_watcher`.
///
/// With the flag unset: no-op.
/// With the flag on:
///   1. Acquire the per-workspace in-flight guard (same guard as boot-init).
///      If held, return — either cold-build is still running or the prior
///      tick's delta is still in flight.
///   2. Cheap `git::head_sha` read; compare against `store.last_head()`.
///      Equal → release guard and return.
///   3. Call `run_init` (which internally routes to delta), release guard
///      on completion / panic.
pub fn refresh_on_reconcile_tick(project_root: &Path);

/// Synchronous init body — the unit-of-work both boot-init and the
/// reconcile tick ultimately perform. Tests call this directly.
///
/// Contract:
///   * Open the store at `db_path`.
///   * If `cold_built_at` is None → run `cold_build`.
///   * Else if `git::head_sha(repo_root) == store.last_head()` → no-op.
///   * Else → run `apply_head_delta`.
///   * All errors are returned; callers (background thread / tick) drop
///     them silently with a single `debug!` line.
pub(crate) fn run_init(db_path: &Path, repo_root: &Path) -> Result<(), String>;
```

### Per-workspace in-flight guard

```rust
fn guard_for(project_root: &Path) -> Arc<AtomicBool> {
    use std::collections::HashMap;
    use std::sync::{Arc, OnceLock};
    static GUARDS: OnceLock<Mutex<HashMap<PathBuf, Arc<AtomicBool>>>> = OnceLock::new();
    let map = GUARDS.get_or_init(|| Mutex::new(HashMap::new()));
    let key = project_root.to_path_buf(); // canonicalised by the caller
    let mut g = map.lock().expect("coupling guard map poisoned");
    Arc::clone(g.entry(key).or_insert_with(|| Arc::new(AtomicBool::new(false))))
}
```

Acquisition pattern (reused by both entry points):

```rust
let guard = guard_for(project_root);
if guard.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
    return; // another refresh owns this workspace
}
let _release = GuardRelease(&guard);  // inline RAII, ~5 lines at module scope

// ... work ...

struct GuardRelease<'a>(&'a AtomicBool);
impl Drop for GuardRelease<'_> {
    fn drop(&mut self) { self.0.store(false, Ordering::SeqCst); }
}
```

Inline RAII over pulling `scopeguard` — one struct + one `Drop` impl, not worth a dependency.

**Coverage:** the guard is acquired by `init_coupling_store` **before** spawning its thread and released inside the thread's completion path. The reconcile tick acquires the same guard. A slow first cold-build therefore blocks every tick for its workspace until it finishes; ticks for *other* workspaces are unaffected. This closes the race between boot-init and the first 30-second reconcile tick noted at `src/watcher/mod.rs:544`.

**Spawn-failure path.** `init_coupling_store` must use `std::thread::Builder::spawn` (not `thread::spawn`, which panics on OS thread-limit exhaustion). On `Err` from the builder: release the guard immediately, emit one `debug!` line, return. The workspace must not wedge silently with a permanently-held guard just because the OS refused a thread.

```rust
// Acquire guard.
if guard.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
    return;
}

let guard_clone = Arc::clone(&guard);
let db_path = project_root.join(crate::paths::SYMFORGE_COUPLING_DB_PATH);
let repo_root = project_root.to_path_buf();

let spawn_result = std::thread::Builder::new()
    .name("coupling-init".into())
    .spawn(move || {
        let _release = GuardRelease(&guard_clone);
        debug!("coupling init: starting");
        match run_init(&db_path, &repo_root) {
            Ok(()) => debug!("coupling init: ok"),
            Err(e) => debug!("coupling init: failed: {e}"),
        }
    });

if let Err(e) = spawn_result {
    guard.store(false, Ordering::SeqCst); // release — no thread will do it for us
    debug!("coupling init: spawn failed: {e}");
}
```

Tested via test 7 extension (or a new test 13) — inject a spawn failure is awkward in unit tests, but the guard-release branch is small enough to cover via direct code inspection + one test that drives the synchronous `run_init` while asserting the guard returns to `false` after the call.

**Key canonicalisation:** callers pass `project_root` after `.canonicalize()`-style normalisation; the watcher already does this (`src/watcher/mod.rs::normalize_event_path`). Without canonicalisation, `/proj` and `/proj/./` would map to different guards.

## Failure semantics

Every failure mode reduces to "coupling store is not current; no user-visible effect". Log budget for 3.1: **at most one `debug!` at start, one `debug!` at completion, one `debug!` at failure** per refresh cycle — no progress meter. Errors are dropped; nothing escalates to `error!`.

| Case | Behaviour |
|---|---|
| `SYMFORGE_COUPLING` unset or `!= "1"` | Full no-op. No DB file created, no thread spawned, no git read, no guard entry created |
| `SYMFORGE_COUPLING=1` but **no git repo** (no `.git` at or above `project_root`) | Full no-op. `init_coupling_store` detects missing repo via `git::head_sha` / `git2::Repository::discover` **before** opening the store. No `coupling.db` created. If the user later runs `git init`, the next boot will cold-build as normal |
| `SYMFORGE_COUPLING=1`, git repo present, **no commits yet** (no HEAD) | Store **is** opened. Cold-build runs via `compute_window`, which handles no-HEAD by purging rows and setting `cold_built_at = Some(now)`, `last_head = None`. Subsequent ticks will pick up the first commit when it lands |
| `SYMFORGE_COUPLING=1` but `.symforge/` unwriteable | `CouplingStore::open` returns Err; silent drop; one `debug!` line. Guard releases. Retry next session |
| First session, successful cold-build | `cold_built_at` goes from `None` → `Some(now)`; `last_head` set. Subsequent boot sees `cold_built_at = Some` and routes to delta |
| First session, cold-build fails mid-way (git2 error, SQLite error) | Transaction rolls back atomically (already guaranteed by `commit_cold_build`). `cold_built_at` stays `None`. Next session retries cold-build from scratch |
| Repeat session, same HEAD | `run_init` branch: `cold_built_at = Some`, pre-check finds `git::head_sha == store.last_head()` → returns Ok without calling delta. Net cost: one git ref read per boot, one SQLite read for `last_head` |
| Repeat session, HEAD moved | Delta path: pre-check mismatches → `apply_head_delta` runs → ledger subtracts evicted commits, adds new ones |
| Mid-session HEAD move (user runs `git checkout`) | Within ≤ 30 s, reconcile tick fires → `refresh_on_reconcile_tick` → pre-check mismatches → delta runs |
| Tick fires while prior refresh still running (boot cold-build or previous delta) | Per-workspace guard `compare_exchange` fails; tick skips without walking git. No queueing. Next tick in 30 s retries. Does **not** affect other workspaces' ticks |
| Store file corrupted or schema mismatch | `CouplingStore::open` returns Err; silent drop. Admin must delete `.symforge/coupling.db` to recover. Acceptable — matches frecency's behaviour |
| `SYMFORGE_RECONCILE_INTERVAL=0` | Reconcile branch disabled; mid-session HEAD moves are NOT tracked by the tick path. Boot-time init still catches HEAD moves across sessions. Documented tradeoff — revisit when 3.3 makes coupling user-visible; not in 3.1 scope |

## Test plan

Tests land in `src/live_index/coupling/lifecycle.rs` under `#[cfg(test)] mod tests` — colocated with the code, mirroring frecency's pattern.

**Testability pattern (mirrors frecency).** Tests drive the synchronous `run_init(db_path, repo_root)` helper and the guard primitives directly, not the public `init_coupling_store` / `refresh_on_reconcile_tick` wrappers. No thread joins, no `sleep`s, no polling. The wrappers are thin: env-flag check, guard acquire, spawn_blocking → `run_init`. Tests of the wrappers themselves are limited to wrapper-only concerns (env gate, guard skip). A shared `FRECENCY_ENV_LOCK`-style `COUPLING_ENV_LOCK` mutex serialises env-var mutation in tests.

| # | Test | Drives | Assertion |
|---|---|---|---|
| 1 | `public_init_is_noop_when_flag_unset` | `init_coupling_store` | No `.symforge/coupling.db` created; guard map has no entry for project_root |
| 2 | `public_init_is_noop_on_non_git_project` | `init_coupling_store` | Temp dir without `.git`; flag on; no `coupling.db` created; no guard entry for project_root in the guard map. Outcome-based only — do not attempt to observe thread-count side effects |
| 3 | `run_init_cold_builds_on_first_session` | `run_init` | Multi-commit repo; after call: `cold_built_at` is `Some`, `last_head` matches HEAD, row count > 0 |
| 4 | `run_init_is_noop_on_repeated_head` | `run_init` | Seed via prior `run_init`; capture `cold_built_at` and row-count snapshot; second call; assert both unchanged AND `last_reference_ts` unchanged. Enforces advisor's "cold-build must NOT re-run" bar |
| 5 | `run_init_applies_delta_on_head_move` | `run_init` | Seed at commit A; add commit B; second call; `last_head` advances to B, new rows reflect B's files, `cold_built_at` unchanged |
| 6 | `run_init_handles_empty_repo_no_head` | `run_init` | `git init` with no commits; call; `cold_built_at` is `Some`, `last_head` is `None`, row count is zero. **Non-git case is test 2; this is the distinct no-HEAD case** |
| 7 | `run_init_reports_err_on_unwriteable_dir` | `run_init` | Make `.symforge/` a file (not a directory); call; assert `Err` returned (wrapper drops it, but the helper surfaces it) |
| 8 | `refresh_on_tick_is_noop_when_flag_unset` | `refresh_on_reconcile_tick` | Flag cleared; call; no DB touch (verify via no `coupling.db` creation in a fresh temp) |
| 9 | `refresh_on_tick_shortcircuits_on_unchanged_head` | `refresh_on_reconcile_tick` | Seed via `run_init` at commit A; capture `last_reference_ts`; call tick; assert `last_reference_ts` unchanged (delta always bumps ref_ts, so stability proves delta was skipped) |
| 10 | `refresh_on_tick_applies_delta_when_head_moved` | `refresh_on_reconcile_tick` | Seed at A; advance HEAD to B; call tick; `last_head` becomes B |
| 11 | `guard_skips_when_held` | `guard_for` + `init_coupling_store` + `refresh_on_reconcile_tick` | Acquire guard for project_root manually; call both entry points; assert neither mutates the store (inspect `cold_built_at` / `last_reference_ts`); release guard; re-call; assert normal behaviour resumes |
| 12 | `guard_is_per_workspace` | `guard_for` | Acquire guard for project_root_A; acquire guard for project_root_B; both `compare_exchange` calls succeed (different `Arc<AtomicBool>` instances). Proves workspace A cannot block workspace B |

Tests 1–2 and 8 exercise wrapper-only concerns (env gate, non-git detection). Tests 3–7 exercise `run_init`'s branching logic. Tests 9–10 exercise the tick's short-circuit and delta dispatch. Tests 11–12 exercise the per-workspace guard directly. Together they cover every row in the failure-semantics table.

**What is explicitly NOT tested in 3.1:**
- End-to-end wiring from the watcher tick to the refresh call (integration-test territory — defer to 3.3 when a user-visible consumer exists and behaviour can be asserted against query outcomes).
- Query-path reads of the store (no reader exists yet).
- Ranker-fusion behaviour (phase 3.3 scope).
- Performance of the reconcile-tick coupling refresh under sustained churn (phase 3.6 perf pass).

## Touched files

| File | Change |
|---|---|
| `src/paths.rs` | Add `pub const SYMFORGE_COUPLING_DB_PATH: &str = ".symforge/coupling.db";` |
| `src/live_index/coupling/mod.rs` | Add `pub mod lifecycle;` and `pub use` re-exports for `init_coupling_store`, `refresh_on_reconcile_tick` |
| `src/live_index/coupling/lifecycle.rs` | **NEW** — `init_coupling_store`, `refresh_on_reconcile_tick`, `run_init`, `guard_for` (per-workspace `HashMap<PathBuf, Arc<AtomicBool>>`), `GuardRelease` RAII struct, tests 1–12. Same-process `cached_store_for` deferred to 3.2 when the first reader appears |
| `src/live_index/store.rs` | Single-line addition at `:1090` — call `init_coupling_store` after `init_frecency_store` |
| `src/watcher/mod.rs` | Single-block addition inside `run_watcher`'s reconcile branch (lines ~566–578) — a second `spawn_blocking` that calls `refresh_on_reconcile_tick`. Leave all existing stale-file logic untouched |
| `docs/plans/cochange-coupling-execution.md` | Mark 3.1 as in-progress; link to this design doc |

Nothing else. Explicitly **not** touched: `rank_signals.rs`, `query.rs`, `protocol/tools.rs`, `protocol/format.rs`, `SearchFilesHit`, any test under `tests/rank_signal_behavior.rs`. If the diff shows a change to any of those, it's a scope breach — block the PR.

## Reviewer decisions (resolved)

1. **Guard scope:** per-workspace `HashMap<PathBuf, Arc<AtomicBool>>` from day one. Global guard rejected — daemon already runs concurrent workspaces via `src/daemon.rs:280, :1020`.
2. **Guard coverage:** both entry points. Boot-init acquires before spawning; reconcile tick acquires before any work. Prevents the first-tick-vs-cold-build race at `src/watcher/mod.rs:544`.
3. **Non-git vs no-HEAD split:** resolved. No git repo → full no-op, no DB file. Git repo with no commits → open store, run cold-build (purges), set `cold_built_at`. Tests 2 and 6 enforce both cases distinctly.
4. **RAII release:** inline `GuardRelease` struct, no new crate dependency.
5. **Reconcile interval:** no separate coupling env var in 3.1. Stays tied to `SYMFORGE_RECONCILE_INTERVAL`. Revisit when 3.3 makes coupling user-visible.
6. **Progress logging:** no progress meter. One `debug!` at start, one `debug!` at completion/failure. Nothing else.
7. **Testability:** synchronous `run_init(db_path, repo_root)` helper is the unit of work. Public wrappers are thin (env gate + guard + spawn). Tests drive `run_init` directly — no thread-join, no sleep, no polling.

## Next

Sign-off on this revision → implement → tests green → advisor review → reviewer review → land.
