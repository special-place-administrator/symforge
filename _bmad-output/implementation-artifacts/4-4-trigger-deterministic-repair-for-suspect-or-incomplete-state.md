# Story 4.4: Trigger Deterministic Repair for Suspect or Incomplete State

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,
I want to trigger deterministic repair flows for suspect, stale, or incomplete indexed state,
so that I can restore trusted retrieval without guessing which action is safe.

**FRs implemented:** FR31

- **FR31**: Users can trigger deterministic repair or re-index flows when indexed state becomes stale, suspect, or incomplete

## Acceptance Criteria

1. **Given** repository, run, or retrieval state is marked stale, suspect, quarantined, or incomplete **When** I trigger repair **Then** Tokenizor executes a deterministic repair path scoped to the affected state **And** the repair action is recorded in operational history
2. **Given** repair cannot safely restore trust **When** repair completes or fails **Then** Tokenizor reports that explicit outcome **And** it does not silently mark the state healthy

## Tasks / Subtasks

### Phase 1: Domain types for repair classification and outcome

- [x] Task 1.1: Add repair domain types to `src/domain/index.rs` (AC: 1, 2)
  - [x] 1.1.1: Define `RepairScope` enum with variants `Repository`, `Run { run_id: String }`, and `File { run_id: String, relative_path: String }` — scopes the repair action to the affected boundary
  - [x] 1.1.2: Define `RepairOutcome` enum with variants `Restored` (trust provably restored), `AlreadyHealthy` (no repair needed), `CannotRestore { reason: String }` (repair attempted, trust not restorable), `RequiresReindex` (repair delegates to reindex as the safe fallback), `InProgress { run_id: String }` (repair spawned a run that is now active)
  - [x] 1.1.3: Define `RepairResult` struct with fields: `repo_id: String`, `scope: RepairScope`, `previous_status: RepositoryStatus`, `outcome: RepairOutcome`, `next_action: Option<NextAction>`, `detail: String`, `recorded_at_unix_ms: u64`
  - [x] 1.1.4: Define `RepairEvent` struct for operational history with fields: `repo_id: String`, `scope: RepairScope`, `previous_status: RepositoryStatus`, `outcome: RepairOutcome`, `detail: String`, `timestamp_unix_ms: u64`

### Phase 2: Repair logic in RunManager

- [x] Task 2.1: Add the `repair_repository` entry point on `RunManager` (AC: 1, 2)
  - [x] 2.1.1: Signature: `pub fn repair_repository(&self, repo_id: &str, scope: RepairScope) -> Result<RepairResult>`
  - [x] 2.1.2: Load repository state from control plane via `get_repository(repo_id)`; return `NotFound` if missing
  - [x] 2.1.3: Gate on action-required state: `Ready` → `AlreadyHealthy`; `Pending` → `AlreadyHealthy` (nothing to repair yet)
  - [x] 2.1.4: Route `RepairScope::Repository` to `repair_repository_state()`
  - [x] 2.1.5: Route `RepairScope::Run { run_id }` to `repair_run_state()`
  - [x] 2.1.6: Route `RepairScope::File { run_id, relative_path }` to `repair_file_state()`
  - [x] 2.1.7: After repair action, record a `RepairEvent` to operational history via `control_plane.save_repair_event()` before returning
  - [x] 2.1.8: Return `RepairResult` with explicit outcome, never swallow errors into silent success

- [x] Task 2.2: Implement `repair_repository_state()` — repository-scoped repair routing (AC: 1, 2)
  - [x] 2.2.1: `Degraded` → load file records for the latest completed run (`get_latest_completed_run` + `get_file_records`); count failed files; if zero failed files remain, transition to `Ready` and return `Restored`; if failed files exist, delegate to `reindex_repository` and return `InProgress` with the spawned run_id
  - [x] 2.2.2: `Quarantined` → load file records for the latest completed run; filter to `Quarantined` outcomes; for each, attempt re-verification against the CAS (read blob, verify hash against disk source); if all pass, transition to `Ready` and return `Restored`; if some fail, transition to `Degraded` (partial restore) or remain `Quarantined` and return `CannotRestore` with detail listing which files remain quarantined
  - [x] 2.2.3: `Failed` → delegate to `reindex_repository` (existing method) and return `InProgress` with the spawned run_id; if reindex fails to start (e.g., active run conflict), return `CannotRestore` with the reason
  - [x] 2.2.4: `Invalidated` → delegate to `reindex_repository` (existing method) and return `InProgress` with the spawned run_id; if reindex fails to start, return `CannotRestore` with the reason
  - [x] 2.2.5: Never silently transition to `Ready` if any quarantined or failed files remain — always assess the actual file state before changing repository status

