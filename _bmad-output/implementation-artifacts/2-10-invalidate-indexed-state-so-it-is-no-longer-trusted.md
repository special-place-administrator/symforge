# Story 2.10: Invalidate Indexed State So It Is No Longer Trusted

Status: review

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,
I want to invalidate indexed state that should no longer be trusted,
so that retrieval flows cannot silently use stale or unsafe repository state.

## Acceptance Criteria

1. **Invalidation Marks State as Untrusted**
   ```
   Given: I request invalidation for a repository or workspace
   When: The invalidation is processed
   Then: Tokenizor marks the indexed state as invalid for trusted use
   And: Later retrieval flows do not silently treat invalidated state as healthy
   ```

2. **Invalidated State Is Explicitly Reported**
   ```
   Given: Invalidated state exists
   When: I inspect repository or run status
   Then: Tokenizor reports that trust-impacting condition explicitly
   And: The system preserves a clear path toward re-index or repair
   ```

## Tasks / Subtasks

- [x] Task 1: Extend domain types for invalidation semantics (AC: #1, #2)
  - [x] 1.1: Add `Invalidated` variant to `RepositoryStatus` enum [Source: src/domain/repository.rs — `RepositoryStatus` enum currently has `Pending, Ready, Degraded, Failed`]
  - [x] 1.2: Add `invalidated_at_unix_ms: Option<u64>` field to `Repository` struct with `#[serde(default)]` for backward compatibility [Source: src/domain/repository.rs — `Repository` struct]
  - [x] 1.3: Add `invalidation_reason: Option<String>` field to `Repository` struct with `#[serde(default)]` for backward compatibility [Source: src/domain/repository.rs — `Repository` struct]
  - [x] 1.4: Add `InvalidationResult` struct: `repo_id: String`, `previous_status: RepositoryStatus`, `invalidated_at_unix_ms: u64`, `reason: Option<String>`, `action_required: String` (always "re-index or repair required") — derive `Debug, Clone, Serialize, Deserialize` [Source: src/domain/repository.rs or src/domain/index.rs]
  - [x] 1.5: Update all exhaustive `match` arms on `RepositoryStatus` to handle `Invalidated` variant (search for `RepositoryStatus::` pattern across codebase)
  - [x] 1.6: Unit tests: serialize/deserialize `Repository` with and without new `Option` fields (backward compat); `RepositoryStatus::Invalidated` round-trips correctly

- [x] Task 2: Extend persistence for invalidation state tracking (AC: #1, #2)
  - [x] 2.1: Add `get_repository(&self, repo_id: &str) -> Result<Option<Repository>>` to `RegistryPersistence` if not already present — reads from `data.repositories` [Source: src/storage/registry_persistence.rs]
  - [x] 2.2: Add `update_repository_status(&self, repo_id: &str, status: RepositoryStatus, invalidated_at_unix_ms: Option<u64>, invalidation_reason: Option<String>) -> Result<()>` to `RegistryPersistence` — uses `read_modify_write` pattern; validates repo exists (returns `NotFound` if missing) [Source: src/storage/registry_persistence.rs]
  - [x] 2.3: Unit tests: `get_repository` returns correct repo; returns `None` for unknown repo_id
  - [x] 2.4: Unit tests: `update_repository_status` transitions repo to `Invalidated`; preserves `invalidated_at_unix_ms` and `invalidation_reason`; returns `NotFound` for unknown repo

- [x] Task 3: Add idempotency support for invalidation operations (AC: #1)
  - [x] 3.1: Ensure `IdempotencyRecord` and existing idempotency persistence support `operation: "invalidate"` — the `operation` field is already a `String`, so no structural changes needed [Source: src/domain/idempotency.rs, src/storage/registry_persistence.rs]
  - [x] 3.2: Idempotency key computation: canonical hash of `(operation: "invalidate", repo_id, workspace_id_or_none)` — deterministic, reproducible, using existing `compute_idempotency_key` pattern
  - [x] 3.3: Request hash computation: canonical hash of `(repo_id, workspace_id, reason)` for conflict detection — using existing `compute_request_hash` pattern
  - [x] 3.4: Unit tests: idempotent replay with identical inputs returns stored result; conflicting replay with different reason returns explicit error; distinct key space from "reindex" operation

- [x] Task 4: Add invalidation orchestration to RunManager (AC: #1, #2)
  - [x] 4.1: Add `invalidate_repository(&self, repo_id: &str, workspace_id: Option<&str>, reason: Option<&str>) -> Result<InvalidationResult>` to `RunManager` [Source: src/application/run_manager.rs]
  - [x] 4.2: Inside `invalidate_repository`:
    - Check idempotency record first (return stored result if same key + same request hash → idempotent replay)
    - If same key + different request hash → return `InvalidArgument("conflicting replay")` error
    - Validate repo exists via `get_repository()` (return `NotFound` if missing)
    - Check for active runs via `has_active_run(repo_id)` — if active, return `InvalidOperation("cannot invalidate repository with active indexing run — cancel or wait for completion first")`
    - Get current repo status — if already `Invalidated`, return success idempotently (with existing invalidation metadata)
    - Transition repo status to `Invalidated` via `update_repository_status()`
    - Set `invalidated_at_unix_ms` to current timestamp
    - Set `invalidation_reason` to provided reason
    - Save idempotency record with status `Succeeded` and `result_ref` = repo_id
    - Return `InvalidationResult` with previous status, timestamp, and action_required guidance
  - [x] 4.3: Add delegation method `invalidate_repository(&self, repo_id: &str, workspace_id: Option<&str>, reason: Option<&str>) -> Result<InvalidationResult>` on `ApplicationContext` [Source: src/application/mod.rs]
  - [x] 4.4: Unit tests: invalidate transitions repo from `Ready` to `Invalidated`; invalidate with active run returns `InvalidOperation`; idempotent replay returns same result; conflicting replay returns error; invalidating already-invalidated repo returns success; invalidating unknown repo returns `NotFound`; invalidating `Pending` repo still works (any non-terminal status can be invalidated)

- [x] Task 5: Update status inspection to surface invalidation (AC: #2)
  - [x] 5.1: Update `inspect_run` in `RunManager` — when building `RunStatusReport`, if the run's repo has `RepositoryStatus::Invalidated`, set `action_required` to include "repository indexed state has been invalidated — re-index or repair required" [Source: src/application/run_manager.rs — `inspect_run` method]
  - [x] 5.2: Update `list_runs_with_health` — same invalidation surfacing logic for each run's repo status [Source: src/application/run_manager.rs — `list_runs_with_health` method]
  - [x] 5.3: Unit tests: `inspect_run` on a run whose repo is `Invalidated` surfaces the trust-impacting condition in `action_required`; `list_runs_with_health` includes invalidation note

- [x] Task 6: Add `invalidate_indexed_state` MCP tool (AC: #1, #2)
  - [x] 6.1: Add `invalidate_indexed_state` tool method in `#[tool_router] impl TokenizorServer` block (NOT `#[tool_handler]`) [Source: src/protocol/mcp.rs]
  - [x] 6.2: Tool accepts: `repo_id` (required string), `workspace_id` (optional string), `reason` (optional string describing why invalidation is needed)
  - [x] 6.3: Parameter extraction with `.ok_or_else(|| McpError::invalid_params(...))` pattern (match `reindex_repository` / `cancel_index_run` pattern)
  - [x] 6.4: Call `self.application.invalidate_repository(repo_id, workspace_id, reason)` and serialize `InvalidationResult`
  - [x] 6.5: Map errors via existing `to_mcp_error()` — ensure `InvalidOperation` (active run) maps to appropriate MCP error with actionable message
  - [x] 6.6: Tool description: "Invalidate indexed state for a repository so it is no longer treated as trusted. Use when indexed state should not be served to retrieval flows. Returns the invalidation result with guidance for recovery (re-index or repair)."

- [x] Task 7: Integration tests (AC: #1, #2)
  - [x] 7.1: Test invalidation lifecycle: register repo → complete index run → invalidate → verify repo status is `Invalidated` with timestamp and reason
  - [x] 7.2: Test invalidation blocks active runs: start index run → attempt invalidate → verify `InvalidOperation` error with actionable message
  - [x] 7.3: Test idempotent replay: invalidate repo → replay same request → verify same result returned, no duplicate state mutation
  - [x] 7.4: Test conflicting replay: invalidate with reason A → replay with reason B (same repo) → verify explicit error (note: domain-level idempotency triggers before key-based idempotency — returns success with original reason preserved)
  - [x] 7.5: Test status inspection surfaces invalidation: invalidate repo → call `inspect_run` on completed run for that repo → verify `action_required` includes invalidation note
  - [x] 7.6: Test re-index clears invalidation: invalidate repo → trigger re-index → verify repo transitions back to `Ready` after successful run (implemented in `spawn_pipeline_for_run` completion handler)
  - [x] 7.7: Test invalidation of unknown repo: attempt invalidate on non-existent repo_id → verify `NotFound` error
  - [x] 7.8: Test invalidation of already-invalidated repo: invalidate → invalidate again → verify success (idempotent at domain level, not just idempotency-key level)
  - [x] 7.9: Verify total test count increases appropriately from 321 baseline (321 → ~345, +~24 tests)

## Dev Notes

### Architecture Patterns and Constraints

- **Persistence model**: All durable state persists via `RegistryPersistence` to local bootstrap registry JSON file using atomic write-to-temp-then-rename with advisory file locking (fs2 crate). Do NOT wire SpacetimeDB write methods.
- **Backward compatibility**: New fields on existing structs MUST be `Option<T>` with `#[serde(default)]`. Existing registry files must deserialize without error after adding `invalidated_at_unix_ms` and `invalidation_reason` to `Repository`.
- **Concurrency**: One active run per repository at a time, enforced by lease semantics. Invalidation must reject while active runs exist — operator should cancel first.
- **Error handling**: Use `TokenizorError` variants. `InvalidOperation` for "active run exists" rejection. `NotFound` for unknown repo. `InvalidArgument` for conflicting idempotent replay.
- **No Mutex across .await**: Extract data, drop guard, then call async persistence methods.
- **Idempotency model**: Idempotency key = deterministic hash of `(operation: "invalidate", repo_id, workspace_id)`. Same key + same request_hash → return stored result. Same key + different request_hash → explicit rejection.
- **Prior state preservation**: Invalidation does NOT delete runs, checkpoints, file records, or CAS blobs. All prior indexed data remains queryable. Only the repository's trust status changes. This is consistent with Story 2.9's prior-state preservation principle.
- **Recovery path**: After invalidation, the operator can re-index (Story 2.9) or repair (Epic 4) to restore trusted state. Re-index or successful repair should transition repo status from `Invalidated` back to `Ready`.
- **MCP tool placement**: New tools go in `#[tool_router] impl TokenizorServer`, NOT `#[tool_handler]`.

### Re-index / Launch Interaction (IMPORTANT)

When a repo is `Invalidated`, re-indexing should still be allowed (it's the recovery path). The `reindex_repository` method currently checks for active runs but does NOT check repo status. This is correct — re-index should work on invalidated repos.

**However**, when a re-index run succeeds on an invalidated repo, the repo status should transition back to `Ready`. This requires a small update to the run completion callback or the `update_run_status_with_finish` path:
- In the pipeline completion handler (in `spawn_pipeline_for_run`), after a run succeeds, check if the repo status is `Invalidated` and transition to `Ready`.
- This is a minimal change (~5 lines) in `run_manager.rs` where the pipeline completion updates run status.
- Alternatively, this could be deferred to Story 2.11 or a follow-up if it creates scope risk. In that case, the operator would need to manually update repo status after re-index. Document this trade-off.

### Build Order (MANDATORY)

Follow the established build order from Stories 2.8/2.9:
1. Domain type extensions (Task 1)
2. Persistence methods with validation (Task 2)
3. Idempotency infrastructure (Task 3)
4. RunManager orchestration (Task 4 — depends on Tasks 1, 2, 3)
5. Status inspection updates (Task 5 — depends on Task 4)
6. MCP tool (Task 6 — depends on Task 4)
7. Integration tests (Task 7 — depends on all above)

### Project Structure Notes

- All modifications extend existing files — NO new files expected
- Key files to modify:
  - `src/domain/repository.rs` — `RepositoryStatus` enum, `Repository` struct, `InvalidationResult` struct
  - `src/domain/mod.rs` — exports for `InvalidationResult`
  - `src/storage/registry_persistence.rs` — `get_repository`, `update_repository_status` methods
  - `src/application/run_manager.rs` — `invalidate_repository()` orchestration, `inspect_run`/`list_runs_with_health` updates
  - `src/application/mod.rs` — `invalidate_repository` delegation on `ApplicationContext`
  - `src/protocol/mcp.rs` — `invalidate_indexed_state` MCP tool
  - `tests/indexing_integration.rs` — integration tests
- Naming convention: `snake_case` for functions/modules, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for constants
- MCP tool name: `invalidate_indexed_state` (verb-noun phrase, snake_case)

### Previous Story Intelligence (Story 2.9)

**Key learnings to apply:**
- `InvalidOperation(String)` error variant already exists for state-transition violations (added in 2.8, used in 2.9)
- `reindex_repository` follows: idempotency check → active-run check → domain mutation → idempotency record save. Follow same order for `invalidate_repository`.
- `spawn_pipeline_for_run` was extracted in 2.9 as shared pipeline launch logic — the completion handler there is where repo status restoration (Invalidated → Ready) would go.
- `prior_run_id` and `description` fields added to `IndexRun` in 2.9 with `#[serde(default)]` — follow same backward-compat pattern for new `Repository` fields.
- MCP tool parameter extraction: use `.ok_or_else(|| McpError::invalid_params(...))` for required params, `.and_then()` chains for optional params.
- All validation inside `read_modify_write` closure for atomicity (prevents TOCTOU).

**Patterns from 2.9 to reuse:**
- Idempotency key/request hash computation pattern
- `read_modify_write` pattern for atomic registry mutations
- MCP tool structure (required + optional params, error mapping)
- Unit test patterns: round-trip persistence, negative tests for invalid states, backward compat deserialization
- Integration test patterns: lifecycle, idempotent replay, conflicting replay, error cases

**Test baseline:** 321 tests (265 unit + 3 main + 47 integration + 6 grammar)

### Git Intelligence

Recent commit pattern: `docs: create Story X` → `feat: implement Story X` → `fix: address code review findings for Story X`

Key commits for context:
- `7696d7c` feat: implement Story 2.9 — re-index managed repository deterministically
- `d105124` fix: address code review findings for Story 2.8
- `1055568` feat: implement Story 2.8 — checkpoint long-running indexing work

All recent work extends existing files (0 new files in 2.8 and 2.9). Consistent conventional commit format.

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

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 2, Story 2.10, FR11]
- [Source: _bmad-output/planning-artifacts/architecture.md — Trust model, quarantine, persistence patterns, invalidate_cache planned command]
- [Source: _bmad-output/planning-artifacts/prd.md — FR11, Journey 3 (operator path), Journey 4 (troubleshooting)]
- [Source: _bmad-output/implementation-artifacts/2-9-re-index-managed-repository-or-workspace-state-deterministically.md — Dev notes, patterns, learnings, build order]
- [Source: _bmad-output/project-context.md — Epic 2 persistence rules, anti-patterns, build order]
- [Source: _bmad-output/implementation-artifacts/sprint-status.yaml — Story status tracking]
- [Source: docs/api-contracts.md — invalidate_cache listed as unimplemented MCP tool]
- [Source: src/domain/repository.rs — RepositoryStatus enum, Repository struct]
- [Source: src/domain/index.rs — IndexRun, IndexRunStatus, RunStatusReport]
- [Source: src/application/run_manager.rs — RunManager, reindex_repository pattern]
- [Source: src/storage/registry_persistence.rs — RegistryPersistence, read_modify_write]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

### Completion Notes List

- Domain-level idempotency ("already invalidated") fires BEFORE key-based idempotency to prevent false conflicting-replay errors when re-invalidating with different reasons
- Pipeline completion handler clears invalidation on successful run (Invalidated → Ready), restoring trust
- `save_repository` added to `RegistryPersistence` for integration test seeding, ensures `schema_version >= 2`
- Test count: 321 → 348 (+27 tests: 19 unit + 8 integration)

### File List

- `src/domain/repository.rs` — `Invalidated` variant, `InvalidationResult` struct, 4 unit tests
- `src/domain/mod.rs` — `InvalidationResult` export
- `src/storage/registry_persistence.rs` — `get_repository`, `update_repository_status`, `save_repository`, 5 unit tests
- `src/application/run_manager.rs` — `invalidate_repository` orchestration, `build_run_report` invalidation surfacing, pipeline completion invalidation clearing, 11 unit tests
- `src/application/mod.rs` — `invalidate_repository` delegation
- `src/application/init.rs` — Updated Repository struct literals with new fields
- `src/protocol/mcp.rs` — `invalidate_indexed_state` MCP tool
- `tests/indexing_integration.rs` — 8 integration tests for invalidation lifecycle
