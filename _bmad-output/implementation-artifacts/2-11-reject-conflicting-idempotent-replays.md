# Story 2.11: Reject Conflicting Idempotent Replays

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,
I want conflicting replays of idempotent indexing mutations to fail deterministically,
so that retries cannot silently mutate state under a reused idempotency identity.

## Acceptance Criteria

1. **Conflicting Replay Is Rejected Deterministically**
   ```
   Given: A mutating indexing-related request has already been recorded with an idempotency key
   When: The same key is replayed with different effective inputs
   Then: Tokenizor rejects the replay deterministically
   And: It preserves the original request record and outcome
   ```

2. **Idempotent Replay Returns Stored Outcome**
   ```
   Given: The same key is replayed with the same effective inputs
   When: Tokenizor processes the request
   Then: It returns the stored outcome
   And: It does not execute a second conflicting mutation
   ```

## Tasks / Subtasks

- [x] Task 1: Add `ConflictingReplay` error variant (AC: #1)
  - [x] 1.1: Add `#[error("conflicting replay: {0}")]` variant `ConflictingReplay(String)` to `TokenizorError` in `src/error.rs` [Source: src/error.rs — `TokenizorError` enum]
  - [x] 1.2: Add `ConflictingReplay` arm to `is_systemic()` returning `false` — this is a client/operator error, not a systemic failure [Source: src/error.rs — `is_systemic` method]
  - [x] 1.3: Map `ConflictingReplay` in `to_mcp_error()` → `McpError::invalid_params(...)` with message including the idempotency key and guidance to retry with identical inputs or use a new key [Source: src/protocol/mcp.rs — `to_mcp_error` function]
  - [x] 1.4: Migrate `start_run_idempotent` conflicting replay error from `TokenizorError::InvalidArgument("conflicting replay...")` to `TokenizorError::ConflictingReplay(...)` [Source: src/application/run_manager.rs — `start_run_idempotent` method, ~line 165]
  - [x] 1.5: Migrate `reindex_repository` conflicting replay error from `InvalidArgument` to `ConflictingReplay` [Source: src/application/run_manager.rs — `reindex_repository` method, ~line 215]
  - [x] 1.6: Unit tests: `ConflictingReplay` Display formats correctly; `is_systemic()` returns `false`; existing conflicting-replay tests updated to assert `ConflictingReplay` variant instead of `InvalidArgument`

- [x] Task 2: Fix stale idempotency records in `start_run_idempotent` (AC: #1, #2)
  - [x] 2.1: After finding an existing idempotency record, look up the referenced run via `self.persistence.find_run(&run_id)`. Determine if the run is terminal (`IndexRunStatus::Completed | Failed | Cancelled | Aborted`) or non-terminal (`Queued | Running | Paused`). If the run is not found, treat the record as stale (orphaned). [Source: src/application/run_manager.rs — `start_run_idempotent`, ~line 150]
  - [x] 2.2: **Same hash + terminal/missing run**: Log `info!("stale idempotent record — referenced run is terminal, proceeding with new run")` and fall through to new run creation. Overwrite the stale idempotency record with the new one after run creation.
  - [x] 2.3: **Different hash + terminal/missing run**: Log `info!("stale conflicting record — referenced run is terminal, allowing new run")` and fall through to new run creation. Overwrite stale record.
  - [x] 2.4: **Same hash + active run**: Return `IdempotentRunResult::ExistingRun { run_id }` (existing behavior, idempotent replay)
  - [x] 2.5: **Different hash + active run**: Return `ConflictingReplay` error (existing behavior, now with dedicated variant)
  - [x] 2.6: Unit tests: (a) same hash + active run → idempotent replay returns stored run_id; (b) same hash + terminal run → new run created, old record overwritten; (c) different hash + active run → `ConflictingReplay` error; (d) different hash + terminal run → new run created; (e) orphaned record (run not found) → new run created

- [x] Task 3: Fix stale idempotency records in `reindex_repository` (AC: #1, #2)
  - [x] 3.1: Apply same stale-record detection pattern as Task 2 — after finding an idempotency record, check referenced run's terminal status before deciding replay vs conflict vs stale [Source: src/application/run_manager.rs — `reindex_repository`, ~line 195]
  - [x] 3.2: **Same hash + terminal/missing run**: Fall through to new reindex run creation (overwrite stale record)
  - [x] 3.3: **Different hash + terminal/missing run**: Fall through to new reindex run (overwrite stale record)
  - [x] 3.4: **Same hash + active run**: Return stored `IndexRun` (idempotent replay)
  - [x] 3.5: **Different hash + active run**: Return `ConflictingReplay` error
  - [x] 3.6: Remove the TODO comment about stale idempotency records in `reindex_repository` (resolving documented Story 2.9 bug) [Source: src/application/run_manager.rs — TODO comment near reindex idempotency check]
  - [x] 3.7: Unit tests: mirror Task 2 test cases for reindex operations — (a) same hash + active → replay; (b) same hash + terminal → new run; (c) different hash + active → `ConflictingReplay`; (d) different hash + terminal → new run; (e) orphaned record → new run

- [x] Task 4: Validate `invalidate_repository` conflicting replay behavior (AC: #1, #2)
  - [x] 4.1: Confirm domain-level check (repo already `Invalidated` → return success) correctly prevents false conflicts after re-invalidation — no code change expected, just verify coverage [Source: src/application/run_manager.rs — `invalidate_repository`, ~line 305]
  - [x] 4.2: Confirm stale record handling (repo no longer `Invalidated` after re-index → stale record bypassed, invalidation re-applied) correctly works with both same-reason and different-reason replays — the H1 fix from Story 2.10 already handles this [Source: src/application/run_manager.rs — stale detection block, ~line 345]
  - [x] 4.3: Unit tests: (a) invalidate → re-index clears invalidation → different-reason replay → re-applies with new reason; (b) invalidate → re-index clears invalidation → same-reason replay → re-applies successfully; (c) invalidate while active run → `InvalidOperation` (not `ConflictingReplay` — this is a different rejection reason)

- [x] Task 5: Integration tests for cross-operation conflicting replay lifecycle (AC: #1, #2)
  - [x] 5.1: Test: `start_index_run` → run completes → same-param retry → new run created (stale record bypassed)
  - [x] 5.2: Test: `start_index_run` → run completes → different-param retry → new run created (stale record bypassed)
  - [x] 5.3: Test: `start_index_run` active → different-param retry → `ConflictingReplay` error with original record preserved
  - [x] 5.4: Test: `reindex_repository` → run completes → same-param retry → new run created
  - [x] 5.5: Test: `reindex_repository` → run completes → different-param retry → new run created
  - [x] 5.6: Test: `reindex_repository` active → different-param retry → `ConflictingReplay` error
  - [x] 5.7: Test: `invalidate` → re-index → `invalidate` with different reason → succeeds (domain-level stale handling)
  - [x] 5.8: Test: idempotency key space isolation — verify `index::`, `reindex::`, and `invalidate::` keys never collide
  - [x] 5.9: Verify total test count increases appropriately from 350 baseline (350 → 369, +19 tests)

## Dev Notes

### Architecture Patterns and Constraints

- **Persistence model**: All durable state persists via `RegistryPersistence` to local bootstrap registry JSON file using atomic write-to-temp-then-rename with advisory file locking (fs2 crate). Do NOT wire SpacetimeDB write methods.
- **Backward compatibility**: No new persisted fields in this story — `IdempotencyRecord` and `IndexRun` structures are unchanged. The fix is purely behavioral (checking run status before deciding idempotency outcome).
- **Error handling**: Use `TokenizorError` variants. New `ConflictingReplay` variant for conflicting replays (replacing generic `InvalidArgument` usage). `InvalidOperation` remains for "active run blocks this action" rejections. `Integrity` for orphaned idempotency records referencing missing runs.
- **No Mutex across .await**: Extract data, drop guard, then call async persistence methods.
- **Idempotency model (corrected)**: The idempotency check becomes a 5-case decision tree:
  1. No prior record → proceed with new operation
  2. Same key + same hash + active run → return stored result (idempotent replay)
  3. Same key + same hash + terminal/missing run → proceed with new operation (stale record)
  4. Same key + different hash + active run → `ConflictingReplay` error
  5. Same key + different hash + terminal/missing run → proceed with new operation (stale record)
- **Stale detection strategy**: Check the referenced run's `IndexRunStatus` to determine if the idempotency record is still relevant. Terminal statuses: `Completed`, `Failed`, `Cancelled`, `Aborted`. Non-terminal: `Queued`, `Running`, `Paused`. Missing run = orphaned record = stale.
- **Invalidation stale detection**: Uses domain-level check (repo status != `Invalidated`) instead of run status. This is correct because invalidation doesn't create runs — it mutates repo status directly. No change needed to this pattern.
- **MCP tool changes**: None. Existing MCP tools (`start_index_run`, `reindex_repository`, `invalidate_indexed_state`) already delegate to the application layer methods being fixed. The error mapping update in `to_mcp_error()` is the only MCP-layer change.

### Stale Idempotency Record Bug (CRITICAL CONTEXT)

**The problem (documented as TODO in Story 2.9):**
When an index or reindex run completes (terminal state), the idempotency record remains in the registry. A subsequent request with the same idempotency key (same params) incorrectly returns the old terminal run instead of launching a new one. A request with different params is incorrectly rejected as a "conflicting replay" even though the old run is finished and no concurrent mutation risk exists.

**The fix:**
Before deciding "idempotent replay" vs "conflicting replay", check whether the referenced run is still active. If the run is terminal or missing, the idempotency record is stale and should be overwritten. Only active runs warrant idempotency protection.

**Why `invalidate_repository` already works:**
Story 2.10's H1 fix added domain-level staleness detection: if the repo is NOT `Invalidated` (meaning a re-index restored it to `Ready`), any existing idempotency record is treated as stale and the invalidation is re-applied. This is the correct pattern for non-run-based operations.

### Build Order (MANDATORY)

Follow the established build order:
1. Error variant extension (Task 1 — no dependencies)
2. Fix `start_run_idempotent` stale handling (Task 2 — depends on Task 1)
3. Fix `reindex_repository` stale handling (Task 3 — depends on Task 1)
4. Validate `invalidate_repository` (Task 4 — depends on Task 1, mostly test-only)
5. Integration tests (Task 5 — depends on all above)

### Project Structure Notes

- All modifications extend existing files — NO new files expected
- Key files to modify:
  - `src/error.rs` — `ConflictingReplay` variant on `TokenizorError`, `is_systemic()` arm
  - `src/application/run_manager.rs` — stale record detection in `start_run_idempotent` and `reindex_repository`, error variant migration, TODO removal, ~15 unit tests
  - `src/protocol/mcp.rs` — `to_mcp_error()` mapping for `ConflictingReplay`
  - `tests/indexing_integration.rs` — ~9 integration tests for lifecycle stale handling and conflict rejection
- Naming convention: `snake_case` for functions/modules, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for constants

### Previous Story Intelligence (Story 2.10)

**Key learnings to apply:**
- Domain-level idempotency (check actual state) fires BEFORE key-based idempotency to prevent false conflicts — apply same principle to run-based operations by checking run terminal status
- `save_repository` was added to `RegistryPersistence` for integration test seeding
- Pipeline completion handler clears invalidation on successful run (`Invalidated` → `Ready`) — this is the trigger that makes invalidation idempotency records stale
- H1 fix pattern: when domain state says "no longer relevant", treat existing idempotency record as stale and fall through to re-apply
- Test count: 321 → 350 (+29 tests: 21 unit + 8 integration)

**Patterns from 2.10 to reuse:**
- Two-tier idempotency: domain-level check (is run terminal?) then key-based check
- Stale record logging: `debug!` level for stale detection, `info!` for fallthrough decision
- Integration test pattern: lifecycle tests that exercise full run-then-retry scenarios
- Unit test pattern: seeded idempotency records + seeded run records to test all 5 cases

**Code review findings from 2.10 to avoid:**
- H1 (stale idempotency causing silent failure) — this is EXACTLY the bug we're fixing for index/reindex
- Ensure `PartialEq, Eq` on any new types per project convention

### Git Intelligence

Recent commit pattern: `docs: create Story X` → `feat: implement Story X` → `fix: address code review findings for Story X`

Key commits for context:
- `d2555cf` feat: implement Story 2.10 — invalidate indexed state for untrusted use
- `7696d7c` feat: implement Story 2.9 — re-index managed repository deterministically
- `d105124` fix: address code review findings for Story 2.8

All recent work extends existing files (0 new files in 2.8, 2.9, 2.10). Consistent conventional commit format.

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

### Existing Idempotency Infrastructure (CRITICAL REFERENCE)

**IdempotencyRecord fields:** `operation: String`, `idempotency_key: String`, `request_hash: String`, `status: IdempotencyStatus`, `result_ref: Option<String>`, `created_at_unix_ms: u64`, `expires_at_unix_ms: Option<u64>`

**IdempotencyStatus enum:** `Pending`, `Succeeded`, `Failed` — NOTE: not reliably updated after creation for run-based operations (index/reindex save as `Pending`, invalidation saves as `Succeeded`)

**Key computation patterns (DO NOT CHANGE):**
- Index: key = `index::{repo_id}::{workspace_id}`, hash = `digest_hex("index:{repo_id}:{workspace_id}:{mode_str}")`
- Reindex: key = `reindex::{repo_id}::{workspace_id}`, hash = `digest_hex("index:{repo_id}:{workspace_id}:reindex")`
- Invalidate: key = `invalidate::{repo_id}::{workspace_id}`, hash = `digest_hex("invalidate:{repo_id}:{workspace_id}:{reason}")`

**Persistence methods (use as-is):**
- `find_idempotency_record(&self, key: &str) -> Result<Option<IdempotencyRecord>>`
- `save_idempotency_record(&self, record: &IdempotencyRecord) -> Result<()>` (upsert — updates existing if key matches)
- `find_run(&self, run_id: &str) -> Result<Option<IndexRun>>` — use this to check referenced run's terminal status

**Terminal run statuses (for stale detection):**
```rust
fn is_terminal(status: &IndexRunStatus) -> bool {
    matches!(status, IndexRunStatus::Completed | IndexRunStatus::Failed
        | IndexRunStatus::Cancelled | IndexRunStatus::Aborted)
}
```

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 2, Story 2.11, FR16]
- [Source: _bmad-output/planning-artifacts/architecture.md — Mutation Safety Model: "idempotent operations must reject conflicting replays deterministically"]
- [Source: _bmad-output/planning-artifacts/prd.md — FR16: "rejection of conflicting replays where the same idempotency identity is reused with different effective inputs"]
- [Source: _bmad-output/implementation-artifacts/2-10-invalidate-indexed-state-so-it-is-no-longer-trusted.md — H1 fix for stale idempotency, two-tier idempotency pattern, completion notes]
- [Source: _bmad-output/project-context.md — Idempotency section, error handling ADR-3, MCP tool patterns ADR-4]
- [Source: _bmad-output/implementation-artifacts/sprint-status.yaml — Story status tracking]
- [Source: src/error.rs — TokenizorError enum, is_systemic() method]
- [Source: src/domain/idempotency.rs — IdempotencyRecord, IdempotencyStatus]
- [Source: src/application/run_manager.rs — start_run_idempotent, reindex_repository, invalidate_repository, compute_request_hash, compute_invalidation_request_hash]
- [Source: src/storage/registry_persistence.rs — find_idempotency_record, save_idempotency_record, find_run]
- [Source: src/protocol/mcp.rs — to_mcp_error() mapping]
- [Source: tests/indexing_integration.rs — existing idempotency integration tests]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

### Completion Notes List

- Added `ConflictingReplay(String)` error variant to `TokenizorError` with `is_systemic() = false` and MCP error mapping to `invalid_params` with guidance message
- Migrated existing `InvalidArgument("conflicting replay...")` errors to `ConflictingReplay` in both `start_run_idempotent` and `reindex_repository`
- Implemented 5-case stale idempotency record decision tree in `start_run_idempotent`: checks referenced run's terminal status via `IndexRunStatus::is_terminal()` before deciding replay vs conflict vs stale fallthrough
- Applied identical stale-record detection pattern to `reindex_repository`, replacing the old TODO comment from Story 2.9
- Removed TODO(Story 2.9 bug) comment — resolved by the stale detection fix
- Validated `invalidate_repository` already handles staleness correctly via domain-level check (existing Story 2.10 H1 fix)
- Updated existing conflicting-replay tests to assert `ConflictingReplay` variant instead of `InvalidArgument`
- Updated existing integration test `test_reindex_conflicting_replay_returns_error` to reference an active run instead of an orphaned record
- Test count: 350 → 369 (+19 tests: 10 unit + 9 integration)

### Implementation Plan

Followed the 5-case idempotency decision tree from Dev Notes:
1. No prior record → proceed with new operation
2. Same key + same hash + active run → return stored result (idempotent replay)
3. Same key + same hash + terminal/missing run → proceed with new operation (stale record)
4. Same key + different hash + active run → `ConflictingReplay` error
5. Same key + different hash + terminal/missing run → proceed with new operation (stale record)

Used existing `IndexRunStatus::is_terminal()` method for stale detection. Missing run (orphaned record) treated as stale.

### File List

- `src/error.rs` — Added `ConflictingReplay(String)` variant, `is_systemic()` arm, 2 unit tests
- `src/protocol/mcp.rs` — Added `ConflictingReplay` arm to `to_mcp_error()` with guidance message
- `src/application/run_manager.rs` — Rewrote idempotency checks in `start_run_idempotent` and `reindex_repository` with 5-case stale detection; removed TODO comment; migrated error variants; updated 2 existing tests; added 8 new unit tests
- `tests/indexing_integration.rs` — Added 9 integration tests for stale lifecycle and key space isolation; updated 1 existing test

## Change Log

- 2026-03-08: Implemented Story 2.11 — reject conflicting idempotent replays with dedicated `ConflictingReplay` error variant and stale record detection (18 new tests)
- 2026-03-08: Code review fix — added missing same-reason invalidation replay test (Task 4.3b), test count 368 → 369