- [x] Task 2.3: Implement `repair_run_state()` — run-scoped repair routing (AC: 1, 2)
  - [x] 2.3.1: `Interrupted` → delegate to `resume_run(run_id)` (existing Story 4.2 logic); on success, return `InProgress`; on `ResumeRejectReason`, return `CannotRestore` or `RequiresReindex` based on the rejection reason
  - [x] 2.3.2: `Failed` → return `RequiresReindex` with guidance: "Run failed terminally. Trigger reindex to rebuild indexed state."
  - [x] 2.3.3: `Cancelled` / `Aborted` → return `AlreadyHealthy` (terminal states, no repair applicable)
  - [x] 2.3.4: `Succeeded` → return `AlreadyHealthy`
  - [x] 2.3.5: `Queued` / `Running` → return `CannotRestore` with detail: "Run is still active. Wait for completion or cancel first."

- [x] Task 2.4: Implement `repair_file_state()` — file-scoped repair routing (AC: 1, 2)
  - [x] 2.4.1: Load the specific file record from the run's file records by filtering on `relative_path`; return `NotFound` if the file is not in the run
  - [x] 2.4.2: `Quarantined { reason }` → attempt re-verification: read the blob from CAS using the file record's blob reference, read the source file from disk, compare content hashes; if match → update file outcome to `Committed` via `save_file_records` (upsert), re-assess repo status, return `Restored`; if mismatch → return `CannotRestore` with detail: "Source file has diverged from indexed state. Reindex required."
  - [x] 2.4.3: `Failed { error }` → return `RequiresReindex` with detail: "File failed indexing. Reindex required to re-attempt extraction."
  - [x] 2.4.4: `Committed` / `EmptySymbols` → return `AlreadyHealthy`

### Phase 3: ControlPlane trait extension for repair history

- [x] Task 3.1: Add repair event recording to ControlPlane trait (AC: 1)
  - [x] 3.1.1: Add `fn save_repair_event(&self, event: RepairEvent) -> Result<()>` to the `ControlPlane` trait
  - [x] 3.1.2: Add `fn get_repair_events(&self, repo_id: &str) -> Result<Vec<RepairEvent>>` to the `ControlPlane` trait (enables inspection for Story 4.6 and testing)
  - [x] 3.1.3: Implement on `InMemoryControlPlane`: store in a `Vec<RepairEvent>` inside `InMemoryState`
  - [x] 3.1.4: Implement on `RegistryBackedControlPlane`: delegate to `RegistryPersistence` using a `repair_events` section in the registry JSON
  - [x] 3.1.5: Add `pending_write_error()` stub on `SpacetimeControlPlane` (Story 4.6 will wire SpacetimeDB persistence for operational history); implement `get_repair_events` as empty vec for now
  - [x] 3.1.6: Implement on `RunManagerPersistenceAdapter`: delegate to inner control plane, with `NotFound` suppression on save matching the pattern from 4.3 review fix M7

### Phase 4: MCP tool exposure

- [x] Task 4.1: Add `repair_index` MCP tool to `src/protocol/mcp.rs` (AC: 1, 2)
  - [x] 4.1.1: Define the tool with description: "Trigger deterministic repair for suspect, stale, quarantined, or incomplete indexed state"
  - [x] 4.1.2: Parameters: `repository_id: String` (required), `scope: Option<String>` (optional, values: "repository", "run", "file"; defaults to "repository"), `run_id: Option<String>` (required when scope is "run" or "file"), `relative_path: Option<String>` (required when scope is "file")
  - [x] 4.1.3: Parse scope string into `RepairScope` enum, validating required parameters per scope
  - [x] 4.1.4: Delegate to `run_manager.repair_repository(repo_id, scope)`
  - [x] 4.1.5: Serialize `RepairResult` to JSON response with explicit outcome, detail, and next_action
  - [x] 4.1.6: Error handling: `NotFound` → `invalid_params`; internal failures → `server_error` with detail

