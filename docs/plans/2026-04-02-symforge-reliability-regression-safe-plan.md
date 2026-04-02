# SymForge Reliability Regression-Safe Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the installed-build gaps found during real MCP use: startup empty-index behavior, pre-index `what_changed` degradation, path-aware `ask` routing, and missing failed-file detail in `health`, while treating parse-count drift as reproduce-first work only.

**Architecture:** Keep the read path non-blocking and preserve daemon/local fallback semantics. Prefer small, test-first changes in `src/main.rs`, `src/protocol`, and `src/live_index` with no more than 5 touched files per phase. Do not "fix" the parse-drift report until a deterministic failing test proves an internal bug.

**Tech Stack:** Rust, rmcp, reqwest daemon proxy, LiveIndex snapshots under `.symforge/`, tree-sitter parsers, Cargo tests.

---

## Discovery Facts

- The root source repo at `E:/project/symforge` is aligned at `v6.0.1` / `bd3a29647bd416c9a3a2ddd381988cc2b6211a21`.
- The installed-build baseline must be compared against the **execution worktree actually running Task 1**, not just the root repo.
- The current Task 1 worktree at `C:/Users/rakovnik/.config/superpowers/worktrees/symforge/reliability-regression-safe` is still stale:
  - `git rev-parse --short HEAD` -> `a4e2367`
  - `git describe --tags --always` -> `v6.0.0`
  - `Cargo.toml` -> `version = "6.0.0"`
  - `target/debug/symforge.exe --version` -> `symforge 6.0.0`
- Because of that stale worktree state, the startup rerun is still contaminated by `6.0.0` vs installed `6.0.1` mismatch until the worktree itself is rebased or recreated from `bd3a296`.
- Startup already has auto-index and daemon-backed attach logic in `src/main.rs:80-103` and `src/main.rs:143-233`.
- Empty-index `health` is currently intentional and covered by `src/protocol/tools.rs:6847-6860`.
- Pre-index `what_changed` hard-fails when no `repo_root` is attached in `src/protocol/tools.rs:768-806` and `src/protocol/tools.rs:3905-4122`.
- `ask` currently loses file context for caller queries because routing only carries a symbol string, not a path, in `src/protocol/smart_query.rs:46-233` and `src/protocol/tools.rs:5492-5693`.
- Production `health` uses published state and explicitly drops `partial_parse_files` and `failed_files` in `src/protocol/format.rs:1003-1031`, even though full detail exists in `src/live_index/query.rs:2270-2324`.
- I do **not** have proof yet that parse-count drift is an internal counting bug; watcher/background-verify activity could explain it legitimately.

## Audit Triage

Validated as current-plan relevant:

- The startup behavior still needs a current-version baseline rerun on `v6.0.1`.
- `what_changed` graceful degradation remains a real gap.
- `ask` path-aware caller routing remains a real gap.
- `health` published-state detail loss remains a real gap.

Explicitly **not** accepted into this regression-safe plan:

- `src/daemon.rs:357` is an `unwrap()` on a key proven present under the same `projects` write lock. It is not a demonstrated panic path in current code.
- The cited `src/cli/hook.rs:1300` unwrap is test-only in the current tree, not a production reliability issue.
- The cited `src/cli/hook.rs:1933-1938` file-I/O unwrap chain is also test-only in the current tree.
- The "4 dead pub getters" claim is materially inaccurate. `project_id()`, `session_id()`, and `port()` have current production callers in `src/main.rs`.

Deferred to a separate hygiene/refactor plan:

- `live_index/store.rs::load()` extraction and deduplication with `build_reload_data()`
- `cli/hook.rs::run_hook()` refactor
- replacing public `anyhow::Result` with domain enums
- sleep-based test cleanup
- unsafe-block audit
- clone-reduction cleanup

## Verified Current Tests

These current behaviors are already codified and passed fresh:

- `cargo test test_health_always_responds_on_empty_index -- --nocapture`
- `cargo test test_what_changed_defaults_to_uncommitted_git_changes -- --nocapture`
- `cargo test test_health_report_from_published_state_matches_live_index_output -- --nocapture`
- `cargo test test_health_report_lists_partial_parse_files -- --nocapture`

---

### Task 1: Re-Establish Startup Baseline on `v6.0.1`

**Files:**
- Inspect: `Cargo.toml`
- Inspect: `src/main.rs`
- Inspect: `tests/live_index_integration.rs`

**Step 1: Verify the execution worktree itself, not just the root repo**

Before trusting any startup result, check all of these inside the execution worktree:

- `git rev-parse --short HEAD`
- `git describe --tags --always`
- `Cargo.toml` package version
- `target/debug/symforge.exe --version` after `cargo build`

Required baseline for Task 1:

- HEAD based on `bd3a296`
- tag/describe resolving to `v6.0.1`
- `Cargo.toml` showing `version = "6.0.1"`
- built binary reporting `symforge 6.0.1`

If any of those still report `6.0.0`, rebase or recreate the worktree from `bd3a296` before continuing. Root-repo alignment alone is not sufficient evidence.

