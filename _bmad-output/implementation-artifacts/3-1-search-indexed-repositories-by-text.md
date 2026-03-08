# Story 3.1: Search Indexed Repositories by Text

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an AI coding user,
I want to search indexed repositories by text,
so that I can find relevant code locations without brute-force file rereads.

**FRs implemented:** FR17, FR23

## Acceptance Criteria

1. **Given** a repository has indexed searchable content **When** I perform a text search **Then** Tokenizor returns matching results scoped to the correct repository or workspace context **And** the response is fast enough to support normal coding workflow use (AC: 1)
2. **Given** no matches exist **When** I perform a text search **Then** Tokenizor returns an explicit empty result **And** it does not imply stale or hidden matches (AC: 2)
3. **Given** a repository is invalidated, failed, or has an active mutation in progress **When** I perform a text search **Then** the entire request fails with an explicit status-based rejection before any search executes (AC: 3, request-fatal gate). Note: `Degraded` is NOT request-fatal for text search — results are returned with provenance metadata so callers can assess.
4. **Given** a repository has no completed index runs **When** I perform a text search **Then** the request fails with an explicit "not indexed" / missing state, not a generic empty result (AC: 4, disambiguation)
5. **Given** a repository has quarantined files **When** text search matches content in quarantined files **Then** those files are excluded unconditionally from search results (AC: 5, quarantine exclusion)
6. **Given** a search returns results **When** the response is constructed **Then** every result carries `run_id` and `committed_at_unix_ms` provenance from the originating `FileRecord` (AC: 6, provenance)

## Tasks / Subtasks

### Phase 0: Shared Read-Side Contract Types (prerequisite for all Phase 1 stories)

- [x] Task 0.1: Define result envelope types (AC: 1, 2, 3, 4, 5, 6)
  - [x] 0.1.1: Create `src/domain/retrieval.rs` with shared contract types
  - [x] 0.1.2: Define `RetrievalOutcome` enum: `Success`, `Empty`, `NotIndexed`, `Stale`, `Quarantined`, `Blocked`
  - [x] 0.1.3: Define `TrustLevel` enum: `Verified`, `Unverified`, `Suspect`, `Quarantined`
  - [x] 0.1.4: Define `Provenance` struct: `run_id: String`, `committed_at_unix_ms: u64`, `repo_id: String`
  - [x] 0.1.5: Define `ResultEnvelope<T>` with `outcome`, `trust`, `provenance`, `data: Option<T>`
  - [x] 0.1.6: Define `RequestGateError` enum for request-fatal conditions
  - [x] 0.1.7: Define `SearchResultItem` struct for text search results (see schema below)
  - [x] 0.1.8: Register module in `src/domain/mod.rs`

- [x] Task 0.2: Implement request-level gate function (AC: 3, 4)
  - [x] 0.2.1: Create gate function that checks all request-fatal conditions (see rules below)
  - [x] 0.2.2: Gate consults `RegistryPersistence::get_repository()` for repo status + invalidation
  - [x] 0.2.3: Gate checks for active mutations via `RunManager::has_active_run()` (in-memory HashMap, no lease mechanism)
  - [x] 0.2.4: Gate checks index completeness via `get_latest_completed_run()` — `None` is request-fatal, but see disambiguation note below
  - [x] 0.2.5: Gate returns `Ok(())` for `Degraded` repos (soft warning, not fatal for text search)
  - [x] 0.2.6: Add `TokenizorError::RequestGated { gate_error: String }` variant to `src/error.rs`. Classify `is_systemic() = false` (client/context error, not systemic). Map in `to_mcp_error()` to `invalid_params` with gate error details. Gate function returns this variant on failure.
  - [x] 0.2.7: Unit tests for every request-fatal condition AND a test confirming Degraded passes the gate

### Phase 1: Text Search Implementation