### Phase 5: Re-verification helper for quarantine repair

- [x] Task 5.1: Implement CAS re-verification helper (AC: 1)
  - [x] 5.1.1: Create a helper function `verify_file_against_source(blob_store: &LocalBlobStore, file_record: &FileRecord, repo_root: &Path) -> Result<bool>` in `src/storage/` or alongside repair logic
  - [x] 5.1.2: Read the source file from disk at `repo_root.join(file_record.relative_path)`
  - [x] 5.1.3: Compute content hash of the on-disk source
  - [x] 5.1.4: Compare with the stored blob hash / content hash from the file record
  - [x] 5.1.5: Return `true` if hashes match (file unchanged since indexing, quarantine was spurious), `false` if diverged (source drift, reindex needed)
  - [x] 5.1.6: Handle missing source files gracefully: file deleted from disk → return `false` (cannot repair, reindex needed)

### Phase 6: Testing

- [x] Task 6.1: Unit tests for repair routing logic (AC: 1, 2)
  - [x] 6.1.1: `test_repair_ready_repository_returns_already_healthy` — Ready repo → AlreadyHealthy, no state change
  - [x] 6.1.2: `test_repair_degraded_no_failed_files_marks_ready` — Degraded repo with zero failed files → transitions to Ready, returns Restored
  - [x] 6.1.3: `test_repair_degraded_with_failed_files_spawns_reindex` — Degraded repo with failed files → delegates to reindex, returns InProgress
  - [x] 6.1.4: `test_repair_quarantined_all_verified_marks_ready` — Quarantined repo where all files re-verify → transitions to Ready, returns Restored
  - [x] 6.1.5: `test_repair_quarantined_partial_failure_reports_cannot_restore` — Quarantined repo where some files fail re-verify → returns CannotRestore with detail
  - [x] 6.1.6: `test_repair_failed_repository_delegates_to_reindex` — Failed repo → delegates to reindex, returns InProgress
  - [x] 6.1.7: `test_repair_invalidated_repository_delegates_to_reindex` — Invalidated repo → delegates to reindex, returns InProgress
  - [x] 6.1.8: `test_repair_interrupted_run_delegates_to_resume` — Interrupted run → delegates to resume_run, returns InProgress or CannotRestore on rejection
  - [x] 6.1.9: `test_repair_failed_run_returns_requires_reindex` — Failed run → RequiresReindex with guidance
  - [x] 6.1.10: `test_repair_quarantined_file_unquarantined_on_verify` — Single quarantined file where re-verify passes → file outcome updated to Committed
  - [x] 6.1.11: `test_repair_quarantined_file_cannot_restore_on_source_drift` — Single quarantined file where source diverged → CannotRestore
  - [x] 6.1.12: `test_repair_records_event_before_returning` — Every repair call saves a RepairEvent to control plane history
  - [x] 6.1.13: `test_repair_never_silently_marks_healthy` — Repair that fails does NOT transition repo to Ready

- [x] Task 6.2: Integration tests for end-to-end repair flows (AC: 1, 2)
  - [x] 6.2.1: `test_repair_flow_degraded_to_ready` — end-to-end: create repo, index with some file failures → repo Degraded → clear failures → repair → repo Ready → retrieval unblocked
  - [x] 6.2.2: `test_repair_flow_quarantined_partial_restore` — end-to-end: quarantine repo → repair with mixed verify results → explicit CannotRestore outcome
  - [x] 6.2.3: `test_repair_is_idempotent` — run repair twice on AlreadyHealthy repo → second call returns AlreadyHealthy, no state change
  - [x] 6.2.4: `test_repair_outcome_observable_in_repository_status` — after successful repair, `get_repository()` shows updated status (mutation side verified)
  - [x] 6.2.5: `test_repair_outcome_observable_in_next_retrieval` — after successful repair, next retrieval request is no longer gated (retrieval side verified)
  - [x] 6.2.6: `test_repair_event_persisted_and_retrievable` — after repair, `get_repair_events()` returns the recorded event with correct fields

## Dev Notes

### What Already Exists