**Step 2: Keep or reapply only zero-risk Task 1 baseline instrumentation**

Use the existing startup baseline work only if it replays cleanly onto `v6.0.1` without semantic changes:

```rust
enum StartupPlan {
    Daemon { root: PathBuf },
    LocalAutoIndex { root: PathBuf },
    LocalEmpty { reason: String },
}
```

and the startup binary probe in `tests/live_index_integration.rs`.

**Step 3: Run the actual current-version startup probes**

Run:

- `cargo build`
- `cargo test --all-targets -- --test-threads=1`
- `cargo test test_startup_binary_reports_branch_health -- --nocapture`

Expected: authoritative startup evidence for a worktree that itself reports `v6.0.1`, not just a root repo that does.

**Step 4: Gate the rest of the plan on the result**

- If the startup probe passes or shows acceptable current behavior, record that and skip Task 2.
- If the startup probe still fails on `v6.0.1`, continue to Task 2 with the captured branch evidence.

**Step 5: Commit baseline-only changes if needed**

```bash
git add src/main.rs tests/live_index_integration.rs
git commit -m "test: capture current startup branch behavior"
```

---

### Task 2: Conditional Startup-Surface Fix

**Only execute this task if Task 1 still reproduces a current `v6.0.1` startup problem.**

**Files:**
- Modify: `src/main.rs`
- Modify: `tests/live_index_integration.rs`
- Modify only one additional file if the proven root cause requires it

**Step 1: Use the Task 1 failing probe as the guardrail**

Do not broaden the startup issue. Fix only the reproduced current branch:

- daemon-backed startup missing a reachable health surface
- local auto-index never transitioning to ready
- empty-start reason not being surfaced precisely

**Step 2: Implement the smallest fix in the proven branch**

Examples of acceptable narrow fixes:

- expose a reliable health surface for daemon-backed startup
- tighten startup branch selection/marker writing
- make local startup publish a deterministic branch reason

Do **not** redesign startup architecture in this phase.

**Step 3: Re-run targeted startup verification**

Run:

- `cargo test test_startup_binary_reports_branch_health -- --nocapture`
- `cargo test --all-targets -- --test-threads=1`

Expected: PASS.

**Step 4: Commit**

```bash
git add src/main.rs tests/live_index_integration.rs
git commit -m "fix: stabilize startup branch health surface"
```

---

### Task 3: Make `what_changed` Graceful When `repo_root` Is Missing

**Files:**
- Modify: `src/protocol/tools.rs`
- Modify: `src/protocol/mod.rs`
- Test: `src/protocol/tools.rs`

**Step 1: Write failing tests**

Add tests next to the existing `what_changed` tests for:

```rust
// repo_root missing, cwd discoverable -> what_changed should lazily recover a git root
// and return working-tree changes instead of "Git change detection unavailable".

// repo_root missing, cwd not discoverable -> return a clearer message that names
// the real problem and next step:
// "No repo root attached; call index_folder(path=...) or pass since=..."
```

**Step 2: Run targeted tests**

Run:

- `cargo test test_what_changed_defaults_to_uncommitted_git_changes -- --nocapture`
- `cargo test test_what_changed_*repo_root* -- --nocapture`

Expected: new tests FAIL.

**Step 3: Implement the minimal fallback**

Add a helper in `src/protocol/mod.rs` or `src/protocol/tools.rs`:

```rust
fn effective_repo_root_for_git_tools(&self) -> Option<PathBuf> {
    self.capture_repo_root().or_else(crate::discovery::find_project_root)
}
```

Use it only in the git-backed branches of `what_changed`. Do not widen scope to unrelated tools in this phase.

**Step 4: Re-run targeted tests**

Run:

- `cargo test test_what_changed_defaults_to_uncommitted_git_changes -- --nocapture`
- `cargo test test_what_changed_*repo_root* -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/protocol/mod.rs src/protocol/tools.rs
git commit -m "fix: make what_changed recover repo root gracefully"
```

---

### Task 4: Make `ask` Preserve File Context for Caller Queries

**Files:**
- Modify: `src/protocol/smart_query.rs`
- Modify: `src/protocol/tools.rs`
- Test: `src/protocol/smart_query.rs`
- Test: `src/protocol/tools.rs`

**Step 1: Write failing tests**

Add tests for:

```rust
// classify_intent("who calls AddCoreServices in src/protocol/tools.rs")
// should preserve:
//   symbol = "AddCoreServices"
//   path = Some("src/protocol/tools.rs")

// ask(...) should route to:
// find_references(name="AddCoreServices", path="src/protocol/tools.rs")
```

Also add a negative test:

```rust
// "who calls actor in production" must NOT parse "in production" as a path.
```

**Step 2: Run targeted tests**

Run: `cargo test test_ask_*path* test_classify_*path* -- --nocapture`

Expected: FAIL because `QueryIntent::FindCallers` only carries a symbol string today.

**Step 3: Implement the minimal schema extension**

