# Story 2.7: Cancel an Active Indexing Run Safely

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,
I want to cancel an active indexing run,
so that I can stop or restart work without leaving ambiguous run state behind.

## Acceptance Criteria

1. **Given** an indexing run is active
   **When** I request cancellation
   **Then** Tokenizor transitions the run into an explicit cancelled terminal state
   **And** cancellation is visible through later run inspection (status: `cancelled`, health: `Healthy`, `is_active: false`)

2. **Given** a run is already terminal (succeeded, failed, cancelled, interrupted, or aborted)
   **When** cancellation is requested
   **Then** Tokenizor responds deterministically by returning the current run status report
   **And** it does not create contradictory run state (no status overwrite, no error)

## Tasks / Subtasks

- [x] Task 1: Wire CancellationToken into IndexingPipeline (AC: #1)
  - [x] 1.1: Add `cancellation_token: CancellationToken` field to `IndexingPipeline` struct in `src/indexing/pipeline.rs`
  - [x] 1.2: Update `IndexingPipeline::new()` to accept `CancellationToken` parameter
  - [x] 1.3: Add cooperative cancellation check after file discovery completes, before processing begins — if cancelled, skip processing and return `IndexRunStatus::Cancelled`
  - [x] 1.4: Add cooperative cancellation check in the concurrent file processing loop — before spawning each new file task, check `self.cancellation_token.is_cancelled()` — if cancelled, stop spawning, wait for in-flight tasks, then return `Cancelled`
  - [x] 1.5: Set `RunPhase::Complete` when exiting via cancellation (same as normal completion path)
  - [x] 1.6: Update all callers of `IndexingPipeline::new()` to pass the `CancellationToken` — the token is already created in the `index_folder` spawned task; pass `token.clone()` or `token.child_token()` to the pipeline
  - [x] 1.7: Unit tests:
    - `test_pipeline_returns_cancelled_when_token_pre_cancelled` — create pipeline with already-cancelled token, execute, verify `status == Cancelled`
    - `test_pipeline_checks_cancellation_between_files` — cancel token after discovery but before all files processed, verify pipeline stops and returns `Cancelled`

- [x] Task 2: Add conditional cancellation persistence method (AC: #1, #2)
  - [x] 2.1: Add `cancel_run_if_active(&self, run_id: &str, finished_at_unix_ms: u64) -> Result<bool>` method on `RegistryPersistence` in `src/storage/registry_persistence.rs`
  - [x] 2.2: Inside `read_modify_write`: find run by `run_id`, check `is_terminal()` — if terminal, return `Ok(false)` (no change); if non-terminal, set `status = Cancelled`, `finished_at_unix_ms = Some(timestamp)`, return `Ok(true)`
  - [x] 2.3: If run not found, return `TokenizorError::NotFound`
  - [x] 2.4: Unit tests:
    - `test_cancel_run_if_active_transitions_running_to_cancelled`
    - `test_cancel_run_if_active_returns_false_for_terminal_run`
    - `test_cancel_run_if_active_returns_not_found_for_missing_run`

- [x] Task 3: Add `RunManager::cancel_run()` orchestration method (AC: #1, #2)
  - [x] 3.1: Add `pub fn cancel_run(&self, run_id: &str) -> Result<RunStatusReport>` on `RunManager` in `src/application/run_manager.rs`
  - [x] 3.2: Load run from persistence (via existing lookup pattern used by `build_run_report`) to get `repo_id` and validate existence — `NotFound` if run does not exist
  - [x] 3.3: If `run.status.is_terminal()` → return `self.inspect_run(run_id)` immediately (AC #2: deterministic response, no mutation)
  - [x] 3.4: Lock `active_runs`, look up entry by `repo_id` — if found, remove entry and call `active_run.cancellation_token.cancel()` — if not found (Queued run not yet started, or race), proceed without signaling
  - [x] 3.5: Call `self.persistence.cancel_run_if_active(run_id, unix_timestamp_ms())` — atomic, race-safe persistence update
  - [x] 3.6: Return `self.inspect_run(run_id)` for the updated report
  - [x] 3.7: Unit tests:
    - `test_cancel_active_run_signals_token_and_returns_cancelled`
    - `test_cancel_terminal_run_returns_current_report_without_mutation`
    - `test_cancel_nonexistent_run_returns_not_found`
    - `test_cancel_queued_run_without_active_entry_transitions_to_cancelled`
    - `test_cancel_removes_from_active_runs`

- [x] Task 4: Update spawned pipeline task for cancellation safety (AC: #1)
  - [x] 4.1: In the spawned task in `index_folder` (in `src/protocol/mcp.rs`), after pipeline completes: check if the run is already terminal in persistence before updating status
  - [x] 4.2: If status is already `Cancelled` (set by `cancel_run()`), skip the status update — do not overwrite with the pipeline's own result
  - [x] 4.3: Make `active_runs` cleanup idempotent — `HashMap::remove()` is already a no-op if the key was previously removed by `cancel_run()`
  - [x] 4.4: Ensure the spawned task does not panic or error if the active_run entry is missing

- [x] Task 5: Add `cancel_index_run` MCP tool (AC: #1, #2)
  - [x] 5.1: Add `#[tool(description = "Cancel an active indexing run. Returns the updated run status report. If the run is already terminal, returns the current status without modification.")]` method in the `#[tool_router] impl TokenizorServer` block in `src/protocol/mcp.rs`
  - [x] 5.2: Method signature: `fn cancel_index_run(&self, params: rmcp::model::JsonObject) -> Result<CallToolResult, McpError>`
  - [x] 5.3: Extract `run_id` required string parameter from `params` — missing → `McpError::invalid_params("missing required parameter: run_id")`
  - [x] 5.4: Call `self.application.run_manager().cancel_run(&run_id).map_err(to_mcp_error)?`
  - [x] 5.5: Serialize `RunStatusReport` to JSON via `serde_json::to_string`, return as `CallToolResult::success(vec![Content::text(json)])`

- [x] Task 6: Integration testing (AC: #1, #2)
  - [x] 6.1: Test: create run -> start pipeline -> cancel via `cancel_run()` -> inspect -> `status == Cancelled`, `is_active == false`, `health == Healthy`, `progress.phase == Complete`
  - [x] 6.2: Test: create run -> succeed -> cancel -> returns `RunStatusReport` with `status == Succeeded` unchanged (AC #2)
  - [x] 6.3: Test: cancel with non-existent `run_id` -> `TokenizorError::NotFound`
  - [x] 6.4: Test: create run -> fail -> cancel -> returns `RunStatusReport` with `status == Failed` unchanged (AC #2)
  - [x] 6.5: Test: pipeline with pre-cancelled token -> pipeline returns `Cancelled` without processing files (verify `files_processed == 0`)
  - [x] 6.6: Test: double cancel same run -> first returns `Cancelled`, second returns same `Cancelled` report (AC #2)
  - [x] 6.7: Verify test count does not regress below 259 (Story 2.6 baseline)

## Dev Notes

### CRITICAL: Load project-context.md FIRST

MUST load `_bmad-output/project-context.md` BEFORE starting implementation. It contains 87 agent rules scoped to Epic 2 covering persistence architecture, type design, concurrency, error handling, testing, and anti-patterns. Failure to load this will cause architectural violations.

### Build Order (MANDATORY)

Follow the build-then-test pattern established in Stories 2.2-2.6:

1. **Pipeline cancellation wiring** (Task 1) — add `CancellationToken` field and cooperative checks to `IndexingPipeline`
2. **Persistence method** (Task 2) — add `cancel_run_if_active` with atomic conditional update
3. **RunManager orchestration** (Task 3) — add `cancel_run()` that coordinates token signaling, persistence, and active_runs cleanup
4. **Spawned task safety** (Task 4) — make the `index_folder` spawned task handle pre-cancelled state
5. **MCP tool** (Task 5) — add `cancel_index_run` tool in `#[tool_router]` block
6. **Integration tests** (Task 6) — end-to-end verification of both acceptance criteria

### This Story Extends Existing Cancellation Infrastructure — Not New Infrastructure

The cancellation infrastructure already exists but is **not wired**:

| Component | Status | What exists |
|-----------|--------|-------------|
| `IndexRunStatus::Cancelled` | EXISTS | Domain enum variant, treated as terminal, classified as `Healthy` |
| `CancellationToken` | EXISTS but UNWIRED | Created per run, stored in `ActiveRun`, but never signaled or checked |
| `ActiveRun` struct | EXISTS | Holds `JoinHandle`, `CancellationToken`, `progress` — no changes needed |
| `is_terminal()` | EXISTS | `Cancelled` already returns `true` |
| `classify_run_health()` | EXISTS | `Cancelled` already maps to `Healthy` |
| Pipeline cancellation checks | MISSING | Pipeline never calls `token.is_cancelled()` |
| `RunManager::cancel_run()` | MISSING | No orchestration method to coordinate cancellation |
| `cancel_index_run` MCP tool | MISSING | No user-facing cancellation endpoint |
| Conditional persistence update | MISSING | Existing `update_run_status` is unconditional |

Do NOT redesign `ActiveRun`, `IndexRunStatus`, or health classification. Wire the existing pieces together.

### Key Design Decisions

**Cooperative cancellation, not forceful abort.** The pipeline checks `token.is_cancelled()` at natural boundaries (after discovery, before each file spawn). In-flight file tasks complete naturally. This avoids partial writes and complex cleanup. The architecture says "cancellation must produce durable terminal state" — cooperative cancellation achieves this cleanly.

**Cancellation check points in pipeline:**
```
1. Start pipeline (token received)
2. Discover files
3. CHECK: token.is_cancelled()? → return Cancelled
4. Set total_files, phase = Processing
5. For each file to spawn:
   5a. CHECK: token.is_cancelled()? → break loop
   5b. Spawn file task
6. Await all in-flight tasks
7. If cancelled: phase = Complete, return Cancelled
8. Normal completion path
```

**`cancel_run_if_active` is atomic and race-safe.** Uses `read_modify_write` (advisory file locking) to conditionally set `Cancelled` only if the run is non-terminal. This prevents a race where `cancel_run()` overwrites a status that the pipeline already set to `Succeeded` or `Failed`. If the run is already terminal, the method returns `false` (no-op).

**`cancel_run()` signals token THEN updates persistence.** Ordering: (1) remove from active_runs and signal token, (2) update persistence. The token signal is immediate; the persistence update is durable. If the process crashes between (1) and (2), the startup sweep (Story 2.1) will transition the `Running` run to `Interrupted` — which is the correct recovery behavior for an ambiguous state.

**Queued runs can be cancelled.** A `Queued` run may not have an active entry in `active_runs` yet (the pipeline hasn't started). `cancel_run()` handles this: no active entry to signal, just update persistence to `Cancelled`. The conditional persistence method handles this correctly since `Queued` is non-terminal.

**`active_runs` lookup requires repo_id, not run_id.** `active_runs` is `HashMap<String, ActiveRun>` keyed by `repo_id`. `cancel_run()` receives `run_id`. Solution: load the run from persistence first to get `repo_id`, then look up in `active_runs`. This is consistent with the one-active-run-per-project invariant.

**Terminal run cancellation is deterministic, not an error.** AC #2 says "responds deterministically" and "does not create contradictory run state." Returning the current `RunStatusReport` without mutation satisfies both. Do NOT return an error for cancelling a terminal run — it's a valid, expected operation (idempotent cancellation).

### Spawned Task Cancellation Safety

The spawned task in `index_folder` currently updates persistence unconditionally after pipeline completion. After this story, it must handle the case where `cancel_run()` already set the status:

```
Pipeline completes → check persistence status:
  - If already terminal (Cancelled by cancel_run()): skip status update
  - If still Running: update to pipeline result status

Active run cleanup: HashMap::remove() is no-op if already removed
```

The conditional check prevents these race scenarios:
1. **Cancel before pipeline completes:** `cancel_run()` sets `Cancelled`, pipeline finishes, spawned task sees `Cancelled` → skips update
2. **Pipeline completes before cancel:** spawned task sets `Succeeded`, `cancel_run()` later sees terminal → returns current (AC #2)
3. **Simultaneous:** `cancel_run_if_active` is atomic (file lock) — one writer wins, the other sees the result

### Previous Story Review Patterns (Apply Defensively)

From Story 2.6 code review learnings:

| Pattern | Risk | Prevention |
|---------|------|------------|
| **Wrong `#[tool_handler]` vs `#[tool_router]`** | Adding the cancel tool in the handler block | `cancel_index_run` goes in `#[tool_router] impl TokenizorServer`, NOT `#[tool_handler]` |
| **Unconditional status overwrite** | `cancel_run()` overwrites `Succeeded` with `Cancelled` | Use `cancel_run_if_active` with conditional check inside `read_modify_write` |
| **Non-idempotent cleanup** | Spawned task panics when active_run entry is missing | `HashMap::remove()` returns `None` silently — no panic |
| **Missing parameter validation** | `cancel_index_run` tool doesn't validate `run_id` | Follow exact pattern from `get_index_run`: `.ok_or_else(\|\| McpError::invalid_params(...))` |
| **Holding Mutex across .await** | `cancel_run()` holds `active_runs` lock while awaiting persistence | Drop the Mutex guard before calling persistence methods |
| **Persistence method returns wrong type** | `cancel_run_if_active` needs to communicate whether it changed anything | Return `Result<bool>` — `true` if transitioned, `false` if already terminal |

### What This Story Does NOT Implement

- **Graceful shutdown with drain timeout** — in-flight file tasks complete naturally; no timeout-based force-kill
- **Cancellation reason or message** — `error_summary` is `None` for user-initiated cancellation (it's not an error)
- **Partial result preservation on cancel** — files committed before cancellation remain in CAS and registry; no special handling needed (CAS is self-healing, committed files are valid)
- **Cancel-and-restart as a single operation** — cancel returns the terminal state; the user starts a new run separately
- **Resource subscription for cancellation events** — the `run_status` resource from Story 2.6 will show `Cancelled` state on next read; no push notification
- **CLI cancel command** — CLI is a separate surface; MCP tool serves AI coding clients first

### Testing Standards

- Naming: `test_verb_condition` (e.g., `test_cancel_active_run_signals_token_and_returns_cancelled`)
- Assertions: plain `assert!`, `assert_eq!` — NO assertion crates
- `#[test]` by default; `#[tokio::test]` only for async
- Fakes: hand-written with `AtomicUsize` call counters — NO mock crates
- Temp directories for all file operations
- Current baseline: 259 tests — must not regress
- Logging: `info!` for `cancel_run` completion, `debug!` for token signal and persistence update — NEVER `info!` per-file

### Existing Code Locations

| Component | Path | What to do |
|-----------|------|------------|
| `IndexingPipeline` (extend) | `src/indexing/pipeline.rs` | Add `cancellation_token` field, cooperative checks in `execute()`/`process_discovered()` |
| `PipelineProgress` | `src/indexing/pipeline.rs` | No changes — phase tracking from Story 2.6 is reused |
| `RegistryPersistence` (extend) | `src/storage/registry_persistence.rs` | Add `cancel_run_if_active()` conditional update method |
| `RunManager` (extend) | `src/application/run_manager.rs` | Add `cancel_run()` orchestration method |
| `TokenizorServer` tools (extend) | `src/protocol/mcp.rs` | Add `cancel_index_run` tool in `#[tool_router]` block |
| Spawned task (modify) | `src/protocol/mcp.rs` | Update `index_folder` spawned task for cancellation safety |
| `IndexRunStatus` | `src/domain/index.rs` | No changes — `Cancelled` variant already exists |
| `ActiveRun` | `src/application/run_manager.rs` | No changes — already holds `CancellationToken` |
| Health classification | `src/application/run_manager.rs` | No changes — `Cancelled` already maps to `Healthy` |
| Domain re-exports | `src/domain/mod.rs` | No changes expected |
| Integration tests (extend) | `tests/indexing_integration.rs` | Add cancellation integration tests |

### Project Structure Notes

Files to create: None

Files to modify:
- `src/indexing/pipeline.rs` — add `CancellationToken` field and cooperative cancellation checks
- `src/storage/registry_persistence.rs` — add `cancel_run_if_active()` conditional persistence method
- `src/application/run_manager.rs` — add `cancel_run()` orchestration method
- `src/protocol/mcp.rs` — add `cancel_index_run` MCP tool, update `index_folder` spawned task for cancellation safety
- `tests/indexing_integration.rs` — add cancellation integration tests

No conflicts with unified project structure detected. All changes follow existing module patterns.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Epic-2-Story-2.7]
- [Source: _bmad-output/planning-artifacts/prd.md#FR14]
- [Source: _bmad-output/planning-artifacts/architecture.md#Progress-Cancellation-Model]
- [Source: _bmad-output/planning-artifacts/architecture.md#Mutation-and-Recovery-Rules — "cancellation must produce durable terminal state"]
- [Source: _bmad-output/planning-artifacts/architecture.md#Source-Tree — cancel_index_run.rs target locations]
- [Source: _bmad-output/planning-artifacts/architecture.md#Pattern-Examples — "a cancelled indexing run transitions to a durable terminal state"]
- [Source: _bmad-output/project-context.md#MCP-Server-Run-Management]
- [Source: _bmad-output/project-context.md#Indexing-Pipeline-Architecture — concurrency and error handling]
- [Source: _bmad-output/project-context.md#Epic-2-Persistence-Architecture — read_modify_write, advisory locking]
- [Source: _bmad-output/project-context.md#Testing-Rules]
- [Source: _bmad-output/implementation-artifacts/2-6-observe-live-or-near-live-indexing-progress.md — previous story patterns, 259 test baseline]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

None — clean implementation, no blocking issues encountered.

### Completion Notes List

- Task 1: Added `CancellationToken` field to `IndexingPipeline`, cooperative cancellation checks after discovery and before each file spawn, phase set to Complete on cancel. Updated all callers (10 callsites). Added 2 unit tests.
- Task 2: Added `cancel_run_if_active` atomic conditional persistence method using `read_modify_write`. Returns `bool` indicating whether the status was changed. Added 3 unit tests.
- Task 3: Added `cancel_run` orchestration on `RunManager` — loads run, returns current report for terminal runs (AC #2), signals cancellation token and removes from active_runs (drops Mutex before persistence call), calls conditional persistence update, returns updated report. Added 5 unit tests.
- Task 4: Updated spawned pipeline task for cancellation safety — checks if run is already terminal before `update_run_status_with_finish`. Made `transition_to_running` conditional (skips if already terminal) to prevent Queued→Cancelled→Running overwrite race. Added early-exit in spawned task if run is already terminal before pipeline starts. `deregister_active_run` is idempotent by design.
- Task 5: Added `cancel_index_run` MCP tool in `#[tool_router]` block following exact pattern from `get_index_run`. Validates `run_id` parameter, delegates to `RunManager::cancel_run`, serializes `RunStatusReport` to JSON.
- Task 6: Added 6 integration tests covering: active run cancellation (AC #1), succeeded/failed run cancellation returns unchanged (AC #2), not-found error, pre-cancelled token, double cancel idempotency. Total test count: 275 (baseline 259, added 16).

### Change Log

- 2026-03-07: Implemented Story 2.7 — Cancel an Active Indexing Run Safely (all 6 tasks, 16 new tests)
- 2026-03-07: Code review — fixed 4 MEDIUM issues (misleading cancel log, incomplete test assertion for task 6.5, silently swallowed persistence errors in spawned task, race-dependent progress unwrap in integration test). 3 LOW issues noted as acceptable risk.

### File List

- `src/indexing/pipeline.rs` — Added `cancellation_token: CancellationToken` field, updated `new()` signature, cooperative cancellation checks in `execute()` and `process_discovered()`, 2 new unit tests
- `src/storage/registry_persistence.rs` — Added `cancel_run_if_active()` conditional persistence method, made `transition_to_running()` conditional (skip if terminal), 3 new unit tests
- `src/application/run_manager.rs` — Added `cancel_run()` orchestration method, updated spawned task in `launch_run()` for cancellation safety (terminal check before pipeline, terminal check before status update), 5 new unit tests
- `src/protocol/mcp.rs` — Added `cancel_index_run` MCP tool in `#[tool_router]` block
- `tests/indexing_integration.rs` — Added 6 cancellation integration tests