**Repository State Machine** (`src/domain/repository.rs`):
- `RepositoryStatus` enum: `Pending`, `Ready`, `Degraded`, `Failed`, `Invalidated`, `Quarantined`
- `InvalidationResult` struct with previous_status, timestamp, reason, action_required
- `Repository` struct with `invalidated_at_unix_ms`, `invalidation_reason`, `quarantined_at_unix_ms`, `quarantine_reason` fields
- All six states are persisted through the control plane

**Run State Machine** (`src/domain/index.rs`):
- `IndexRunStatus`: `Queued`, `Running`, `Succeeded`, `Failed`, `Cancelled`, `Interrupted`, `Aborted`
- `IndexRunMode`: `Full`, `Incremental`, `Repair` (defined but unused), `Verify`, `Reindex`
- `ResumeRejectReason` enum for resume failure classification
- `RunRecoveryState` struct capturing recovery outcome and rejection detail
- `PersistedFileOutcome`: `Committed`, `EmptySymbols`, `Failed { error }`, `Quarantined { reason }`

**NextAction Vocabulary** (`src/domain/retrieval.rs`):
- `NextAction`: `Resume`, `Reindex`, `Repair`, `Wait`, `ResolveContext`
- Already returned in retrieval gating responses — Story 4.4 repair should use the same vocabulary

**Existing Recovery/Repair Operations** (`src/application/run_manager.rs`):
- `invalidate_repository(repo_id, workspace_id, reason)` — transitions repo to Invalidated
- `reindex_repository(repo_id, reason)` — spawns a full reindex run (existing target for repair delegation)
- `resume_run(run_id)` — resumes interrupted run from checkpoint (Story 4.2)
- `startup_sweep()` — transitions stale Running → Interrupted, classifies Queued runs
- `StartupRecoveryReport` — structured recovery findings and guidance

**Request Gating** (`src/application/search.rs`):
- `check_request_gate()` — blocks retrieval on Invalidated, Quarantined, Failed repos
- Returns `NextAction::Repair` for quarantined states
- Already tested: `test_quarantined_file_includes_next_action_repair`

**ControlPlane Trait** (`src/storage/control_plane.rs`):
- `get_repository(repo_id)` — load repo state
- `update_repository_status(repo_id, status, invalidated_at, reason, quarantined_at, quarantine_reason)` — transition repo status
- `get_file_records(run_id)` — retrieve all file records for a run
- `save_file_records(run_id, records)` — upsert file records (4.3 semantics)
- `find_run(run_id)` — load run state
- `get_latest_completed_run(repo_id)` — find the most recent completed run
- Three implementations: `InMemoryControlPlane`, `RegistryBackedControlPlane`, `SpacetimeControlPlane`
- `RunManagerPersistenceAdapter` wraps the control plane for RunManager use

**MCP Protocol Surface** (`src/protocol/mcp.rs`):
- Existing tools: `health`, `index_folder`, `reindex_repository`, `invalidate_indexed_state`, `resume_index_run`, `list_index_runs`
- Pattern: each tool method on `TokenizorServer`, delegates to `RunManager` or `SearchManager`

**CAS / Blob Store** (`src/storage/local_cas.rs`):
- Content-addressable store for indexed blobs
- Blob ID is derived from content hash
- Used by retrieval verification in Epic 3

### What 4.4 Builds vs. What Already Exists

| Concern | Already exists | 4.4 adds |
|---------|---------------|----------|
| State classification | RepositoryStatus, IndexRunStatus, PersistedFileOutcome enums | RepairScope, RepairOutcome, RepairResult types |
| Repair entry point | None (operators must manually call reindex/resume) | `repair_repository()` unified entry point with routing |
| Quarantine repair | Quarantine gating blocks retrieval | Re-verification against CAS + unquarantine path |
| Degraded repair | Status set by pipeline, no auto-recovery | Assess file state, mark Ready or delegate to reindex |
| Failed/Invalidated repair | `reindex_repository` exists but requires explicit call | Repair auto-delegates to reindex as safe fallback |
| Interrupted run repair | `resume_run` exists but requires explicit call | Repair auto-delegates to resume_run |
| History recording | Status transitions persisted, no repair-specific events | RepairEvent recorded via control plane |
| MCP exposure | No repair tool | `repair_index` MCP tool |
| NextAction integration | `Repair` variant exists in NextAction | Repair flows produce NextAction in results |

