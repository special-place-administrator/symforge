# ADR 0014  Watcher-Subsystem Spawn-Blocking Discipline

**Status:** Accepted (2026-05-15)
**Supersedes:** none
**Related:** Phase H plan 2026-05-12, ADR 0013

## Context

Phase H fixed B-P0-1, where stale watcher-side blocking work could keep running after `index_folder` or `ProjectInstance::reload` moved the shared index to a new project root. The destructive case was:

- A watcher task captured root A.
- A later reload replaced the shared index with root B's file set.
- The stale task read B-relative paths from the shared index, joined them against root A, saw `NotFound`, and removed valid B files from the shared index.

The same mutation surface also had a separate transient-absence failure mode: Windows AV, IDE, build-tool, or checkout activity can briefly report `NotFound` for a file that still exists. Treating the first `NotFound` as a confirmed deletion causes false removals.

The Phase H plan, `docs/plans/2026-05-12-symforge-stability-hotfix.md`, section "Failure-mode coverage matrix (Tier 1 design rationale)", records why the fix is layered. No single layer covers all relevant failure modes:

- Cancellation limits wasted work and closes most stale-task windows, but an already-running blocking closure can still slip past a check.
- Generation fencing is the load-bearing index-boundary guard, but it rejects mutations after the work was already spent.
- Re-stat-on-NotFound handles transient file-system absence, but it does not protect against a stale producer whose A-era path still exists.

This ADR codifies that Phase H convention as reviewer guidance for future watcher-subsystem `spawn_blocking` mutation sites. The convention is deliberately not a lint or CI rule today.

## Decision

New watcher-subsystem `spawn_blocking` sites that can mutate the live index, watcher-derived persisted state, or publication surfaces must follow the three-layer convention unless the PR documents an explicit exemption:

1. Thread the watcher cancellation token into the blocking closure and check it before and during bounded loops.
2. Capture the expected project generation before spawning and consume generation-fenced APIs at the mutation boundary.
3. When a `NotFound` result can lead to `remove_file`, use the bounded re-stat-on-NotFound retry before treating absence as confirmed.

The landed Phase H API surface that makes this convention concrete is:

- `src/live_index/store.rs::SharedIndexHandle::current_project_generation`
- `src/live_index/store.rs::SharedIndexHandle::remove_file_at_generation`
- `src/live_index/store.rs::SharedIndexHandle::update_file_at_generation`
- `src/live_index/store.rs::SharedIndexHandle::touch_mtime_at_generation`
- `src/live_index/store.rs::SharedIndexHandle::update_git_temporal_at_generation`
- `src/live_index/store.rs::SharedIndexHandle::note_rejected_stale_mutation`

### Layer 1  Cancellation token

Watcher-owned blocking work must receive the current watcher stop token. In the current implementation this token is an `Arc<AtomicBool>` passed through `src/watcher/mod.rs::run_watcher_with_stop`; an equivalent cancellation primitive is acceptable if it preserves the same observable behavior.

For blocking closures:

- Check the token before doing work inside the closure.
- Check it between file paths, debounced events, and other bounded loop entries.
- Signal the prior token before replacing or dropping a watcher, as done by `src/daemon.rs::ProjectInstance::reload`, `src/daemon.rs::DaemonState::index_folder_for_session`, and watcher restart paths.

Layer 1 is not a substitute for Layer 2. A closure can pass a cancellation check and then race with reload before its mutation call. Layer 1 exists to stop doomed tasks promptly and reduce blocking-pool pressure.

Exempt cases:

- Read-only `spawn_blocking` sites for telemetry, diagnostics, metrics, or pure retrieval.
- Long-running computations whose only mutation is a final fenced publication may omit cooperative cancellation when the PR documents why cancellation is not useful. `src/live_index/git_temporal.rs::spawn_git_temporal_computation` is the Phase H example: the git walk may finish, but publication goes through `update_git_temporal_at_generation`.

### Layer 2  Generation fence via *_at_generation fenced API

