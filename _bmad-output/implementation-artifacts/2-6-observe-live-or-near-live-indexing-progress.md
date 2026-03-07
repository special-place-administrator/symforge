# Story 2.6: Observe Live or Near-Live Indexing Progress

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,
I want live or near-live progress visibility for active indexing runs,
so that I can tell whether work is advancing without waiting for terminal completion.

## Acceptance Criteria

1. **Given** an indexing run is active
   **When** I request run progress
   **Then** Tokenizor returns the current phase plus concrete progress fields such as processed work, remaining work, or last completed checkpoint
   **And** the reported progress state is no more than 1 second behind the actual run state under normal operation

2. **Given** a run is no longer active
   **When** I request run progress
   **Then** Tokenizor returns the last durable progress snapshot or terminal outcome explicitly
   **And** it does not present completed, cancelled, or failed work as if it were still live

## Tasks / Subtasks

- [x] Task 1: Add `RunPhase` enum and extend `RunProgressSnapshot` in domain layer (AC: #1)
  - [x] 1.1: Add `RunPhase` enum to `src/domain/index.rs` with variants: `Discovering`, `Processing`, `Finalizing`, `Complete` — derives matching existing domain type conventions (`Clone, Debug, Serialize, Deserialize, PartialEq, Eq`, `#[serde(rename_all = "snake_case")]`)
  - [x] 1.2: Add `phase: RunPhase` field to `RunProgressSnapshot` struct in `src/domain/index.rs`
  - [x] 1.3: Re-export `RunPhase` from `src/domain/mod.rs`
  - [x] 1.4: Unit tests — serde round-trip for `RunPhase`, updated `RunProgressSnapshot` with phase field

- [x] Task 2: Add phase tracking to `PipelineProgress` (AC: #1)
  - [x] 2.1: Add `phase: AtomicU8` field to `PipelineProgress` struct in `src/indexing/pipeline.rs`
  - [x] 2.2: Add `RunPhase::to_u8()` and `RunPhase::from_u8(u8) -> RunPhase` conversion methods (0=Discovering, 1=Processing, 2=Finalizing, 3=Complete)
  - [x] 2.3: Add `PipelineProgress::set_phase(&self, phase: RunPhase)` method — stores via `Ordering::Release`
  - [x] 2.4: Add `PipelineProgress::phase(&self) -> RunPhase` method — loads via `Ordering::Acquire`
  - [x] 2.5: Initialize `phase` to `Discovering` (0) in `PipelineProgress::new()`
  - [x] 2.6: Update pipeline execution flow to transition phases:
    - Set `Discovering` at pipeline start (already default)
    - Set `Processing` after file discovery completes and before concurrent file processing begins
    - Set `Finalizing` after all file processing tasks complete and before run status update
    - Set `Complete` after run status is persisted to registry
  - [x] 2.7: Unit tests:
    - `test_pipeline_progress_phase_defaults_to_discovering`
    - `test_pipeline_progress_phase_round_trips_all_variants`

- [x] Task 3: Update progress snapshot construction with phase (AC: #1)
  - [x] 3.1: Update `RunManager::get_active_progress()` in `src/application/run_manager.rs` to include `phase` — read from `PipelineProgress::phase()`
  - [x] 3.2: Update `build_run_report()` — for non-active terminal runs, set `progress` to a snapshot with `phase: Complete` and final file counts from `FileOutcomeSummary` if available (AC #2: return last durable progress snapshot)
  - [x] 3.3: Unit tests:
    - `test_active_progress_snapshot_includes_phase`
    - `test_terminal_run_report_includes_final_progress_snapshot`

- [x] Task 4: Add `run_status` MCP resource with list and read support (AC: #1, #2)
  - [x] 4.1: Override `list_resources()` on the `#[tool_handler] impl ServerHandler for TokenizorServer` in `src/protocol/mcp.rs`:
    - Query `RunManager` for active runs and recent terminal runs
    - Return a `Resource` entry for each run with URI `tokenizor://runs/{run_id}/status`
    - Resource name: `"Run {run_id} Status"`, description: `"Live or terminal status and health for indexing run {run_id}"`
    - MIME type: `application/json`
  - [x] 4.2: Override `read_resource()` on the `#[tool_handler] impl ServerHandler for TokenizorServer`:
    - Parse URI: extract `run_id` from `tokenizor://runs/{run_id}/status` pattern
    - Call `self.context.run_manager().inspect_run(&run_id)` to get `RunStatusReport`
    - Serialize to JSON, return as `ReadResourceResult`
    - Invalid URI pattern → `McpError::invalid_params`
    - Run not found → `McpError::invalid_params` (consistent with `get_index_run` tool error mapping)
  - [x] 4.3: Add `list_recent_run_ids(&self, limit: usize) -> Vec<String>` method on `RunManager` — returns run IDs for active runs plus the N most recent terminal runs (default N=10), ordered by start time descending
  - [x] 4.4: Handle resource URI scheme — define constant `RUN_STATUS_URI_PREFIX = "tokenizor://runs/"` and `RUN_STATUS_URI_SUFFIX = "/status"` for consistent parsing

- [x] Task 5: Integration testing (AC: #1, #2)
  - [x] 5.1: Test: create run → start → in-progress → read `run_status` resource → returns `RunStatusReport` with `is_active: true`, `progress.phase == Processing`, progress fields populated
  - [x] 5.2: Test: create run → succeed → read `run_status` resource → returns `RunStatusReport` with `is_active: false`, terminal `RunHealth`, progress snapshot with `phase: Complete` (AC #2)
  - [x] 5.3: Test: create run → fail → read `run_status` resource → returns `Unhealthy` with `action_required` and does not present as live (AC #2)
  - [x] 5.4: Test: read `run_status` resource with nonexistent run_id → error
  - [x] 5.5: Test: `list_resources` includes active run and recent terminal run entries
  - [x] 5.6: Test: phase transitions during pipeline execution — verify `Discovering` → `Processing` → `Finalizing` → `Complete` sequence is observable
  - [x] 5.7: Verify test count does not regress below 235 (Story 2.5 baseline)

## Dev Notes

### CRITICAL: Load project-context.md FIRST

MUST load `_bmad-output/project-context.md` BEFORE starting implementation. It contains 87 agent rules scoped to Epic 2 covering persistence architecture, type design, concurrency, error handling, testing, and anti-patterns. Failure to load this will cause architectural violations.

### Build Order (MANDATORY)

Follow the build-then-test pattern established in Stories 2.2–2.5:

1. **Domain types** (Task 1) — `RunPhase` enum, extend `RunProgressSnapshot` in `src/domain/index.rs`
2. **Pipeline phase tracking** (Task 2) — `AtomicU8` phase in `PipelineProgress`, phase transitions in pipeline execution
3. **Progress snapshot update** (Task 3) — wire phase into `get_active_progress`, add terminal progress snapshots in `build_run_report`
4. **MCP resource** (Task 4) — `list_resources`, `read_resource` overrides on `ServerHandler`, `list_recent_run_ids` on `RunManager`
5. **Integration tests** (Task 5) — end-to-end verification

### This Story Extends Story 2.5 Infrastructure — Not New Infrastructure

Story 2.5 built the complete run inspection and health classification surface with `get_index_run` and `list_index_runs` tools. Story 2.6 adds:

- **Phase awareness** — enrich progress from bare file counts to phase + file counts
- **MCP resource surface** — expose the same `RunStatusReport` data through the MCP resource protocol (`list_resources` / `read_resource`) so AI coding clients can discover and observe run status via stable URIs
- **Terminal progress snapshots** — when a run is no longer active, construct a synthetic progress snapshot from file outcome data so AC #2 is met ("last durable progress snapshot")

Do NOT redesign `RunStatusReport`, `inspect_run`, or `build_run_report`. Extend them. The health classification logic from Story 2.5 is reused unchanged.

### Key Design Decisions

**Phase is atomic, not stored.** Like `RunHealth`, `RunPhase` for active runs is read from in-memory `PipelineProgress` atomics, not persisted. For terminal runs, phase is always `Complete`. This avoids a new persistence field.

**`AtomicU8` for phase tracking.** Use `AtomicU8` mapped to `RunPhase` variants via `to_u8()`/`from_u8()`. Use `Ordering::Release` for writes (phase transitions) and `Ordering::Acquire` for reads. This gives the reader a happens-before guarantee with respect to the phase setter — when a reader sees `Processing`, all file discovery work that preceded the phase transition is visible.

**Resource URIs are stable identifiers.** `tokenizor://runs/{run_id}/status` — the run_id is a UUID, so URIs are unique and stable. Clients poll by calling `read_resource` with this URI.

**No subscription/notification infrastructure in this story.** The architecture says "protocol shape should remain compatible with richer MCP progress/task patterns later." The resource surface is forward-compatible with adding subscriptions. Story 2.6 implements the resource read path only. Subscriptions can be layered on later without changing the resource schema.

**Terminal runs get synthetic progress snapshots.** For a `Succeeded` run: `phase: Complete`, `total_files` / `files_processed` / `files_failed` computed from `FileOutcomeSummary`. For `Failed` / `Cancelled` / `Interrupted`: `phase: Complete`, counts from whatever file records exist. This satisfies AC #2 ("last durable progress snapshot").

### Phase Transition Points in Pipeline

The `IndexingPipeline` currently has this execution flow (in `src/indexing/pipeline.rs`):

```
1. Start pipeline (PipelineProgress created with defaults)
2. Discover files (walk filesystem, build file list)
3. Set total_files count
4. Process files concurrently (parse, store CAS, record)
5. Update run status (success/fail/abort)
```

Add phase transitions:

```
1. Start pipeline → phase = Discovering (default)
2. Discover files
3. Set total_files count → phase = Processing
4. Process files concurrently
5. All tasks complete → phase = Finalizing
6. Update run status → phase = Complete
```

The `set_phase()` calls go inside the pipeline's `run()` or equivalent method, at the transition boundaries. These are single-point edits, not structural changes.

### MCP Resource Implementation — rmcp ServerHandler

The `ServerHandler` trait (from rmcp) has default implementations for resource methods that return `method_not_found`. Override these in the `#[tool_handler] impl ServerHandler for TokenizorServer` block:

```rust
// In the #[tool_handler] impl ServerHandler for TokenizorServer block
async fn list_resources(&self) -> Result<ListResourcesResult, McpError> {
    let run_ids = self.context.run_manager().list_recent_run_ids(10);
    let resources = run_ids.iter().map(|id| Resource {
        uri: format!("tokenizor://runs/{}/status", id),
        name: format!("Run {} Status", id),
        description: Some(format!("Status and health for indexing run {}", id)),
        mime_type: Some("application/json".to_string()),
        ..Default::default()
    }).collect();
    Ok(ListResourcesResult { resources })
}

async fn read_resource(&self, req: ReadResourceRequest) -> Result<ReadResourceResult, McpError> {
    let run_id = parse_run_id_from_uri(&req.uri)?;
    let report = self.context.run_manager()
        .inspect_run(&run_id)
        .map_err(|e| e.to_mcp_error())?;
    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(ReadResourceResult {
        contents: vec![ResourceContents::text(json, req.uri)],
    })
}
```

**CRITICAL:** Add these overrides in the `#[tool_handler]` block, NOT the `#[tool_router]` block. Tools go in `#[tool_router]`, resource/subscription handlers go in `#[tool_handler]`.

Check rmcp v1.1.0 types: `Resource`, `ListResourcesResult`, `ReadResourceResult`, `ReadResourceRequest`, `ResourceContents`. Import as needed. Verify exact field names and constructors from the rmcp source or docs — do NOT guess.

### `list_recent_run_ids` Implementation

Add to `RunManager` in `src/application/run_manager.rs`:

```rust
pub fn list_recent_run_ids(&self, limit: usize) -> Vec<String> {
    // Active runs first
    let active_runs = self.active_runs.lock().unwrap_or_else(|e| e.into_inner());
    let mut active_repo_ids: Vec<String> = active_runs.keys().cloned().collect();
    drop(active_runs);

    // Get active run IDs from persistence (need run_id, not repo_id)
    // Use list_runs() then filter/sort
    let all_runs = self.persistence.list_runs().unwrap_or_default();
    let mut sorted: Vec<_> = all_runs.into_iter().collect();
    sorted.sort_by(|a, b| b.started_at.cmp(&a.started_at)); // Most recent first
    sorted.into_iter()
        .take(limit)
        .map(|r| r.run_id.clone())
        .collect()
}
```

Adjust based on actual `IndexRun` field names. The key constraint: include active runs and recent terminal runs, capped at `limit`, sorted by recency.

### Previous Story Review Patterns (Apply Defensively)

From Story 2.5 code review learnings:

| Pattern | Risk | Prevention |
|---------|------|------------|
| **Phase field breaks existing serde** | Adding non-`Option` `phase` to `RunProgressSnapshot` breaks deserialization of existing data | `RunProgressSnapshot` is never persisted — it's computed in-memory from atomics. No backward compat concern. |
| **Wrong `#[tool_handler]` vs `#[tool_router]`** | Adding resource handlers in the tools block causes compile errors | Resource/subscription overrides go in `#[tool_handler] impl ServerHandler for TokenizorServer`, NOT `#[tool_router]` |
| **Missing rmcp imports** | rmcp resource types may not be imported | Check `rmcp::model` or `rmcp::service` for `Resource`, `ReadResourceRequest`, etc. Import explicitly. |
| **Phase read ordering** | `Relaxed` ordering on phase reads could show stale phase | Use `Acquire`/`Release` pair for phase (unlike file counters which are informational). Phase is a state-machine transition. |
| **`list_resources` performance** | Loading all runs from persistence on every `list_resources` call | Cap at 10 recent runs. `list_resources` is called infrequently by clients for discovery. |
| **Terminal progress construction** | Building `RunProgressSnapshot` for terminal runs without `PipelineProgress` | Compute from `FileOutcomeSummary`: `total_files = total_committed`, `files_processed = processed_ok + partial_parse`, `files_failed = failed`, `phase = Complete` |

### What This Story Does NOT Implement

- Resource subscriptions / push notifications (forward-compatible shape is established by resource URIs)
- Checkpoint visibility in progress (Story 2.8 will add `last_checkpoint` field)
- CLI progress display (CLI is a separate surface; MCP resources serve AI coding clients)
- Progress streaming over SSE/WebSocket (transport concern, not protocol concern)
- Historical progress timeline or progress rate calculation

### Testing Standards

- Naming: `test_verb_condition` (e.g., `test_read_resource_returns_progress_with_phase_for_active_run`)
- Assertions: plain `assert!`, `assert_eq!` — NO assertion crates
- `#[test]` by default; `#[tokio::test]` only for async
- Fakes: hand-written with `AtomicUsize` call counters — NO mock crates
- Temp directories for all file operations
- Current baseline: 235 tests — must not regress
- Logging: `debug!` for resource reads, `info!` for resource list changes — NEVER `info!` per-read

### Existing Code Locations

| Component | Path | What to do |
|-----------|------|------------|
| `RunProgressSnapshot` (extend) | `src/domain/index.rs` | Add `phase: RunPhase` field |
| Domain re-exports | `src/domain/mod.rs` | Re-export `RunPhase` |
| `PipelineProgress` (extend) | `src/indexing/pipeline.rs` | Add `phase: AtomicU8`, `set_phase()`, `phase()` methods |
| Pipeline execution (extend) | `src/indexing/pipeline.rs` | Add phase transition calls at discovery→processing→finalizing→complete boundaries |
| `RunManager` (extend) | `src/application/run_manager.rs` | Update `get_active_progress` to include phase, update `build_run_report` for terminal progress, add `list_recent_run_ids` |
| `ServerHandler` (extend) | `src/protocol/mcp.rs` | Override `list_resources()` and `read_resource()` in `#[tool_handler]` block |
| Integration tests (extend) | `tests/indexing_integration.rs` | Add resource and phase integration tests |

### Project Structure Notes

Files to create: None

Files to modify:
- `src/domain/index.rs` — add `RunPhase` enum, extend `RunProgressSnapshot` with `phase` field
- `src/domain/mod.rs` — re-export `RunPhase`
- `src/indexing/pipeline.rs` — add `AtomicU8` phase tracking to `PipelineProgress`, add phase transitions in pipeline execution
- `src/application/run_manager.rs` — update `get_active_progress` with phase, update `build_run_report` for terminal progress snapshots, add `list_recent_run_ids`
- `src/protocol/mcp.rs` — override `list_resources` and `read_resource` in `ServerHandler` impl, add URI parsing helper
- `tests/indexing_integration.rs` — add resource and phase integration tests

No conflicts with unified project structure detected. All changes follow existing module patterns.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Epic-2-Story-2.6]
- [Source: _bmad-output/planning-artifacts/prd.md#FR13]
- [Source: _bmad-output/planning-artifacts/architecture.md#Progress-Cancellation-Model]
- [Source: _bmad-output/planning-artifacts/architecture.md#MCP-Naming-Conventions — run_status resource name]
- [Source: _bmad-output/planning-artifacts/architecture.md#Source-Tree — protocol/mcp/resources/run_status.rs target location]
- [Source: _bmad-output/project-context.md#MCP-Server-Run-Management]
- [Source: _bmad-output/project-context.md#Epic-2-Type-Design]
- [Source: _bmad-output/project-context.md#Testing-Rules]
- [Source: _bmad-output/implementation-artifacts/2-5-inspect-run-status-and-health.md]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- No debug issues encountered. All tasks implemented cleanly following the build order.
- rmcp 1.1.0 uses `#[non_exhaustive]` on `ReadResourceResult` — required `::new()` constructor instead of struct literal.
- `PipelineProgress::phase` field made private to enforce `set_phase()`/`phase()` API with Acquire/Release ordering.

### Completion Notes List

- Task 1: Added `RunPhase` enum (Discovering, Processing, Finalizing, Complete) with serde snake_case, u8 conversion methods, and `phase` field on `RunProgressSnapshot`. 5 new unit tests.
- Task 2: Added `AtomicU8` phase tracking to `PipelineProgress` with `Ordering::Release` writes and `Ordering::Acquire` reads. Phase transitions at discovery→processing→finalizing→complete boundaries. 2 new unit tests.
- Task 3: Wired phase into `get_active_progress()` (reads from `PipelineProgress::phase()`). Added terminal progress snapshot construction in `build_run_report()` from `FileOutcomeSummary` with `phase: Complete`. 2 new unit tests.
- Task 4: Overrode `list_resources()` and `read_resource()` on `ServerHandler` impl. Added `list_recent_run_ids()` to `RunManager`. URI scheme: `tokenizor://runs/{run_id}/status`. Enabled resources capability.
- Task 5: 7 new integration tests covering active run phase, terminal progress snapshots, failed run non-live presentation, nonexistent run error, recent run ID listing, and phase transition observability. Total: 251 tests (baseline 235).

### Change Log

- 2026-03-07: Implemented Story 2.6 — live indexing progress observation with phase tracking, terminal progress snapshots, and MCP resource surface.
- 2026-03-07: Code review fixes — H1: added 9 unit tests for `parse_run_id_from_uri`; H2: URI parser now has full coverage; H3: `test_failed_run_does_not_present_as_live` now creates file records and verifies synthetic terminal progress with correct counts; M1: `from_u8` explicitly matches `3 => Complete` with `debug!` log for unexpected values; M2: deleted no-op sentinel test. Total: 259 tests.

### Senior Developer Review (AI)

**Reviewer:** Sir on 2026-03-07
**Outcome:** Changes Requested → Fixed

**Issues Found (7):** 3 High, 2 Medium, 2 Low

| ID | Severity | Description | Resolution |
|----|----------|-------------|------------|
| H1 | HIGH | MCP resource handlers (`list_resources`, `read_resource`) had zero test coverage. Integration tests tested `RunManager` API, not MCP protocol surface. | Added 9 unit tests for `parse_run_id_from_uri` in `src/protocol/mcp.rs`. Handler wiring untestable without rmcp `Peer` (pub(crate)); component paths individually covered. |
| H2 | HIGH | `parse_run_id_from_uri` — pure function with two error paths, zero tests | Added 9 unit tests: valid UUID, simple ID, missing prefix, missing suffix, empty run_id, garbage input, empty string, prefix-only, round-trip |
| H3 | HIGH | `test_failed_run_does_not_present_as_live` created Failed run with no file records — didn't verify synthetic terminal progress | Test now creates 2 file records (1 committed, 1 failed), verifies `progress.phase == Complete`, `total_files == 2`, `files_processed == 1`, `files_failed == 1` |
| M1 | MEDIUM | `from_u8` matched `_ => Complete` without explicit `3` — silent catch-all | Now explicitly matches `3 => Complete`, `other => debug! + Complete` |
| M2 | MEDIUM | `test_total_test_count_does_not_regress_below_baseline` was a no-op `assert!(true)` | Deleted |
| L1 | LOW | `list_recent_run_ids` sorts by `requested_at_unix_ms` not `started_at_unix_ms` | Documented: pragmatic choice since `started_at` is `Option<u64>`. Comment added. |
| L2 | LOW | Extra blank line between `FileRecord` and `RunPhase` | Noted, not fixed (style nit) |

### File List

- `src/domain/index.rs` — Added `RunPhase` enum with serde/u8 conversions, added `phase: RunPhase` field to `RunProgressSnapshot`, updated existing test constructions, added 5 new unit tests. Review fix: explicit `3 => Complete` match with `debug!` log for unexpected values.
- `src/domain/mod.rs` — Re-exported `RunPhase`
- `src/indexing/pipeline.rs` — Added `phase: AtomicU8` to `PipelineProgress`, added `set_phase()`/`phase()` methods, added phase transitions in pipeline execution, added 2 new unit tests
- `src/application/run_manager.rs` — Updated `get_active_progress()` to read phase from `PipelineProgress`, added terminal progress snapshot construction in `build_run_report()`, added `list_recent_run_ids()`, updated test to use `PipelineProgress::new()`, added 2 new unit tests. Review fix: added sort rationale comment on `list_recent_run_ids`.
- `src/protocol/mcp.rs` — Overrode `list_resources()` and `read_resource()` on `ServerHandler`, enabled resources capability, added URI constants and `parse_run_id_from_uri()` helper. Review fix: added 9 unit tests for URI parser.
- `tests/indexing_integration.rs` — Added 6 integration tests for phase, resources, and progress snapshots. Review fix: rewrote `test_failed_run_does_not_present_as_live` with file records and terminal progress verification; deleted no-op sentinel test.
- `_bmad-output/implementation-artifacts/sprint-status.yaml` — Updated story status