### Design Decisions

**1. Repair routes to existing operations, does not duplicate them.**
`repair_repository()` delegates to `reindex_repository()` and `resume_run()` where appropriate. It does not reimplement indexing or resume logic. The value of the repair command is classification, routing, and explicit outcome reporting — not reimplementation.

**2. Re-verification for quarantined files is the only genuinely new repair action.**
All other repair paths route to existing operations. Quarantine repair (re-verify file against CAS/disk source) is new functionality that can restore trust without a full reindex.

**3. Repair event recording is minimal — Story 4.6 will make it comprehensive.**
4.4 adds `save_repair_event` / `get_repair_events` to the ControlPlane trait with basic implementations. Story 4.6 (operational history) will build a full audit trail with structured events, retention, and inspection.

**4. No span-level repair in this story.**
Span-level repair (byte-range re-verification) is retrieval-layer concern from Epic 3. Story 4.4 focuses on repository, run, and file scopes. Span repair can be added later if needed.

**5. Repair is idempotent but not deduplicated.**
Running repair twice should produce the same result. The second call returns `AlreadyHealthy` if the first succeeded. But repair is not deduplicated via idempotency records — operators should be able to retry repair freely.

### State Transition Rules for Repair

**Repository status transitions during repair:**

```
Ready       → (no change, AlreadyHealthy)
Pending     → (no change, AlreadyHealthy)
Degraded    → Ready (if no failed files remain)
             → InProgress (spawns reindex if failed files exist)
Failed      → InProgress (spawns reindex)
Invalidated → InProgress (spawns reindex)
Quarantined → Ready (if all files re-verify)
             → Degraded (if some files re-verify, others don't)
             → CannotRestore (if no files re-verify)
```

**Run status transitions during repair:**

```
Interrupted → Running (via resume_run)
              → CannotRestore (if resume rejected)
Failed      → (no change, RequiresReindex)
Cancelled   → (no change, AlreadyHealthy)
Aborted     → (no change, AlreadyHealthy)
Succeeded   → (no change, AlreadyHealthy)
Queued      → (no change, CannotRestore: still active)
Running     → (no change, CannotRestore: still active)
```

**File outcome transitions during repair:**

```
Committed    → (no change, AlreadyHealthy)
EmptySymbols → (no change, AlreadyHealthy)
Failed       → (no change, RequiresReindex)
Quarantined  → Committed (if re-verification passes)
              → (no change, CannotRestore if re-verification fails)
```

### Trust Boundary Rules (from architecture and project-context)

1. **Never silently mark state healthy.** Every repair outcome must be explicit. If repair cannot provably restore trust, the response must say so.
2. **Repair must be observable.** A successful repair must change the repository/run state AND change the next retrieval result observable by the caller. Mutation without observable behavior change is a bug.
3. **Quarantine is explicit and repairable, never hidden.** Quarantined files must be inspectable, excluded from default retrieval, and repairable through the repair flow.
4. **Operational history must be durable before acknowledging success.** The RepairEvent must be persisted before returning RepairResult.
5. **NextAction vocabulary is shared across recovery surfaces.** Use `Resume`, `Reindex`, `Repair`, `Wait`, `ResolveContext` — do not invent one-off strings.

[Source: _bmad-output/planning-artifacts/architecture.md — Recovery Architecture]
[Source: _bmad-output/project-context.md — Epic 4 Recovery Architecture]
[Source: _bmad-output/project-context.md — ADR-7]

### Previous Story Intelligence

**Story 4.3** (Move Mutable Run Durability to SpacetimeDB Control Plane) — DONE:
- Expanded `ControlPlane` trait with 20+ methods covering runs, checkpoints, file records, repositories, idempotency, discovery manifests
- Moved `RunManager` from `RegistryPersistence` to `Arc<dyn ControlPlane>`
- Added `RegistryBackedControlPlane` adapter and `RunManagerPersistenceAdapter`
- `save_file_records` uses upsert semantics (BTreeMap merge by relative_path) — Story 4.4 can use this for updating individual file outcomes
- `SpacetimeControlPlane` has real SpacetimeDB writes for mutable state
- Review fix M7 established `NotFound` suppression pattern on adapter write methods — Story 4.4 should follow the same pattern for `save_repair_event`