Change `QueryIntent::FindCallers` to:

```rust
FindCallers { symbol: String, path: Option<String> }
```

Parse a trailing ` in <path>` only when `<path>` passes the existing path heuristics. Update:

- `classify_intent`
- `route_invocation`
- `route_tool_name` if needed
- `ask` tool construction so `FindReferencesInput.path = path.clone()`

Do **not** change unrelated query shapes in this phase.

**Step 4: Re-run targeted tests**

Run: `cargo test test_ask_*path* test_classify_*path* -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/protocol/smart_query.rs src/protocol/tools.rs
git commit -m "fix: preserve file-scoped caller intent in ask routing"
```

---

### Task 5: Expose Failed/Partial File Details in `health` Without Blocking

**Files:**
- Modify: `src/live_index/store.rs`
- Modify: `src/protocol/format.rs`
- Test: `src/protocol/format/tests.rs`

**Step 1: Write failing tests**

Add tests that assert:

```rust
// health_report_from_published_state shows failed file paths and errors
// when published state has failed_count > 0.

// health_report_from_published_state shows partial parse file names
// when published state has partial_parse_count > 0.
```

The important constraint: keep `health` on published state, do not switch it to a long-lived live-index read lock.

**Step 2: Run targeted tests**

Run:

- `cargo test test_health_report_from_published_state_matches_live_index_output -- --nocapture`
- `cargo test test_health_report_*published* -- --nocapture`

Expected: new tests FAIL.

**Step 3: Extend published state with bounded detail**

Add bounded summary fields to `PublishedIndexState`:

```rust
pub partial_parse_files: Vec<String>,
pub failed_files: Vec<(String, String)>,
```

Populate them in `PublishedIndexState::capture` using the already-computed `health_stats()`, capped to 10 entries so published snapshots stay cheap.

Update `health_report_from_published_state` to preserve those lists instead of zeroing them.

**Step 4: Re-run targeted tests**

Run:

- `cargo test test_health_report_from_published_state_matches_live_index_output -- --nocapture`
- `cargo test test_health_report_*published* -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/live_index/store.rs src/protocol/format.rs src/protocol/format/tests.rs
git commit -m "fix: surface failed and partial files in health published state"
```

---

### Task 6: Reproduce Parse-Count Drift Before Any Fix

**Files:**
- Modify: `src/live_index/persist.rs`
- Modify: `src/live_index/store.rs`
- Modify: `src/watcher/mod.rs`
- Test: `tests/watcher_integration.rs`
- Test: `src/live_index/store.rs`

**Step 1: Write a deterministic reproduction test**

Add a test that exercises:

```rust
// snapshot restore -> background_verify -> watcher idle/no code changes
// assert published_state.generation may advance,
// but file_count / partial_parse_count / failed_count stay stable
// unless an actual parsed file changed.
```

Add a second case with a real file change to prove the test is not over-constrained.

**Step 2: Run targeted tests**

Run: `cargo test test_*published_state_tracks_* test_*background_verify* test_*watcher_* -- --nocapture`

Expected: unknown. If all PASS and no deterministic repro exists, stop here and document that the ce-registry-pro drift may have been integration noise or an external file mutation.

**Step 3: Only if the test fails, fix the proven root cause**

Likely candidates:

- background verify reparsing files that were not semantically changed
- watcher reconcile publishing transient parse failures
- published-state generation updates that incorrectly alter counts

Implement the smallest fix in the proven path only.

**Step 4: Re-run targeted tests**

Run: `cargo test test_*published_state_tracks_* test_*background_verify* test_*watcher_* -- --nocapture`

Expected: PASS.

**Step 5: Commit only if a real bug was fixed**

```bash
git add src/live_index/persist.rs src/live_index/store.rs src/watcher/mod.rs tests/watcher_integration.rs
git commit -m "fix: stabilize published parse counts across verify and watcher updates"
```

If no deterministic failure was found, commit only the reproduction test or notes, not a behavior change.

---

## Final Verification

Run targeted commands after each phase, then finish with:

```bash
cargo build
cargo test
```

If startup behavior was touched, also re-run:

```bash
cargo test test_startup_binary_reports_branch_health -- --nocapture
```

If protocol behavior was touched, also re-run:

```bash
cargo test test_health_always_responds_on_empty_index -- --nocapture
cargo test test_what_changed_defaults_to_uncommitted_git_changes -- --nocapture
cargo test test_health_report_from_published_state_matches_live_index_output -- --nocapture
```

## Stop Conditions

- If the execution worktree does not satisfy all Task 1 version checks (`HEAD`, `git describe`, `Cargo.toml`, and built binary version), rebase or recreate it before making code changes.
- If Task 1 no longer reproduces a current startup problem on `v6.0.1`, skip Task 2 entirely.
- Do not implement Task 6 without a failing reproduction.
- Do not expand this plan into structural refactors or general hygiene work from the audit.
- Do not bundle Tasks 3-5 into one commit; keep each phase under 5 touched files and stabilize before continuing.