Watcher-subsystem blocking work must capture `expected_gen = shared.current_project_generation()` before spawning or before entering the async task that owns the spawn. The blocking closure must reuse that captured generation. It must not re-sample `current_project_generation()` inside or immediately before the mutation, because re-sampling converts a stale producer into a current-generation writer.

All shared-index mutations from the stale-prone path must use a fenced API:

- file content updates use `update_file_at_generation`
- mtime-only updates use `touch_mtime_at_generation`
- removals use `remove_file_at_generation`
- git-temporal publication uses `update_git_temporal_at_generation`

For streamed or non-atomic work that cannot fence every write, place a best-effort pre-flight generation check at function entry and document the accepted residual. `src/live_index/coupling/lifecycle.rs::refresh_on_reconcile_tick` is the Phase H example: it checks `expected_gen` before flag, git-repo, guard, or disk work and increments `rejected_stale_mutations` on rejection.

Exempt cases:

- Read-only `spawn_blocking` sites.
- Blocking work that is provably local to the closure and never writes to shared index state, watcher-derived persistent state, or a published result.

### Layer 3  Optional re-stat-on-NotFound retry

Layer 3 applies when `NotFound` can drive a `remove_file_at_generation` call. The watcher should retry the stat/read/hash/parse/update pipeline with bounded backoff before treating the file as confirmed absent.

The Phase H implementation is `src/watcher/mod.rs::maybe_reindex`: initial attempt, then 50 ms, 200 ms, and 500 ms retries. If all attempts still observe `NotFound`, the watcher calls `remove_file_at_generation(relative_path, expected_gen)`.

Layer 3 is optional because it only applies to absence-driven removal paths. It is not applicable to publication-only sites such as git-temporal publication or coupling-refresh pre-flight checks, because they do not remove indexed files.

## Failure-mode coverage matrix

This matrix restates the canonical guidance from `docs/plans/2026-05-12-symforge-stability-hotfix.md`, section "Failure-mode coverage matrix (Tier 1 design rationale)", using current symbol names instead of line-number references.

| Spawn site | Mechanism it can hit | Layer 1 cancellation token | Layer 2 generation fence | Layer 3 re-stat retry |
|---|---|---|---|---|
| Periodic reconcile sweep spawned by `src/watcher/mod.rs::run_watcher_with_stop` and executed by `src/watcher/mod.rs::reconcile_stale_files_with_stop` | A: cross-root stale producer. C: transient `NotFound` from AV, IDE, build, or checkout activity. | Token cancels loop entry; per-path checks break before each `freshen_file_if_stale_at_generation`. | A doomed task that already read paths cannot remove files in the new root's index because `remove_file_at_generation` rejects stale generation. | Transient `NotFound` retries before confirmed removal. |
| Coupling refresh spawned by `src/watcher/mod.rs::run_watcher_with_stop` and executed by `src/live_index/coupling/lifecycle.rs::refresh_on_reconcile_tick` | A: stale workspace refresh. | Token check before spawn; closure body is not cancellable once running. | Best-effort pre-flight check re-checks generation at function entry; a doomed-just-spawned task aborts before disk work. Mid-walk completion is accepted residual because coupling writes are streamed to the captured root's per-workspace database. | Not applicable: no `remove_file` call. |
| Event batch handler `src/watcher/mod.rs::process_events` | A: cross-root stale producer. C: transient `NotFound`. | Token check per event and per event path; Layer 2 remains load-bearing for high-throughput gaps. | Every remove triggered by a stale remove event goes through `remove_file_at_generation`; create/modify paths call `maybe_reindex` with the captured generation. | Event-driven `NotFound` paths retry through `maybe_reindex`. |
| Overflow-triggered reconcile spawned by `src/watcher/mod.rs::run_watcher_with_stop` and executed by `src/watcher/mod.rs::reconcile_stale_files_with_stop` | A: cross-root stale producer. C: transient `NotFound`. | Token cancels before overflow sweep and between paths. | Every remove during overflow sweep goes through `remove_file_at_generation`. | Per-path `NotFound` retry through `maybe_reindex`. |
| Git-temporal computation `src/live_index/git_temporal.rs::spawn_git_temporal_computation`, including calls from `src/daemon.rs::ProjectInstance::reload` | A: stale temporal data published for the wrong root. | Not applicable: long-running git walks are allowed to finish. | `update_git_temporal_at_generation` rejects A-era publication after reload to root B. | Not applicable: no `remove_file` call. |