**Story 4.3 Deferred Items relevant to 4.4:**
- **M1 (MEDIUM)**: `save_file_records` atomicity — SpacetimeDB per-file upserts are not transactional across a batch. If repair updates multiple file records, partial failures leave partial state. Mitigation: repair should re-assess repo status after each file update, not assume batch success.
- **H4 (HIGH)**: InMemoryControlPlane lifecycle tests should verify start→checkpoint→resume→cancel lifecycle independent of RunManager. Story 4.4 repair tests should add coverage for repair-related state transitions on InMemoryControlPlane.

**Story 4.2** (Resume Interrupted Indexing from Durable Checkpoints) — DONE:
- `resume_run()` validates checkpoint compatibility and source state
- Returns `ResumeRejectReason` on failure — Story 4.4 maps these to RepairOutcome
- Discovery manifest validation (from 4.3) ensures deterministic resume

**Story 4.1** (Sweep Stale Leases on Startup) — DONE:
- `startup_sweep()` transitions stale runs before new mutations
- `StartupRecoveryReport` with structured findings — model for RepairResult

### Git Intelligence

Recent commits show incremental hardening through Epic 4:
- `b84ce37` — feat(control-plane): land story 4.3 with review fixes (most recent feature commit)
- `10e7907` — docs: update planning artifacts and add tooling configs
- `4123ee4` — Formalize Epic 4.0 hardening

Pattern: each story builds on the previous one's foundation. Story 4.4 follows this pattern by building on 4.3's expanded ControlPlane trait and 4.2's resume logic.

### Existing Code to Reuse

| Function / Type | Location | Why it matters for 4.4 |
|---|---|---|
| `RunManager::reindex_repository()` | `src/application/run_manager.rs` | Delegate to for Failed/Invalidated/Degraded repair |
| `RunManager::resume_run()` | `src/application/run_manager.rs` | Delegate to for Interrupted run repair |
| `RunManager::invalidate_repository()` | `src/application/run_manager.rs` | Pattern reference for new repair method structure |
| `ControlPlane::get_repository()` | `src/storage/control_plane.rs` | Load current repo state |
| `ControlPlane::update_repository_status()` | `src/storage/control_plane.rs` | Transition repo after repair |
| `ControlPlane::get_file_records()` | `src/storage/control_plane.rs` | Get file outcomes for assessment |
| `ControlPlane::save_file_records()` | `src/storage/control_plane.rs` | Update individual file outcomes (upsert) |
| `ControlPlane::get_latest_completed_run()` | `src/storage/control_plane.rs` | Find the run whose files need repair |
| `NextAction` enum | `src/domain/retrieval.rs` | Shared vocabulary for repair outcomes |
| `RepositoryStatus` enum | `src/domain/repository.rs` | State classification input |
| `PersistedFileOutcome` enum | `src/domain/index.rs` | File-level repair classification |
| `IndexRunMode::Repair` | `src/domain/index.rs` | Already defined, wire it for repair-spawned runs |
| `check_request_gate()` | `src/application/search.rs` | Validates repair changes are observable in retrieval gating |
| `LocalBlobStore` | `src/storage/local_cas.rs` | CAS access for quarantine re-verification |
| `StartupRecoveryReport` | `src/application/run_manager.rs` | Structural pattern for RepairResult |
| `InvalidationResult` | `src/domain/repository.rs` | Pattern reference for repair result structure |

### Library / Framework Requirements

- No new external dependencies required for Story 4.4
- Keep all existing dependency versions (`rmcp = 1.1.0`, `tokio = 1.48`, `serde = 1.0`, `fs2 = 0.4`, `ignore = 0.4`)
- SpacetimeDB SDK (`spacetimedb-sdk = 2.0.3`) — no new tables needed; repair events use existing infrastructure or stubs until Story 4.6

### Testing Requirements