- [x] Task 1.1: Implement text search query logic (AC: 1, 2, 5, 6)
  - [x] 1.1.1: Create `src/application/search.rs` (existing `application/` uses flat files — no `queries/` subfolder exists)
  - [x] 1.1.2: Implement `search_text(repo_id, query, options) -> Result<ResultEnvelope<Vec<SearchResultItem>>>`
  - [x] 1.1.3: Call request gate first — if gate fails, return request-fatal error immediately
  - [x] 1.1.4: Get latest completed run for repo via `get_latest_completed_run()`
  - [x] 1.1.5: Get file records for that run via `get_file_records(run_id)`
  - [x] 1.1.6: Filter out quarantined files (`PersistedFileOutcome::Quarantined`)
  - [x] 1.1.7: For each non-quarantined file, read blob via `BlobStore::read_bytes(blob_id)`, then re-hash bytes with SHA-256 and compare to `blob_id` from `FileRecord`. Mismatch = skip file (item-local integrity error, do not fail entire search). Log `warn!` with blob_id and file path. Only search verified bytes.
  - [x] 1.1.8: Search verified blob bytes for query matches
  - [x] 1.1.9: Build `SearchResultItem` for each match with provenance
  - [x] 1.1.10: Return `RetrievalOutcome::Empty` (not generic empty vec) when no matches found in healthy index
  - [x] 1.1.11: Return `RetrievalOutcome::NotIndexed` when no completed runs exist (defense-in-depth — gate should catch this first, but guards against direct calls bypassing the gate)

- [x] Task 1.2: Wire search into application layer (AC: 1)
  - [x] 1.2.1: Add `search_text` method to `RunManager` or create dedicated query service
  - [x] 1.2.2: Ensure method receives `Arc<RegistryPersistence>` and `Arc<dyn BlobStore>`

- [x] Task 1.3: Unit tests for search logic (AC: 1, 2, 3, 4, 5, 6)
  - [x] 1.3.1: `test_search_text_returns_matching_results` — happy path with matches
  - [x] 1.3.2: `test_search_text_returns_empty_for_no_matches` — explicit empty, not generic
  - [x] 1.3.3: `test_search_text_rejects_invalidated_repo` — request-fatal gate
  - [x] 1.3.4: `test_search_text_allows_degraded_repo_with_provenance` — NOT request-fatal, returns results
  - [x] 1.3.5: `test_search_text_rejects_failed_repo` — request-fatal gate
  - [x] 1.3.6: `test_search_text_rejects_active_mutation` — request-fatal gate (via RunManager::has_active_run)
  - [x] 1.3.7: `test_search_text_rejects_never_indexed_repo` — no runs exist at all → NeverIndexed
  - [x] 1.3.8: `test_search_text_rejects_no_successful_runs` — runs exist but all Failed/Interrupted → NoSuccessfulRuns with latest_status
  - [x] 1.3.9: `test_search_text_excludes_quarantined_files` — quarantine exclusion
  - [x] 1.3.10: `test_search_text_includes_provenance_metadata` — run_id + committed_at_unix_ms
  - [x] 1.3.11: `test_search_text_skips_file_with_corrupted_blob` — blob bytes don't match blob_id hash, file excluded from results, other files still returned
  - [x] 1.3.12: `test_search_text_scopes_to_repo_context` — no cross-repo leakage
  - [x] 1.3.13: `test_search_text_latency_within_bounds` — sanity check (not full benchmark)

- [x] Task 1.4: Integration tests (AC: 1, 2, 3, 5)
  - [x] 1.4.1: Add search integration tests to `tests/indexing_integration.rs` (or new `tests/retrieval_integration.rs`)
  - [x] 1.4.2: End-to-end: index a fixture repo, then search, verify results
  - [x] 1.4.3: End-to-end: search with quarantined files, verify exclusion
  - [x] 1.4.4: End-to-end: search against invalidated repo, verify rejection
  - [x] 1.4.5: End-to-end: search with no completed runs, verify explicit never-indexed rejection (not a generic empty result)