The layer rationale is:

- Layer 1 alone is insufficient because a blocking closure can already be past its cancellation check when reload happens.
- Layer 2 alone is insufficient because stale work can still run to completion, consume blocking threads, and accumulate rejection telemetry.
- Layer 3 alone is insufficient because it only addresses transient `NotFound`; it cannot detect a stale producer whose A-era path exists.

## Scope and exemptions

- Watcher subsystem mutation sites: convention applies
- Read-only spawn sites (telemetry, metrics): exempt
- Enforcement: reviewer responsibility at PR time. NOT enforced by lint/CI.

This ADR applies to future watcher-subsystem blocking work and to adjacent watcher-owned publication paths when their stale output can affect the current project. It does not require retrofitting unrelated read-only `spawn_blocking` calls outside the watcher subsystem.

## Worked example  additive pattern (H.1e + H.1f)

H.1e and H.1f are the additive-pattern templates.

H.1e added a new fenced publication method, `SharedIndexHandle::update_git_temporal_at_generation`, then updated `spawn_git_temporal_computation` callers to capture and pass `expected_gen`. Use this pattern when the blocking work has a single publication boundary.

H.1f could not fence a single commit boundary because coupling-store writes are streamed through the walk. It added parameters to `refresh_on_reconcile_tick(project_root, expected_gen, shared)` and put the generation check at function entry. Use this pattern when the implementation can only provide a best-effort pre-flight fence; document any accepted residual.

Code-shaped template for a new watcher-owned mutation site:

```rust
let expected_gen_for_task = expected_gen;
let shared_for_task = shared.clone();
let stop_for_task = Arc::clone(&stop_token);
let root_for_task = repo_root.clone();

tokio::task::spawn_blocking(move || {
    if stop_for_task.load(Ordering::Acquire) {
        return;
    }

    refresh_new_subsystem_at_generation(
        &root_for_task,
        expected_gen_for_task,
        &shared_for_task,
    );
});
```

The mutation-side entry point should make the fence the first operation that can reject stale work:

```rust
pub fn refresh_new_subsystem_at_generation(
    project_root: &Path,
    expected_gen: u64,
    shared: &SharedIndex,
) {
    let current_gen = shared.current_project_generation();
    if current_gen != expected_gen {
        shared.note_rejected_stale_mutation();
        tracing::trace!(
            expected_gen,
            current_gen,
            "new subsystem refresh rejected stale generation"
        );
        return;
    }

    // Do mutation work here. If the work has a single publication boundary,
    // prefer a dedicated *_at_generation API instead of only a pre-flight check.
    let _ = project_root;
}
```

If this work can remove indexed files after observing `NotFound`, the removal path also needs the Layer 3 retry shape from `src/watcher/mod.rs::maybe_reindex` before calling `remove_file_at_generation`.

## Consequences

Positive:

- Future watcher mutation sites have one named convention instead of rediscovering the Phase H reasoning.
- Stale producers are rejected at the shared-index boundary.
- Transient `NotFound` false removals have a consistent mitigation.
- Reviewers have an explicit checklist for new watcher-side blocking closures.

Costs and risks:

- The convention is manual reviewer discipline, so it can drift without careful PR review.
- More arguments get threaded through call stacks: `stop_token`, `expected_gen`, and sometimes `shared`.
- Best-effort pre-flight fences, such as coupling refresh, can still leave documented residual work when the underlying operation streams writes and has no atomic publication boundary.
- The single `rejected_stale_mutations` counter does not attribute rejections by surface; use trace logs or add per-surface counters in a future ADR if that becomes operationally necessary.

Revisit this ADR if watcher mutation sites expand beyond the current pattern, if lintable structure emerges, or if accepted residual work from streamed mutation paths becomes user-visible correctness risk.