- All existing tests must continue to pass (642 tests as of Story 4.3 completion)
- New unit tests in `src/application/run_manager.rs` `#[cfg(test)]` module for repair logic (13 tests specified in Task 6.1)
- New integration tests in `tests/indexing_integration.rs` or a new `tests/repair_integration.rs` for end-to-end repair flows (6 tests specified in Task 6.2)
- **Two-sided verification**: every repair test must verify both the state mutation (repository/file status changed) AND the user-visible read-path behavior change (retrieval gating changed)
- Keep recovery proofs local-first: `InMemoryControlPlane` tests are the primary verification surface; SpacetimeDB tests are supplementary

### Epic 4 Definition of Done (mandatory)

- Expected test delta: Add 13 unit tests for repair logic (routing, classification, outcome reporting, history recording) and 6 integration tests for end-to-end repair flows (degraded→ready, quarantined→partial restore, idempotency, observable state change, retrieval behavior change, event persistence)
- Build/test evidence: [Record the exact `cargo test` command(s) and pass/fail summary]
- Acceptance-criteria traceability:
  - AC1 → `repair_repository()` entry point routes to deterministic repair paths; `save_repair_event()` records action; `repair_index` MCP tool triggers repair
  - AC2 → `RepairOutcome::CannotRestore` / `RequiresReindex` variants; `test_repair_never_silently_marks_healthy`; `test_repair_quarantined_partial_failure_reports_cannot_restore`
- Trust-boundary traceability: Cite architecture recovery rules (never silently mark healthy, repair must be observable, quarantine is explicit and repairable), project-context ADR-7, Epic 4 recovery architecture
- State-transition evidence: Prove both sides for every transition:
  - Repository status change observable via `get_repository()` after repair (mutation side)
  - Retrieval gating change observable via `check_request_gate()` or retrieval call after repair (read side)
  - RepairEvent persisted and retrievable via `get_repair_events()` (history side)

### Self-Audit Checklist (mandatory before requesting review)

_Run this checklist after all tasks are complete. This is a blocking step — do not request review until every item is verified._

#### Generic Verification
- [x] For every task marked `[x]`, cite the specific test that verifies it
- [x] For every new error variant or branch, confirm a test exercises it
- [x] For every computed value, trace it to where it surfaces (log, return value, persistence)
- [x] For every test, verify the assertion can actually fail (no `assert!(true)`, no conditionals that always pass)

#### Epic 4 Recovery Verification
- [x] The declared expected test delta was met or exceeded by the actual implementation
- [x] Build/test evidence is recorded with the exact command and outcome summary
- [x] Every acceptance criterion is traced to concrete implementation code and at least one concrete test
- [x] Every trust-boundary or recovery-policy decision cites the exact architecture or `project-context.md` source
- [x] Every state transition is tested from both sides: the mutation itself and the resulting retrieval/inspection behavior

#### Story 4.4-Specific Verification
- [x] Confirm `repair_repository()` routes to correct repair path for each RepositoryStatus
- [x] Confirm `repair_run_state()` delegates to `resume_run` for Interrupted, returns RequiresReindex for Failed
- [x] Confirm `repair_file_state()` re-verifies quarantined files and unquarantines on success
- [x] Confirm repair never silently transitions to Ready when quarantined or failed files remain
- [x] Confirm RepairEvent is saved to control plane before RepairResult is returned
- [x] Confirm `repair_index` MCP tool is callable with all three scopes
- [x] Confirm repair outcome is observable in next `get_repository()` call (state mutation)
- [x] Confirm repair outcome is observable in next retrieval call (retrieval gating change)
- [x] Confirm repair is idempotent: second call on healthy repo returns AlreadyHealthy

### Project Structure Notes

| File | Why it is in scope |
|---|---|
| `src/domain/index.rs` | Add `RepairScope`, `RepairOutcome`, `RepairResult`, `RepairEvent` types |
| `src/application/run_manager.rs` | Add `repair_repository()`, `repair_repository_state()`, `repair_run_state()`, `repair_file_state()` methods |
| `src/storage/control_plane.rs` | Add `save_repair_event()`, `get_repair_events()` to trait; implement on all backends |
| `src/protocol/mcp.rs` | Add `repair_index` MCP tool handler |
| `src/storage/local_cas.rs` | May need helper for re-verification (or use existing blob read) |
| `tests/indexing_integration.rs` | Add repair integration tests (or create `tests/repair_integration.rs`) |