- [x] Task 1.5: Contract-conformance test skeleton (Epic 3 mandatory gate)
  - [x] 1.5.1: Create `tests/retrieval_conformance.rs` — tests that `ResultEnvelope`, `RetrievalOutcome`, `TrustLevel`, `Provenance`, and `RequestGateError` types satisfy the shared contract (constructable, serializable, exhaustive match)
  - [x] 1.5.2: Add conformance test: gate error types cover all request-fatal conditions listed in contract
  - [x] 1.5.3: Add conformance test: `SearchResultItem` includes all required provenance fields
  - [x] 1.5.4: This skeleton is extended by each subsequent Epic 3 story — story is not `done` until conformance tests pass

## Dev Notes

### Contract Requirements (Phase 0 — Shared Read-Side Contract)

**This is the most critical prerequisite.** The contract is specified before Story 3.1 is considered started. It is reviewed against the requirements of 3.1, 3.2, 3.3, 3.5, 3.6, and 3.7 before any story begins. First implementation happens during 3.1, but the contract is NOT shaped by 3.1 alone.

The contract must include:
- **Result envelope**: `outcome` + `trust` + `provenance` on every item/result
- **Request-level gating**: Health/context gate checks repo validity before any item processing
- **Result-state disambiguation**: explicit empty vs missing vs stale vs quarantined
- **Quarantined-file exclusion rules**: search excludes unconditionally; targeted retrieval may return with `trust: quarantined`
- **Active-context resolution**: scoped to correct repo/workspace

**ADR-1**: Contract specified before 3.1 starts; reviewed against all stories — prevents contract from being shaped by a single consumer.
**ADR-2**: Split `outcome` + `trust` fields; request-level gating before item processing — orthogonal concerns; unhealthy repos fail early, not silently degraded.

### Request-Fatal vs Item-Local Rules

**Request-fatal (entire request fails before any search executes):**
- No active context / unknown `repo_id` → `get_repository()` returns `None`
- `RepositoryStatus::Invalidated` — trust explicitly revoked
- `RepositoryStatus::Failed` — index failed, no reliable data guarantee
- Active mutation in progress — detected via `RunManager::has_active_run(repo_id)` (in-memory HashMap; no lease mechanism exists). Alternatively check persistence for `Running`/`Queued` runs via `get_runs_by_repo()`, but note: in-memory check is authoritative for the current process; persistence may show `Running` for a crashed-but-not-yet-swept run
- No completed (`Succeeded`) runs exist — `get_latest_completed_run()` returns `None`. This covers `RepositoryStatus::Pending` and repos where all runs failed/were interrupted

**`get_latest_completed_run()` returns `None` — disambiguation required:**
`None` covers two distinct states that MUST NOT collapse into a generic empty-search outcome:
1. **Never indexed**: `get_runs_by_repo()` returns empty — no runs of any kind exist. Gate error should say "repository has not been indexed."
2. **Indexed but no success**: `get_runs_by_repo()` returns runs, but all are `Failed`/`Interrupted`/`Cancelled`/`Aborted`. Gate error should say "no successful index exists" and include the latest run's status so the caller knows whether to re-index or wait.
The gate must call `get_runs_by_repo()` when `get_latest_completed_run()` returns `None` to distinguish these. A single `NoCompletedRuns` variant is insufficient — split into `NeverIndexed` and `NoSuccessfulRuns { latest_status: IndexRunStatus }`, or carry the detail as a field. This directly satisfies the "empty vs missing vs stale" disambiguation requirement.

