# Story 2.9: Re-Index Managed Repository or Workspace State Deterministically

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,
I want to re-index managed repository or workspace state deterministically,
so that Tokenizor can refresh indexed state after source changes without ambiguous run behavior.

## Acceptance Criteria

1. **Starting New Re-Index Run**
   ```
   Given: An indexed repository or workspace has changed
   When: I trigger re-indexing
   Then: Tokenizor starts a new managed run against the correct target
   And: Prior state remains inspectable until replacement policy is applied
   ```

2. **Idempotent Replay Behavior**
   ```
   Given: The re-index request is replayed with the same effective inputs
   When: Tokenizor processes the request
   Then: It behaves idempotently
   And: It does not create conflicting managed refresh work
   ```

## Tasks / Subtasks

- [x] Task 1: Extend domain types for re-index semantics (AC: #1)
  - [x] 1.1: Add `Reindex` variant to `IndexRunMode` enum if not already present; ensure it carries re-index-specific context (e.g., `prior_run_id: Option<String>`) [Source: src/domain/index.rs — `IndexRunMode` enum]
  - [x] 1.2: Add `ReindexRequest` struct capturing: `repo_id`, `workspace_id` (optional), `reason` (optional description), `prior_run_id` (optional — auto-discovered if omitted) [Source: src/domain/index.rs or new domain type]
  - [x] 1.3: Add `prior_run_id` field to `IndexRun` as `Option<String>` with `#[serde(default)]` for backward compatibility [Source: src/domain/index.rs — `IndexRun` struct]
  - [x] 1.4: Unit tests: serialize/deserialize `IndexRun` with and without `prior_run_id` (backward compat)

- [x] Task 2: Extend persistence for re-index state tracking (AC: #1)
  - [x] 2.1: Add `get_latest_completed_run(&self, repo_id: &str) -> Result<Option<IndexRun>>` to `RegistryPersistence` — finds most recent run with status `Succeeded` for the given repo [Source: src/storage/registry_persistence.rs]
  - [x] 2.2: Add `get_runs_by_repo(&self, repo_id: &str) -> Result<Vec<IndexRun>>` to `RegistryPersistence` — returns all runs for a repo, sorted by `requested_at_unix_ms` descending [Source: src/storage/registry_persistence.rs]
  - [x] 2.3: Ensure existing `create_index_run` handles `prior_run_id` field (persists it in registry JSON)
  - [x] 2.4: Unit tests: `get_latest_completed_run` returns correct run; returns `None` when no completed runs; filters by repo_id correctly
  - [x] 2.5: Unit tests: `get_runs_by_repo` returns all runs sorted; empty result for unknown repo

- [x] Task 3: Add re-index orchestration to RunManager (AC: #1, #2)
  - [x] 3.1: Add `reindex_repository(&self, repo_id: &str, workspace_id: Option<&str>, reason: Option<&str>) -> Result<IndexRun>` to `RunManager` [Source: src/application/run_manager.rs]
  - [x] 3.2: Inside `reindex_repository`:
    - Validate repo exists and is in a valid state for re-indexing (not currently running another index — check active runs / lease)
    - Auto-discover `prior_run_id` by calling `get_latest_completed_run` if not explicitly provided
    - Generate idempotency key from `(operation: "reindex", repo_id, workspace_id, mode: Reindex)`
    - Check idempotency record: if same key + same request_hash exists → return stored result (idempotent replay)
    - If same key + different request_hash → return `ConflictingReplay` error (per Story 2.11 pattern)
    - Create new `IndexRun` with `mode: Reindex`, `prior_run_id` set, status `Pending`
    - Write idempotency record with status `Pending`
    - Launch the run using existing `launch_run` infrastructure
  - [x] 3.3: Prior state preservation: the prior run record and its checkpoints, file records, and CAS blobs remain untouched in the registry — re-index creates a NEW run alongside the old one; no deletion or overwrite of prior state
  - [x] 3.4: Unit tests: reindex creates new run with `prior_run_id` set; reindex with active run for same repo returns error; idempotent replay returns same run_id; conflicting replay returns error
  - [x] 3.5: Unit tests: prior completed run auto-discovered correctly; no prior run still allows re-index (fresh index)

- [x] Task 4: Add idempotency support for re-index operations (AC: #2)
  - [x] 4.1: Ensure `IdempotencyRecord` and `IdempotencyService` (or equivalent persistence methods) support `operation: "reindex"` [Source: check existing idempotency infrastructure in src/storage/registry_persistence.rs or src/application/]
  - [x] 4.2: Idempotency key computation: canonical hash of `(operation, repo_id, workspace_id_or_none, mode)` — deterministic, reproducible
  - [x] 4.3: Request hash computation: canonical hash of the full request payload for conflict detection
  - [x] 4.4: Wire idempotency check into `reindex_repository` flow (before run creation)
  - [x] 4.5: Unit tests: idempotent replay with identical inputs returns stored result; conflicting replay with different inputs returns explicit error; expired idempotency records don't block new requests

- [x] Task 5: Add `reindex_repository` MCP tool (AC: #1, #2)
  - [x] 5.1: Add `reindex_repository` tool method in `#[tool_router] impl TokenizorServer` block (NOT `#[tool_handler]`) [Source: src/protocol/mcp.rs]
  - [x] 5.2: Tool accepts: `repo_id` (required), `workspace_id` (optional), `reason` (optional string)
  - [x] 5.3: Parameter extraction with `.ok_or_else(|| McpError::invalid_params(...))` pattern (match `get_index_run` / `cancel_index_run` pattern)
  - [x] 5.4: Call `self.application.reindex_repository(...)` and serialize result
  - [x] 5.5: Map errors via existing `to_mcp_error()` — ensure `ConflictingReplay` maps to appropriate MCP error
  - [x] 5.6: Unit test: tool returns run with `mode: Reindex` and `prior_run_id` populated (covered in Task 6 integration tests)

- [x] Task 6: Integration tests (AC: #1, #2)
  - [x] 6.1: Test re-index lifecycle: create initial run → complete it → trigger re-index → verify new run created with `prior_run_id` pointing to first run → verify both runs inspectable
  - [x] 6.2: Test prior state preservation: after re-index, query old run by ID → verify still returns complete data (status, checkpoints, file count)
  - [x] 6.3: Test idempotent replay: trigger re-index → replay same request → verify same run_id returned, no duplicate run created
  - [x] 6.4: Test conflicting replay: trigger re-index with repo A → replay with same idempotency key but different workspace → verify explicit error
  - [x] 6.5: Test re-index while active run exists: start index run (don't complete) → attempt re-index → verify rejection (lease/active run conflict)
  - [x] 6.6: Test re-index with no prior completed run: fresh repo → trigger re-index → verify succeeds with `prior_run_id: None` (equivalent to initial index)
  - [x] 6.7: Verify total test count increases appropriately from 299 baseline (299 → 323, +24 tests)

## Dev Notes

### Architecture Patterns and Constraints

- **Persistence model**: All durable state persists via `RegistryPersistence` to local bootstrap registry JSON file using atomic write-to-temp-then-rename with advisory file locking (fs2 crate). Do NOT wire SpacetimeDB write methods.
- **Backward compatibility**: New fields on existing structs MUST be `Option<T>` with `#[serde(default)]`. Existing registry files must deserialize without error.
- **Concurrency**: One active run per repository at a time, enforced by lease semantics. Re-index must check for active runs before creating a new one.
- **Error handling**: Use `TokenizorError` variants. Non-systemic errors (`InvalidArgument`, `InvalidOperation`, `NotFound`) reject before mutation. Systemic errors (`Storage`, `Io`, `Integrity`) transition run to `Failed` and mark recovery-required.
- **No Mutex across .await**: Extract data, drop guard, then call async persistence methods.
- **Idempotency model**: Idempotency key = deterministic hash of `(operation, repo_id, workspace_id, mode)`. Same key + same request_hash → return stored result. Same key + different request_hash → explicit rejection.
- **Prior state preservation**: Re-index creates a NEW run record alongside old runs. Old runs, checkpoints, file records, and CAS blobs are never deleted or overwritten by re-index. Prior state remains queryable by `run_id`.
- **Run lifecycle**: Follows same state machine as all runs: `Pending → Running → Completed/Failed/Cancelled`. Re-index runs carry `mode: Reindex` and `prior_run_id` for traceability.
- **Checkpoint support**: Re-index runs support checkpointing via Story 2.8 infrastructure. Long-running re-index operations can be checkpointed and resumed.
- **MCP tool placement**: New tools go in `#[tool_router] impl TokenizorServer`, NOT `#[tool_handler]`.

### Build Order (MANDATORY)

Follow the established build order from Story 2.8:
1. Domain type extensions (Task 1)
2. Persistence methods with validation (Task 2)
3. Idempotency infrastructure (Task 4)
4. RunManager orchestration (Task 3 — depends on Tasks 1, 2, 4)
5. MCP tool (Task 5 — depends on Task 3)
6. Integration tests (Task 6 — depends on all above)

### Project Structure Notes

- All modifications extend existing files — NO new files expected
- Key files to modify:
  - `src/domain/index.rs` — `IndexRunMode` enum, `IndexRun` struct
  - `src/domain/mod.rs` — exports if new types added
  - `src/storage/registry_persistence.rs` — query methods for runs by repo
  - `src/application/run_manager.rs` — `reindex_repository()` orchestration
  - `src/protocol/mcp.rs` — `reindex_repository` MCP tool
  - `tests/indexing_integration.rs` — integration tests
- Naming convention: `snake_case` for functions/modules, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for constants
- MCP tool name: `reindex_repository` (verb-noun, snake_case)

### Previous Story Intelligence (Story 2.8)

**Key learnings to apply:**
- `InvalidOperation(String)` error variant already exists for state-transition violations (added in 2.8)
- `CheckpointTracker` and `PipelineProgress` use `Arc` sharing pattern for cross-task access
- `checkpoint_run()` drops Mutex guard before calling persistence — follow same pattern for any new orchestration methods
- Files sorted by lowercase `relative_path` for deterministic cursor positions — re-index must maintain same sorting
- All validation inside `read_modify_write` closure for atomicity (prevents TOCTOU)
- Automatic checkpoint callback uses `catch_unwind` — re-index runs inherit this behavior from existing pipeline

**Patterns from 2.8 to reuse:**
- Parameter validation pattern from `get_index_run`/`cancel_index_run` for MCP tools
- `read_modify_write` pattern for atomic registry mutations
- Unit test patterns: round-trip persistence, negative tests for invalid states, backward compat deserialization

**Test baseline:** 299 tests (249 unit + 3 main + 41 integration + 6 grammar)

### Git Intelligence

Recent commit pattern: `docs: create Story X` → `feat: implement Story X` → `fix: address code review findings for Story X`

Key commits for context:
- `d105124` fix: address code review findings for Story 2.8
- `1055568` feat: implement Story 2.8 — checkpoint long-running indexing work
- `9ba4a5d` feat: implement Story 2.7 — cancel an active indexing run safely

All recent work extends existing files (0 new files in 2.8). Consistent conventional commit format.

### Technical Stack Reference

- **Language:** Rust 2024 edition
- **Async:** Tokio 1.48+ (rt-multi-thread)
- **MCP SDK:** rmcp 1.1.0+ (transport-io)
- **Serialization:** serde 1.0 + serde_json 1.0
- **Error handling:** thiserror 2.0 + anyhow 1.0
- **Logging:** tracing 0.1 + tracing-subscriber 0.3
- **File locking:** fs2 0.4
- **Testing:** cargo test + tempfile 3
- **No mock crates** — use fakes with `AtomicUsize` call counters
- **No assertion crates** — plain `assert!`/`assert_eq!`

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 2, Story 2.9]
- [Source: _bmad-output/planning-artifacts/architecture.md — Run lifecycle, idempotency model, persistence patterns]
- [Source: _bmad-output/implementation-artifacts/2-8-checkpoint-long-running-indexing-work.md — Dev notes, patterns, learnings]
- [Source: _bmad-output/project-context.md — Epic 2 persistence rules, anti-patterns, build order]
- [Source: _bmad-output/implementation-artifacts/sprint-status.yaml — Story status tracking]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

None — clean implementation with no blocking issues.

### Completion Notes List

- Task 1: Extended domain types — added `Reindex` variant to `IndexRunMode`, `prior_run_id: Option<String>` and `description: Option<String>` to `IndexRun` with backward-compatible `#[serde(default)]`. Updated all exhaustive match arms and struct literals. Removed unused `ReindexRequest` struct (M2 review fix).
- Task 2: Added `get_latest_completed_run` and `get_runs_by_repo` query methods to `RegistryPersistence`.
- Task 4: Verified existing idempotency infrastructure supports "reindex" operation natively — `operation` is a `String`, `compute_request_hash` handles `IndexRunMode::Reindex`. Added tests confirming distinct key spaces.
- Task 3: Implemented `reindex_repository` on `RunManager` with: idempotency check (replay/conflict) BEFORE active-run validation (H1 review fix), prior-run auto-discovery, new run creation with `mode: Reindex` and `prior_run_id`, pipeline launch via `spawn_pipeline_for_run` (H2 review fix). Extracted shared pipeline spawn logic from `launch_run` into reusable `spawn_pipeline_for_run`. Added delegation from `ApplicationContext`.
- Task 5: Added `reindex_repository` MCP tool in `#[tool_router]` block with `repo_id` (required), `repo_root` (required), `workspace_id` (optional), `reason` (optional) parameters. Follows existing parameter extraction and error mapping patterns.
- Task 6: Added 6 integration tests covering: lifecycle, prior state preservation, idempotent replay (while run is still active — validates H1 fix), conflicting replay, active-run rejection, fresh-repo reindex.
- Test count: 299 → 321 (+22 tests). 265 unit + 3 main + 47 integration + 6 grammar.

### Implementation Plan

Build order followed: Task 1 (domain) → Task 2 (persistence) → Task 4 (idempotency) → Task 3 (orchestration) → Task 5 (MCP) → Task 6 (integration tests).

### Code Review Fixes Applied

- **H1**: Moved idempotency check before active-run check in `reindex_repository` — same pattern as `start_run_idempotent`. Idempotent replays now return stored result even when a run is active.
- **H2**: `reindex_repository` now actually launches the indexing pipeline via extracted `spawn_pipeline_for_run`. MCP tool accepts `repo_root` (required). Runs transition through the full lifecycle instead of staying permanently Queued.
- **M1**: Added `description: Option<String>` field to `IndexRun` with `#[serde(default)]`. Reindex reason stored in `description`, not `error_summary`.
- **M2**: Removed unused `ReindexRequest` struct and its 2 serde tests.

### File List

- `src/domain/index.rs` — Added `Reindex` variant to `IndexRunMode`, `prior_run_id` and `description` fields to `IndexRun`, removed `ReindexRequest`, 3 new unit tests
- `src/domain/mod.rs` — Updated exports (removed `ReindexRequest`)
- `src/storage/registry_persistence.rs` — Added `get_latest_completed_run`, `get_runs_by_repo` methods, 7 new unit tests, updated `sample_run` helper
- `src/application/run_manager.rs` — Added `reindex_repository` and `spawn_pipeline_for_run` methods, refactored `launch_run` to use shared pipeline spawn, updated `compute_request_hash`/`generate_run_id` match arms, 6 new unit tests, updated all `IndexRun` struct literals
- `src/application/mod.rs` — Added `reindex_repository` delegation method on `ApplicationContext` (with `repo_root`)
- `src/protocol/mcp.rs` — Added `reindex_repository` MCP tool (with `repo_root` required param)
- `tests/indexing_integration.rs` — Added 6 re-index integration tests (idempotent replay now tests while-active scenario)

### Change Log

- 2026-03-07: Implemented Story 2.9 — Re-index managed repository or workspace state deterministically. Added re-index orchestration with prior-state preservation, idempotency, and MCP tool. 22 new tests (299 → 321).
- 2026-03-07: Code review fixes — H1: idempotency check ordering, H2: pipeline launch, M1: description field, M2: removed dead ReindexRequest type.