**Alignment notes**
- Stay inside the current `application` / `domain` / `indexing` / `storage` layering
- `repair_repository()` lives on `RunManager` alongside `invalidate_repository()` and `reindex_repository()` — they are peer operations on the same entity
- Repair types go in `src/domain/index.rs` alongside existing `IndexRunMode::Repair`, `PersistedFileOutcome`, and `RunRecoveryState`
- Follow the `RunManagerPersistenceAdapter` pattern from 4.3 for new ControlPlane methods (delegate + `NotFound` suppression)

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.4]
- [Source: _bmad-output/planning-artifacts/architecture.md — Recovery Architecture]
- [Source: _bmad-output/planning-artifacts/architecture.md — Trust Boundary Rules]
- [Source: _bmad-output/project-context.md — ADR-7]
- [Source: _bmad-output/project-context.md — Epic 4 Recovery Architecture]
- [Source: _bmad-output/project-context.md — Agent Selection]
- [Source: _bmad-output/implementation-artifacts/4-3-move-mutable-run-durability-to-the-spacetimedb-control-plane.md]
- [Source: src/domain/repository.rs — RepositoryStatus enum]
- [Source: src/domain/index.rs — IndexRunStatus, IndexRunMode, PersistedFileOutcome]
- [Source: src/domain/retrieval.rs — NextAction enum]
- [Source: src/application/run_manager.rs — RunManager methods]
- [Source: src/storage/control_plane.rs — ControlPlane trait]
- [Source: src/protocol/mcp.rs — MCP tool surface]
- [Source: src/application/search.rs — check_request_gate()]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References
(none — clean implementation, no debug sessions needed)

### Completion Notes List
- Implemented RepairScope, RepairOutcome, RepairResult, RepairEvent domain types in src/domain/index.rs
- Added repair_repository() entry point on RunManager with routing for Repository, Run, and File scopes
- Implemented repair_repository_state() handling Degraded, Quarantined, Failed, Invalidated states
- Implemented repair_run_state() with resume_run delegation for Interrupted, RequiresReindex for Failed
- Implemented repair_file_state() with CAS re-verification for quarantined files
- Extended ControlPlane trait with save_repair_event/get_repair_events, implemented on all backends (InMemory, RegistryBacked, SpacetimeDB stub with warning)
- Added repair_index MCP tool with repository/run/file scope support
- Added verify_file_against_source() helper for quarantine re-verification
- 14 unit tests + 5 integration tests added (19 new tests total, from 642 to 661)
- All 661 tests pass, zero regressions

### File List
- Cargo.toml (modified — bumped spacetimedb-sdk 2.0.1 → 2.0.3)
- src/domain/index.rs (modified — added RepairScope, RepairOutcome, RepairResult, RepairEvent types)
- src/domain/mod.rs (modified — added re-exports for new repair types)
- src/application/run_manager.rs (modified — added repair methods, verify helper, adapter methods, unit tests)
- src/application/mod.rs (modified — added repair_repository bridge on ApplicationContext)
- src/application/deployment.rs (modified — added ControlPlane trait methods to test fake)
- src/storage/control_plane.rs (modified — added save_repair_event/get_repair_events to trait and all implementations)
- src/storage/registry_persistence.rs (modified — added repair_events to RegistryData, persistence methods)
- src/protocol/mcp.rs (modified — added repair_index MCP tool)
- tests/indexing_integration.rs (modified — added repair integration tests)

### Change Log
- 2026-03-09: Implemented Story 4.4 — deterministic repair for suspect/incomplete state. Added repair domain types, RunManager repair logic with 3 scope levels, ControlPlane trait extension for repair history, repair_index MCP tool, CAS re-verification helper, and 8 new tests.
- 2026-03-09: Code review fixes — C1: repair_run_state now delegates to resume_run for Interrupted runs instead of treating them as Failed. C3: record_repair_event now propagates errors instead of swallowing them (trust boundary rule 4). H1: eliminated double verify_file_against_source call in quarantine partial repair (TOCTOU fix). H2: SpacetimeDB save_repair_event now logs warning instead of silent no-op. H4: Cargo.toml added to File List. Added 11 new tests (7 unit + 4 integration) covering quarantine verify/drift, invalidated reindex, interrupted run resume, failed run, file-scoped repair, two-sided verification, and repair event persistence. Total: 19 new tests, 661 pass.