**NOT request-fatal for text search (soft warning via provenance/trust metadata):**
- `RepositoryStatus::Degraded` — some files had issues but completed index data is still readable. Return results with provenance so caller can assess staleness. (Narrative gating thresholds list Degraded as soft warning, not hard block. project-context.md Rule 2 lists it as reject — resolve in favor of the narrative's more specific Epic 3 guidance, but flag the tension in a code comment for future stories to revisit.)
- Aged-but-complete index — old `committed_at_unix_ms` is a staleness signal, not a trust-breaking state. Freshness policy belongs to Epic 4/5.
- Partial symbol coverage — irrelevant to text search (no symbol data used)

**Item-local (per-item within a passing request):**
- Individual file quarantined (`PersistedFileOutcome::Quarantined`) — excluded from search results silently
- Blob read failure for a specific file — skip file, do not fail entire search

**Rule**: Request gating runs FIRST. Item-level outcomes apply ONLY after the gate passes. For text search specifically, quarantined files are silently excluded (not reported as item-local failures) because search is a discovery operation, not a targeted retrieval.

### Search Result Schema

```rust
/// A single text search match
pub struct SearchResultItem {
    pub relative_path: String,       // File path relative to repo root
    pub language: LanguageId,        // Language of the matched file
    pub line_number: u32,            // 1-based line number of match
    pub line_content: String,        // The matched line (trimmed)
    pub match_offset: u32,           // Byte offset of match within line
    pub match_length: u32,           // Length of matched text in bytes
    pub provenance: Provenance,      // run_id + committed_at_unix_ms
}

/// Provenance metadata for trust/staleness evaluation
pub struct Provenance {
    pub run_id: String,
    pub committed_at_unix_ms: u64,
    pub repo_id: String,
}

/// Outcome of a retrieval or search operation
pub enum RetrievalOutcome {
    Success,                         // Results found and returned
    Empty,                           // Searched, nothing matched (healthy index)
    NotIndexed,                      // Target not indexed (no completed runs)
    Stale,                           // Repository invalidated or unhealthy
    Quarantined,                     // Target quarantined (for targeted retrieval)
    Blocked { reason: String },      // Blocked for other integrity reasons
}

/// Trust level of returned data
pub enum TrustLevel {
    Verified,                        // Verified against indexed-state integrity
    Unverified,                      // Not yet verified (not applicable for search)
    Suspect,                         // Integrity concern detected
    Quarantined,                     // Quarantined by system
}

/// Full result envelope wrapping any retrieval/search response
pub struct ResultEnvelope<T> {
    pub outcome: RetrievalOutcome,
    pub trust: TrustLevel,
    pub provenance: Option<Provenance>,  // Aggregate provenance (from latest run)
    pub data: Option<T>,                 // None when outcome != Success
}

/// Request-fatal gate errors (text search)
pub enum RequestGateError {
    NoActiveContext,                   // repo_id not found in registry
    RepositoryInvalidated { reason: Option<String> },  // RepositoryStatus::Invalidated
    RepositoryFailed,                  // RepositoryStatus::Failed
    ActiveMutation { run_id: String }, // RunManager::has_active_run() == true
    NeverIndexed,                      // get_runs_by_repo() is empty — no runs exist at all
    NoSuccessfulRuns {                 // runs exist but none Succeeded
        latest_status: IndexRunStatus, // e.g. Failed, Interrupted — tells caller what happened
    },
}
// Note: Degraded is NOT request-fatal for text search — return results with provenance.
// Note: No repo-level quarantine status exists; quarantine is file-level only
//       (PersistedFileOutcome::Quarantined).
```

**Note on `TrustLevel` for search**: Text search results from a healthy, completed index carry `TrustLevel::Verified` because they come from verified indexed state. `trust: verified` means verified against indexed-state integrity only — NOT against live workspace state. Freshness is conveyed through provenance metadata (index timestamp, run_id). Policy decisions based on freshness belong to Epic 4/5.

### Output Schema Constraints

- Every search result MUST include `provenance` (run_id + committed_at_unix_ms). Omitting provenance violates the trust contract. [Source: project-context.md#Epic-3-Retrieval-Architecture, Rule 3]
- "No results" MUST disambiguate three states: Empty (searched, nothing matched) vs NotIndexed (target not indexed) vs Stale (repository invalidated or unhealthy). A generic empty response violates Epic 3 ACs. [Source: project-context.md#Epic-3-Retrieval-Architecture, Rule 4]
- Quarantined files MUST be excluded from search results unconditionally. They exist in registry for audit/repair only. [Source: project-context.md#Epic-3-Retrieval-Architecture, Rule 6]
- Trust/integrity state vocabulary MUST be centralized and reused across layers. [Source: architecture.md#Result-Format-Rules]

### Latency Requirements

- **`search_text`**: p50 <= 150 ms, p95 <= 500 ms on representative medium-to-large repositories (warm local index) [Source: epics.md#NFR1]
- **"Done" criteria**: A basic latency sanity check against the NFR target is REQUIRED. Not the full benchmark suite, but a test-fixture assertion that the operation completes within a reasonable bound. Performance is not deferred to a later pass. [Source: epics.md#Epic-3-Execution-Narrative]

### Testing Requirements

- **Naming**: `test_verb_condition` (e.g., `test_search_text_returns_matching_results`)
- **Fakes**: Hand-written fakes inside `#[cfg(test)] mod tests`. No mock crates.
- **Assertions**: Plain `assert!`, `assert_eq!`. No assertion crates.
- **Test type**: `#[test]` by default. `#[tokio::test]` only for async fn tests.
- **Unit tests**: `#[cfg(test)]` blocks inside modules.
- **Integration tests**: `tests/` at crate root.
- **Call verification**: `AtomicUsize` counters on fakes to verify interaction counts.
- **Fixture**: Use `tests/fixtures/epic2-registry.json` as baseline fixture. Extend or create `tests/fixtures/epic3-search-fixture.json` if needed.
- **Setup**: Follow existing `setup_test_env()` pattern returning `(TempDir, Arc<RunManager>, TempDir, Arc<dyn BlobStore>)`.
- **Contract conformance**: Story must pass the evolving contract-conformance test skeleton before moving to `done`.

### Epic 3 Retrieval Architecture Rules (Mandatory)

From project-context.md — these are non-negotiable for ALL Epic 3 code:

1. **Never return blob content without verifying blob_id matches.** Every retrieval path must re-verify that the blob_id stored in the FileRecord matches the content-addressed hash of the bytes returned from CAS. Mismatch = integrity error, never stale bytes.
2. **Every retrieval function must check repository status before returning trusted content.** If `RepositoryStatus` is `Invalidated` or `Failed`, retrieval must refuse the request with an explicit status-based rejection. **Note for 3.1**: project-context.md also lists `Degraded` as reject, but the Epic 3 execution narrative's gating thresholds classify it as soft warning. For text search, `Degraded` passes the gate with provenance metadata — add a code comment documenting this decision for future stories to revisit.
3. **Search results must include provenance metadata.** Every result must carry `run_id` and `committed_at_unix_ms` from the originating `FileRecord`.
4. **"No results" responses must disambiguate three states.** Empty vs missing vs stale.
5. **Retrieval paths must reject requests against invalidated or unhealthy repositories.** Gate check happens early — before any CAS reads or search queries execute.
6. **Quarantined file records must never appear in search results.**

### Scope Boundaries — What Story 3.1 Does NOT Cover

- MCP tool exposure (Story 3.4)
- Symbol search (Story 3.2)
- File/repo outlines (Story 3.3)
- Verified source retrieval with byte-exact verification (Story 3.5)
- Blocking/quarantine behavior for targeted retrieval (Story 3.6)
- Batched retrieval (Story 3.7)
- Live filesystem change detection or watch-based invalidation
- Write-side operations (re-indexing, repair, state mutation)
- Freshness policy enforcement (deferred to Epic 4/5)
- Cross-repository or cross-workspace search

### Existing Read-Side Interfaces from Epic 2 (Verified)

All 5 interfaces exist. Signatures and quirks confirmed against source:

| Interface | Actual Signature | Quirks |
|-----------|-----------------|--------|
| Blob lookup | `BlobStore::read_bytes(&self, blob_id: &str) -> Result<Vec<u8>>` (trait in `src/storage/blob.rs:20`, impl `LocalCasBlobStore` in `src/storage/local_cas.rs:239`) | Validates blob_id is exactly 64 hex chars → `InvalidArgument`. Missing blob → IO error (not `NotFound`). |
| File records by run | `RegistryPersistence::get_file_records(&self, run_id: &str) -> Result<Vec<FileRecord>>` (`src/storage/registry_persistence.rs:271`) | Returns empty `Vec` (not error) for unknown run_id — uses `.unwrap_or_default()`. |
| Latest completed run | `RegistryPersistence::get_latest_completed_run(&self, repo_id: &str) -> Result<Option<IndexRun>>` (`src/storage/registry_persistence.rs:220`) | Filters ONLY by `IndexRunStatus::Succeeded`. Picks latest by `requested_at_unix_ms` (not `finished_at`). Returns `None` if no Succeeded runs exist. |
| Active mutation check | `RunManager::has_active_run(&self, repo_id: &str) -> bool` (`src/application/run_manager.rs:121`) | In-memory HashMap check — no lease/persistence. Lost on crash; `startup_sweep()` transitions orphaned `Running` → `Interrupted`. For pure read paths, also consider `get_runs_by_repo()` + filter for `Running`/`Queued` status as a persistence-level fallback. |
| Repo status + invalidation | `RegistryPersistence::get_repository(&self, repo_id: &str) -> Result<Option<Repository>>` (`src/storage/registry_persistence.rs:172`) | Returns `None` for unknown repo_id. `Repository.status: RepositoryStatus` has variants: `Pending`, `Ready`, `Degraded`, `Failed`, `Invalidated`. Invalidation fields: `invalidated_at_unix_ms: Option<u64>`, `invalidation_reason: Option<String>`. |

### Implementation Note: `ActiveMutation` run_id Resolution

`RequestGateError::ActiveMutation { run_id }` requires the actual active run ID, but `RunManager::has_active_run()` returns `bool` only. Before writing gate code, resolve this by either:
1. Adding a method like `get_active_run_id(&self, repo_id: &str) -> Option<String>` to `RunManager` (reads from the existing `active_runs` HashMap)
2. Narrowing the error payload to drop `run_id` (e.g., `ActiveMutation` with no fields)

Do NOT fake/hardcode the run_id. Decide before Task 0.2 begins.

### Build Order (Mandatory — domain first, wiring last)

1. Domain types (`src/domain/retrieval.rs`) — contract types, enums, structs
2. Request gate function — pure function over domain types
3. Search query logic — pure function composing gate + registry + blob reads
4. Application wiring — integrate into `RunManager` or query service
5. Unit tests — one per AC, covering every gate condition
6. Integration tests — end-to-end with fixture repos

### Architecture Compliance

- **Layer**: Search logic belongs in `application/` layer (query). Domain types in `domain/`. MCP exposure deferred to Story 3.4 (`protocol/`).
- **Persistence model**: All reads via `RegistryPersistence`. Do NOT wire SpacetimeDB reads.
- **Error handling**: Use `TokenizorError` variants. Add `TokenizorError::RequestGated { gate_error: String }` (Task 0.2.6). `is_systemic() = false`. Map in `to_mcp_error()` to `invalid_params`.
- **No Mutex across .await**: Extract data, drop guard, then call async methods.
- **No mock crates**: Hand-written fakes with `AtomicUsize` counters.
- **No assertion crates**: Plain `assert!`/`assert_eq!`.

### Previous Story Intelligence (Epic 2.11 + Retrospective)

**Recurring dev agent failure modes to guard against:**
1. **No-op/sentinel tests**: `assert!(true)` or conditional logic that silently passes. Every test assertion MUST be able to fail.
2. **`is_systemic()` misclassification**: If adding new `TokenizorError` variants, classify correctly.
3. **Data computed but dropped**: Logic built correctly but output not wired to caller.
4. **Missing tests for new code paths**: Happy path tested, failure path skipped. Every gate condition needs a test.

**Patterns established in Epic 2:**
- Atomic write-to-temp-then-rename + fs2 advisory locking for persistence
- `setup_test_env()` returns `(TempDir, Arc<RunManager>, TempDir, Arc<dyn BlobStore>)`
- Integration tests in `tests/indexing_integration.rs` (consider new `tests/retrieval_integration.rs` for Epic 3)
- Current test count: 369 (64 lib unit + 299 integration + 6 grammar). Expect ~15-25 new tests.

### Self-Audit Checklist (mandatory before requesting review)

_Run this checklist after all tasks are complete. This is a blocking step — do not request review until every item is verified._

#### Generic Verification
- [ ] For every task marked `[x]`, cite the specific test that verifies it
- [ ] For every new error variant or branch, confirm a test exercises it
- [ ] For every computed value, trace it to where it surfaces (log, return value, persistence)
- [ ] For every test, verify the assertion can actually fail (no `assert!(true)`, no conditionals that always pass)

#### Epic 3-Specific Trust Verification
- [ ] For every search result, confirm provenance metadata (run_id, committed_at_unix_ms) is populated
- [ ] For every request gate condition, confirm a test exercises the rejection path
- [ ] For every "no results" path, confirm the response distinguishes empty vs missing vs stale
- [ ] For quarantined files, confirm they are excluded from search results (not returned with degraded trust)
- [ ] For the request gate, confirm it runs BEFORE any blob reads or search queries
- [ ] For every blob read, confirm blob_id hash verification is performed before content is used (Rule 1)
- [ ] Confirm the latency sanity check test exists and asserts a reasonable bound
- [ ] Confirm contract-conformance test skeleton exists and passes

### Project Structure Notes

- New file: `src/domain/retrieval.rs` — shared contract types for all Epic 3 stories
- New file: `src/application/search.rs` — text search query logic (flat file, matches existing `application/` pattern)
- Extended: `src/domain/mod.rs` — register `retrieval` module
- Extended: `src/error.rs` — add `RequestGated` variant, `is_systemic()` arm, `to_mcp_error()` mapping
- Extended: `src/application/mod.rs` — register `search` module
- Extended: `tests/indexing_integration.rs` or new `tests/retrieval_integration.rs`
- New file: `tests/retrieval_conformance.rs` — contract-conformance test skeleton (grows with each Epic 3 story)
- Extended: `src/protocol/mcp.rs` — `to_mcp_error()` mapping for `RequestGated` only (MCP tool exposure is Story 3.4)
- No new files in `src/storage/` — reads use existing `RegistryPersistence` + `BlobStore`

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story-3.1] — User story, ACs, BDD scenarios
- [Source: _bmad-output/planning-artifacts/epics.md#Epic-3-Execution-Narrative] — Phase model, ADRs, failure modes, gating rules
- [Source: _bmad-output/project-context.md#Epic-3-Retrieval-Architecture] — 6 mandatory retrieval rules
- [Source: _bmad-output/planning-artifacts/architecture.md#Retrieval-Trust-Model] — Trust verification model
- [Source: _bmad-output/planning-artifacts/architecture.md#Result-Format-Rules] — Result format constraints
- [Source: _bmad-output/planning-artifacts/epics.md#NFR1] — search_text latency: p50<=150ms, p95<=500ms
- [Source: _bmad-output/implementation-artifacts/epic-2-retro-2026-03-08.md] — Blind spot analysis, recurring failures
- [Source: _bmad-output/implementation-artifacts/2-11-reject-conflicting-idempotent-replays.md] — Latest patterns, tech stack
- [Source: docs/data-models.md] — FileRecord, SymbolRecord, RepositoryStatus definitions
- [Source: docs/api-contracts.md] — Planned search_text interface

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

None — clean implementation with no blocking issues.

### Completion Notes List

- Implemented shared read-side contract types in `src/domain/retrieval.rs`: `RetrievalOutcome`, `TrustLevel`, `Provenance`, `ResultEnvelope<T>`, `RequestGateError`, `SearchResultItem` with `Display` impl for gate errors
- Implemented request-level gate function `check_request_gate()` in `src/application/search.rs` covering all 6 request-fatal conditions with NeverIndexed/NoSuccessfulRuns disambiguation
- Added `run_id: String` field to `ActiveRun` and `get_active_run_id()` method to `RunManager` to support `ActiveMutation { run_id }` gate error
- Added `TokenizorError::RequestGated` variant with `is_systemic() = false` and `to_mcp_error()` → `invalid_params` mapping
- Implemented `search_text()` with: gate check → latest run lookup → file record iteration → quarantine exclusion → blob integrity verification (SHA-256 re-hash) → line-by-line text matching → provenance-enriched results
- Degraded repos pass the gate with a code comment documenting the decision tension between project-context.md Rule 2 and Epic 3 narrative
- Wired `search_text` through `ApplicationContext` for application-layer access
- 24 unit tests covering all gate conditions, search scenarios, and regression fixes for review findings
- 4 integration tests in `tests/retrieval_integration.rs` with real indexing pipeline
- 8 conformance tests in `tests/retrieval_conformance.rs` covering type exhaustiveness, serializability, and provenance field presence
- Total test count: 405 (up from 369 baseline). No regressions.

### Implementation Plan

**Build order followed**: Domain types → Gate function → Search logic → Application wiring → Unit tests → Integration tests → Conformance tests

**Key decisions**:
- Added `run_id` to `ActiveRun` struct rather than dropping it from `ActiveMutation` — provides more useful diagnostic information
- Gate function returns `TokenizorError::RequestGated` directly rather than an intermediate error type
- Search uses line-by-line matching with `str::find()` for simplicity; advanced regex/fuzzy matching deferred
- Non-UTF-8 blob content is skipped with a warning (text search requires string content)

### File List

New files:
- `src/domain/retrieval.rs` — shared contract types for Epic 3 retrieval
- `src/application/search.rs` — request gate + text search implementation
- `tests/retrieval_integration.rs` — integration tests for search
- `tests/retrieval_conformance.rs` — contract conformance test skeleton

Modified files:
- `src/domain/mod.rs` — registered `retrieval` module, added re-exports
- `src/application/mod.rs` — registered `search` module, added `search_text` to `ApplicationContext`
- `src/application/run_manager.rs` — added `run_id` to `ActiveRun`, added `get_active_run_id()` method
- `src/error.rs` — added `RequestGated` variant with `is_systemic()` and `Display`
- `src/protocol/mcp.rs` — added `RequestGated` arm to `to_mcp_error()`

## Senior Developer Review (AI)

### Reviewer

Codex (GPT-5) — 2026-03-08

### Outcome

Approved

### Review Notes

- Validated all acceptance criteria against the implementation and focused the review on the retrieval/search changes only.
- Fixed review findings before approval: preserved line content and aligned offsets, rejected empty queries, returned multiple matches per line, and replaced fixed integration-test sleeps with run-status polling that fails fast on non-success terminal states.
- Added regression coverage for the defense-in-depth `NotIndexed` branch, empty-query rejection, and multiple matches on the same line.
- Re-ran the full test suite after fixes; all tests passed.

## Change Log

- 2026-03-08: Story 3.1 implemented — shared contract types, request gate, text search with blob integrity verification, 36 new tests (24 unit + 4 integration + 8 conformance). Total test count: 405.
- 2026-03-08: Senior developer review completed — findings resolved, story approved, status moved to `done`, sprint tracking synced.
