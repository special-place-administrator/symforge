# Story 3.6: Block or Quarantine Suspect Retrieval

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an AI coding user,
I want suspect retrieval to fail explicitly instead of being served as trustworthy,
so that integrity problems do not silently poison my coding workflow.

**FRs implemented:** FR26, FR27, FR29

- **FR26**: The system can refuse to serve suspect or unverified retrieval as trustworthy output.
- **FR27**: Users can see when retrieval has failed verification and understand that the result is blocked, quarantined, or marked suspect.
- **FR29**: Users can distinguish between trusted retrieval results and results that require repair or re-index before use.

## Acceptance Criteria

1. **Given** retrieval verification fails because of stale spans, corrupted metadata, or byte mismatch **When** source retrieval is requested **Then** Tokenizor blocks, quarantines, or marks the result suspect explicitly **And** it does not serve the result as trusted code (AC: 1, blocking behavior — already implemented in 3.5, verified by 3.6 tests)
2. **Given** a retrieval result is blocked or quarantined **When** the result is returned **Then** Tokenizor exposes actionable trust or integrity state via a structured `next_action` field **And** the response makes repair or re-index implications understandable (AC: 2, actionable guidance)
3. **Given** a repository has `RepositoryStatus::Quarantined` **When** any retrieval operation is requested **Then** the request is gated (request-fatal) with explicit quarantine reason **And** the gate error message includes `next_action: repair` guidance (AC: 3, repo-level quarantine)
4. **Given** any blocked or quarantined `ResultEnvelope` response **When** serialized to JSON **Then** the response includes a `next_action` field with one of: `reindex`, `repair`, `wait`, `resolve_context` **And** successful results omit the field entirely (AC: 4, structured next\_action)
5. **Given** a `RequestGateError` occurs **When** the error message is returned at the MCP boundary **Then** the message includes actionable guidance identifying the recommended next action (AC: 5, gate error actionability)

## Tasks / Subtasks

### Phase 1: Domain Types

- [x] Task 1.1: Add `NextAction` enum to `src/domain/retrieval.rs` (AC: 2, 4)
  - [x] 1.1.1: Define `#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)] #[serde(rename_all = "snake_case")] pub enum NextAction { Reindex, Repair, Wait, ResolveContext }`
  - [x] 1.1.2: Doc comment: `/// Actionable guidance for blocked, quarantined, or gated responses. Shared vocabulary with Epic 4 repair flows.`

- [x] Task 1.2: Add `next_action` field to `ResultEnvelope<T>` (AC: 2, 4)
  - [x] 1.2.1: Add field: `#[serde(default, skip_serializing_if = "Option::is_none")] pub next_action: Option<NextAction>`
  - [x] 1.2.2: Position after `data` field
  - [x] 1.2.3: **CRITICAL**: This changes every `ResultEnvelope { ... }` construction site. The Rust compiler will find all of them. See Task 2.3 for next\_action mapping by outcome type.

- [x] Task 1.3: Add `RepositoryStatus::Quarantined` variant to `src/domain/index.rs` (AC: 3)
  - [x] 1.3.1: Add `Quarantined` variant after `Invalidated` in the `RepositoryStatus` enum (currently: `Pending, Ready, Degraded, Failed, Invalidated`)
  - [x] 1.3.2: Compiler will find all exhaustive `match repo.status` sites — add the new arm to each

- [x] Task 1.4: Add quarantine fields to `Repository` struct in `src/domain/repository.rs` (AC: 3)
  - [x] 1.4.1: Add `#[serde(default)] pub quarantined_at_unix_ms: Option<u64>` — follows `invalidated_at_unix_ms` pattern
  - [x] 1.4.2: Add `#[serde(default)] pub quarantine_reason: Option<String>` — follows `invalidation_reason` pattern

- [x] Task 1.5: Add `RepositoryQuarantined` to `RequestGateError` in `src/domain/retrieval.rs` (AC: 3, 5)
  - [x] 1.5.1: Add variant: `RepositoryQuarantined { reason: Option<String> }`
  - [x] 1.5.2: Update `Display` impl or derive to include the variant formatting (check existing variants for pattern)

- [x] Task 1.6: Add `next_action()` method to `RequestGateError` in `src/domain/retrieval.rs` (AC: 5)
  - [x] 1.6.1: `pub fn next_action(&self) -> NextAction` with mapping:
    - `NoActiveContext` → `ResolveContext`
    - `RepositoryInvalidated { .. }` → `Reindex`
    - `RepositoryFailed` → `Repair`
    - `RepositoryDegraded` → `Repair`
    - `RepositoryQuarantined { .. }` → `Repair`
    - `ActiveMutation { .. }` → `Wait`
    - `NeverIndexed` → `Reindex`
    - `NoSuccessfulRuns { .. }` → `Wait`

- [x] Task 1.7: Re-export `NextAction` from `src/domain/mod.rs` (AC: 4)
  - [x] 1.7.1: Add `NextAction` to the `pub use retrieval::{...}` list

### Phase 2: Application Layer

- [x] Task 2.1: Update `check_request_gate` in `src/application/search.rs` (AC: 3)
  - [x] 2.1.1: Add `RepositoryStatus::Quarantined` arm to the `match repo.status` block, positioned after `Degraded` and before `Pending | Ready`
  - [x] 2.1.2: Return `gate_error(RequestGateError::RepositoryQuarantined { reason: repo.quarantine_reason.clone() })`

- [x] Task 2.2: Update `gate_error()` helper to include next\_action in message (AC: 5)
  - [x] 2.2.1: Change from `TokenizorError::RequestGated { gate_error: error.to_string() }` to `TokenizorError::RequestGated { gate_error: format!("{error} [next_action: {action}]", action = error.next_action()) }`
  - [x] 2.2.2: This makes gate error messages at the MCP boundary self-documenting for AI clients

- [x] Task 2.3: Update ALL `ResultEnvelope` constructions in `src/application/search.rs` (AC: 2, 4)
  - [x] 2.3.1: **Strategy**: Add the `next_action` field to `ResultEnvelope` (Task 1.2), then `cargo build` — compiler lists every construction site. Fix each:
    - `outcome: Success` → `next_action: None`
    - `outcome: Empty` → `next_action: None`
    - `outcome: NotIndexed` → `next_action: None`
    - `outcome: Stale` → `next_action: None`
    - `outcome: Quarantined` (file-level) → `next_action: Some(NextAction::Repair)`
    - `outcome: Blocked { "blob read failed..." }` → `next_action: Some(NextAction::Repair)`
    - `outcome: Blocked { "blob integrity...hash mismatch" }` → `next_action: Some(NextAction::Reindex)`
    - `outcome: Blocked { "...byte range..." }` → `next_action: Some(NextAction::Reindex)`
    - `outcome: Blocked { "...non-UTF-8..." }` → `next_action: Some(NextAction::Repair)`

### Phase 3: MCP Layer

- [x] Task 3.1: Update server instructions in `src/protocol/mcp.rs` `get_info()` (AC: 2)
  - [x] 3.1.1: Add to instructions text: "Blocked or quarantined results include a next_action field indicating the recommended resolution (reindex, repair, wait, resolve_context)."

- [x] Task 3.2: No structural MCP changes needed (AC: 2, 4)
  - [x] 3.2.1: `ResultEnvelope` serialization handles `next_action` automatically via serde
  - [x] 3.2.2: `to_mcp_error` already maps `RequestGated` — no change needed; gate message now includes `[next_action: ...]` via Task 2.2
  - [x] 3.2.3: Verify `RepositoryStatus::Quarantined` doesn't appear in any MCP-layer match statements (compiler will flag if it does)

### Phase 4: Update Existing Test Construction Sites (Compiler-Guided)

- [x] Task 4.1: Update all `ResultEnvelope` constructions in `src/application/search.rs` test module (AC: 4)
  - [x] 4.1.1: Add `next_action: None` or appropriate `Some(...)` to every `ResultEnvelope` in `#[cfg(test)] mod tests`
  - [x] 4.1.2: Update `assert_eq!` comparisons that check full `ResultEnvelope` values to include `next_action`

- [x] Task 4.2: Update `tests/retrieval_conformance.rs` (AC: 4)
  - [x] 4.2.1: Add `next_action: None` to all existing `ResultEnvelope` test constructions

- [x] Task 4.3: Update `tests/retrieval_integration.rs` (AC: 4)
  - [x] 4.3.1: Add `next_action` assertions to existing tests where `ResultEnvelope` is checked

- [x] Task 4.4: Update any other test files that construct `Repository` with `status` field (AC: 3)
  - [x] 4.4.1: Compiler will flag exhaustive match updates needed for `RepositoryStatus::Quarantined`
  - [x] 4.4.2: Add `quarantined_at_unix_ms: None, quarantine_reason: None` to `Repository` constructions

### Phase 5: New Unit Tests

- [x] Task 5.1: `RequestGateError::next_action()` mapping tests in `src/domain/retrieval.rs` (AC: 5)
  - [x] 5.1.1: `test_request_gate_error_next_action_mapping` — verify every variant maps to the correct `NextAction`

- [x] Task 5.2: Repo-level quarantine gate tests in `src/application/search.rs` (AC: 3)
  - [x] 5.2.1: `test_request_gate_blocks_quarantined_repo` — quarantined repo returns `RequestGated` error with quarantine message
  - [x] 5.2.2: `test_search_text_rejects_quarantined_repo` — full `search_text` call against quarantined repo
  - [x] 5.2.3: `test_search_symbols_rejects_quarantined_repo`
  - [x] 5.2.4: `test_get_file_outline_rejects_quarantined_repo`
  - [x] 5.2.5: `test_get_symbol_rejects_quarantined_repo`
  - [x] 5.2.6: `test_get_repo_outline_rejects_quarantined_repo`

- [x] Task 5.3: NextAction in blocked/quarantined outcomes (AC: 2, 4)
  - [x] 5.3.1: `test_blocked_blob_read_includes_next_action_repair` — blob read failure → `next_action: Some(Repair)`
  - [x] 5.3.2: `test_blocked_blob_integrity_includes_next_action_reindex` — hash mismatch → `next_action: Some(Reindex)`
  - [x] 5.3.3: `test_blocked_byte_range_includes_next_action_reindex` — range out of bounds → `next_action: Some(Reindex)`
  - [x] 5.3.4: `test_blocked_non_utf8_includes_next_action_repair` — non-UTF-8 → `next_action: Some(Repair)`
  - [x] 5.3.5: `test_quarantined_file_includes_next_action_repair` — quarantined file in `get_symbol` → `next_action: Some(Repair)`
  - [x] 5.3.6: `test_quarantined_file_outline_includes_next_action_repair` — quarantined file in `get_file_outline` → `next_action: Some(Repair)`
  - [x] 5.3.7: `test_success_result_has_no_next_action` — successful retrieval → `next_action: None`
  - [x] 5.3.8: `test_gate_error_message_includes_next_action` — verify gate error string contains `[next_action: ...]`

### Phase 6: New Integration Tests

- [x] Task 6.1: Quarantine integration tests in `tests/retrieval_integration.rs` (AC: 1, 2, 3)
  - [x] 6.1.1: `test_quarantined_repo_blocks_all_retrieval` — set repo status to `Quarantined`, verify `search_text`, `search_symbols`, `get_file_outline`, `get_repo_outline`, `get_symbol` all return `RequestGated` errors
  - [x] 6.1.2: `test_blocked_result_includes_next_action_in_json` — serialize a `Blocked` `ResultEnvelope` and verify `"next_action": "reindex"` appears in JSON output
  - [x] 6.1.3: `test_quarantined_file_result_includes_next_action_in_json` — serialize a `Quarantined` `ResultEnvelope` and verify `"next_action": "repair"` in JSON
  - [x] 6.1.4: `test_success_result_omits_next_action_in_json` — serialize a `Success` `ResultEnvelope` and verify `next_action` key is absent from JSON

### Phase 7: Conformance Tests

- [x] Task 7.1: Extend `tests/retrieval_conformance.rs` (AC: 4)
  - [x] 7.1.1: `test_next_action_variants_are_exhaustive` — construct and serialize all 4 `NextAction` variants
  - [x] 7.1.2: `test_result_envelope_with_next_action_serializes` — verify JSON includes `next_action` when `Some`, omits when `None`
  - [x] 7.1.3: `test_request_gate_error_quarantined_variant` — verify `RepositoryQuarantined` variant constructs and serializes
  - [x] 7.1.4: `test_repository_status_quarantined_variant` — verify `RepositoryStatus::Quarantined` serializes to `"quarantined"`

## Dev Notes

### What Already Exists (from Stories 3.1–3.5)

Story 3.5 implemented the core verified retrieval path including all blob verification and blocking behavior. **Story 3.6 does NOT re-implement blocking** — it enriches existing blocked/quarantined responses with actionable guidance and adds repository-level quarantine gating.

**Existing quarantine handling by function:**

| Function | Current Quarantine Behavior |
|----------|---------------------------|
| `search_text_ungated` | Skips quarantined files silently (`continue`) |
| `search_symbols_ungated` | Skips quarantined files, increments `files_skipped_quarantined` counter |
| `get_file_outline_ungated` | Returns `outcome: Quarantined, trust: Quarantined` |
| `get_repo_outline_ungated` | Counts quarantined files in coverage metadata |
| `get_symbol_ungated` | Returns `outcome: Quarantined, trust: Quarantined` |

**Existing blocking behavior (all in `get_symbol_ungated`):**

| Failure Mode | Current Response |
|-------------|-----------------|
| Blob read failure | `Blocked { reason: "blob read failed..." }, trust: Suspect` |
| Blob integrity mismatch | `Blocked { reason: "blob integrity verification failed..." }, trust: Suspect` |
| Byte range out of bounds | `Blocked { reason: "symbol byte range... exceeds..." }, trust: Suspect` |
| Non-UTF-8 source | `Blocked { reason: "symbol source contains non-UTF-8 bytes" }, trust: Suspect` |

**What Story 3.6 Adds:**

1. `NextAction` enum — structured `reindex`/`repair`/`wait`/`resolve_context` guidance (shared vocabulary with Epic 4)
2. `next_action` field on `ResultEnvelope` — populated for blocked/quarantined, `None` for success
3. `RepositoryStatus::Quarantined` — new repo status variant for repo-level quarantine
4. `RequestGateError::RepositoryQuarantined` — new gate error with reason
5. Quarantine gating in `check_request_gate` — blocks all operations on quarantined repos
6. `quarantine_reason` + `quarantined_at_unix_ms` on `Repository` struct (mirrors invalidation pattern)
7. Gate error messages enriched with `[next_action: ...]` suffix
8. Comprehensive test coverage for all quarantine/blocking paths

### NextAction Mapping Reference

**Item-level outcomes (in ResultEnvelope):**

| Outcome | Trust | Next Action | Rationale |
|---------|-------|-------------|-----------|
| `Blocked { "blob read failed..." }` | Suspect | `Repair` | CAS I/O issue — data may be corrupted |
| `Blocked { "blob integrity...hash mismatch" }` | Suspect | `Reindex` | Content changed since indexing |
| `Blocked { "...byte range...exceeds..." }` | Suspect | `Reindex` | Symbol metadata stale vs actual blob |
| `Blocked { "...non-UTF-8..." }` | Suspect | `Repair` | Data corruption |
| `Quarantined` (file-level) | Quarantined | `Repair` | File flagged during indexing — needs repair |
| `Success` | Verified | `None` | No action needed |
| `Empty` | Verified | `None` | No action needed |
| `NotIndexed` | Verified | `None` | Defense-in-depth, gate should catch first |

**Request-fatal outcomes (via RequestGateError → gate\_error message):**

| Gate Error | Next Action | Example Message |
|-----------|-------------|-----------------|
| `NoActiveContext` | `resolve_context` | "no active context [next\_action: resolve\_context]" |
| `RepositoryInvalidated` | `reindex` | "repository invalidated: reason [next\_action: reindex]" |
| `RepositoryFailed` | `repair` | "repository failed [next\_action: repair]" |
| `RepositoryDegraded` | `repair` | "repository degraded [next\_action: repair]" |
| `RepositoryQuarantined` | `repair` | "repository quarantined: reason [next\_action: repair]" |
| `ActiveMutation` | `wait` | "active mutation: run-id [next\_action: wait]" |
| `NeverIndexed` | `reindex` | "never indexed [next\_action: reindex]" |
| `NoSuccessfulRuns` | `wait` | "no successful runs: latest status [next\_action: wait]" |

### Key Type Definitions (Current State)

**`Repository` struct** (`src/domain/repository.rs`):
```rust
pub struct Repository {
    pub repo_id: String,
    pub kind: RepositoryKind,
    pub root_uri: String,
    pub project_identity: String,
    pub project_identity_kind: ProjectIdentityKind,
    pub default_branch: Option<String>,
    pub last_known_revision: Option<String>,
    pub status: RepositoryStatus,
    pub invalidated_at_unix_ms: Option<u64>,
    pub invalidation_reason: Option<String>,
    // ADD: quarantined_at_unix_ms: Option<u64>,
    // ADD: quarantine_reason: Option<String>,
}
```

**`RepositoryStatus` enum** (`src/domain/index.rs`):
```rust
pub enum RepositoryStatus {
    Pending, Ready, Degraded, Failed, Invalidated,
    // ADD: Quarantined,
}
```

**`ResultEnvelope<T>`** (`src/domain/retrieval.rs`):
```rust
pub struct ResultEnvelope<T> {
    pub outcome: RetrievalOutcome,
    pub trust: TrustLevel,
    pub provenance: Option<Provenance>,
    pub data: Option<T>,
    // ADD: #[serde(default, skip_serializing_if = "Option::is_none")]
    //      pub next_action: Option<NextAction>,
}
```

**`RetrievalOutcome` enum** (`src/domain/retrieval.rs`):
```rust
pub enum RetrievalOutcome {
    Success, Empty, NotIndexed, Stale, Quarantined, Blocked { reason: String },
}
// NO CHANGES to this enum — next_action lives on ResultEnvelope, not inside variants
```

**`RequestGateError` enum** (`src/domain/retrieval.rs`):
```rust
pub enum RequestGateError {
    NoActiveContext,
    RepositoryInvalidated { reason: Option<String> },
    RepositoryFailed,
    RepositoryDegraded,
    ActiveMutation { run_id: String },
    NeverIndexed,
    NoSuccessfulRuns { latest_status: IndexRunStatus },
    // ADD: RepositoryQuarantined { reason: Option<String> },
}
```

### check\_request\_gate After Changes

In `src/application/search.rs` — add the `Quarantined` arm:
```rust
match repo.status {
    RepositoryStatus::Invalidated => { /* existing */ }
    RepositoryStatus::Failed => { /* existing */ }
    RepositoryStatus::Degraded => { /* existing */ }
    RepositoryStatus::Quarantined => {
        return Err(gate_error(RequestGateError::RepositoryQuarantined {
            reason: repo.quarantine_reason.clone(),
        }));
    }
    RepositoryStatus::Pending | RepositoryStatus::Ready => {}
}
```

### Blast Radius Assessment

Adding `next_action: Option<NextAction>` to `ResultEnvelope` requires updating every construction site. The Rust compiler will find ALL of them.

| Location | Approx. Sites | Change Type |
|----------|--------------|-------------|
| `src/application/search.rs` (prod code) | ~22 | Add `next_action: None` or `Some(...)` |
| `src/application/search.rs` (unit tests) | ~45 | Add `next_action` to assertions |
| `tests/retrieval_conformance.rs` | ~20 | Add `next_action: None` to constructions |
| `tests/retrieval_integration.rs` | ~15 | Add `next_action` to assertions |

**Strategy**: Add the field to `ResultEnvelope`, then `cargo build` — compiler lists every site. Fix in order: domain → application (prod) → application (tests) → conformance → integration.

Similarly, adding `RepositoryStatus::Quarantined` requires updating exhaustive matches. Adding `quarantine_reason` + `quarantined_at_unix_ms` to `Repository` requires updating all `Repository` struct constructions.

### Quarantine Policy Summary

**Two-level quarantine (from Epic 3 narrative):**

| Level | Scope | Behavior | Example |
|-------|-------|----------|---------|
| Repo-level | `RepositoryStatus::Quarantined` | Request-fatal — all operations blocked | Entire repo flagged for repair |
| File-level | `PersistedFileOutcome::Quarantined` | Search: excluded. Targeted: returned with `trust: Quarantined` | Individual file flagged during indexing |

**Who sets quarantine?** Story 3.6 only implements the read-side gating. Write-side quarantine transitions (who/what marks a repo or file as quarantined) belong to Epic 4 (repair flows). Tests manually set quarantine status to verify the gate works.

### Scope Boundaries — What Story 3.6 Does NOT Cover

- Write-side quarantine transitions (Epic 4)
- Quarantine repair/clearing actions (Epic 4)
- Batched retrieval quarantine handling (Story 3.7 — will inherit from 3.6)
- Live filesystem change detection or watch-based invalidation
- Freshness policy enforcement (Epic 4/5)
- New source files — all changes go in existing modules

### Guard Against Failure Modes

1. **Don't add `next_action` to `RetrievalOutcome` variants** — keep it on `ResultEnvelope` to avoid changing enum JSON serialization structure
2. **Don't skip `#[serde(skip_serializing_if = "Option::is_none")]`** — successful results must NOT include `"next_action": null` in JSON
3. **Don't forget `#[serde(default)]`** — ensures backward-compatible deserialization of older JSON without `next_action`
4. **Don't create new files** — keep all changes in existing modules (`retrieval.rs`, `index.rs`, `repository.rs`, `search.rs`, `mcp.rs`, `mod.rs`)
5. **Don't change the `Blocked { reason }` variant structure** — reason stays as String, next\_action goes on the envelope
6. **Don't implement quarantine WRITE operations** — 3.6 only does read-side gating; tests manually set status
7. **Don't add `Quarantined` to `is_systemic()` in error.rs** — quarantine flows through `RequestGated` which is already `false`

### Epic 3 Retrieval Architecture Rules (Mandatory)

1. **Rule 1 (blob verification)**: Already implemented in 3.5. Story 3.6 adds `next_action` to the blocked response.
2. **Rule 2 (repo status check)**: Story 3.6 extends the gate with `RepositoryStatus::Quarantined`.
3. **Rule 3 (provenance)**: Unchanged — all envelopes include provenance.
4. **Rule 4 (disambiguation)**: Story 3.6 adds `next_action` to further disambiguate blocked/quarantined outcomes.
5. **Rule 5 (early gating)**: Gate runs FIRST. Quarantine gate is request-fatal.
6. **Rule 6 (quarantine in search)**: Unchanged — quarantined files excluded from search results.

### Previous Story Intelligence (from Story 3.5)

**Key patterns established:**
- `FakeBlobStore` for controlling blob content in tests — reuse for blocked outcome tests
- `setup_test_env()` / `setup_verified_env()` patterns — extend for quarantined repo setup
- `check_request_gate()` is the universal entry gate — all retrieval functions call it first
- `gate_error()` helper wraps `RequestGateError` into `TokenizorError::RequestGated`
- `file_record_provenance()` helper builds provenance — no changes needed
- `parse_kind_filter()` and `required_non_empty_string_param()` reusable MCP helpers
- Empty string validation on required parameters — maintain consistency

**Story 3.5 review findings to carry forward:**
- Every test assertion MUST be able to fail (no `assert!(true)`)
- Weak assertions (`assert!(result.is_ok())`) are insufficient — check data contents
- Tools go in `#[tool_router]`, NOT `#[tool_handler]`
- `to_mcp_error()` in `src/protocol/mcp.rs` maps `RequestGated` → `invalid_params`

**Test count after Story 3.5:** 549 tests, 0 failures.

### Self-Audit Checklist (mandatory before requesting review)

_Run this checklist after all tasks are complete. This is a blocking step — do not request review until every item is verified._

#### Generic Verification
- [x] For every task marked `[x]`, cite the specific test that verifies it
- [x] For every new error variant or branch, confirm a test exercises it
- [x] For every computed value, trace it to where it surfaces (log, return value, persistence)
- [x] For every test, verify the assertion can actually fail (no `assert!(true)`, no conditionals that always pass)

#### Epic 3-Specific Trust Verification
- [x] For every retrieval path, confirm a test exercises blob_id trust verification
- [x] For every query, confirm a test exercises the invalidated/unhealthy rejection path
- [x] For every "no results" path, confirm the response distinguishes empty vs missing vs stale

#### Story 3.6-Specific Verification
- [x] Confirm `RepositoryStatus::Quarantined` is handled in `check_request_gate` as request-fatal
- [x] Confirm `RequestGateError::RepositoryQuarantined` is defined with `reason: Option<String>`
- [x] Confirm `NextAction` enum has exactly 4 variants: `Reindex`, `Repair`, `Wait`, `ResolveContext`
- [x] Confirm `ResultEnvelope.next_action` uses `#[serde(default, skip_serializing_if = "Option::is_none")]`
- [x] Confirm ALL existing `ResultEnvelope` constructions compile with new `next_action` field
- [x] Confirm `next_action: None` for `Success`, `Empty`, `NotIndexed`, `Stale` outcomes
- [x] Confirm `next_action: Some(Repair)` for blob read failure and quarantined file
- [x] Confirm `next_action: Some(Reindex)` for blob integrity mismatch and byte range out of bounds
- [x] Confirm `next_action: Some(Repair)` for non-UTF-8 source
- [x] Confirm repo-level quarantine blocks ALL 5 retrieval operations (`search_text`, `search_symbols`, `get_file_outline`, `get_repo_outline`, `get_symbol`)
- [x] Confirm gate error messages include `[next_action: ...]` suffix
- [x] Confirm server instructions text mentions quarantine handling and `next_action`
- [x] Confirm JSON serialization omits `next_action` when `None`, includes it when `Some`
- [x] Confirm `Repository` struct has `quarantined_at_unix_ms` and `quarantine_reason` fields
- [x] Confirm no new files were created — all changes in existing modules
- [x] Confirm full test suite passes (`cargo test` — 572 tests, 0 failures)

### Project Structure Notes

**Modified Files (no new files):**

| File | Changes |
|------|---------|
| `src/domain/retrieval.rs` | `NextAction` enum, `next_action` field on `ResultEnvelope`, `RepositoryQuarantined` gate error variant, `next_action()` method on `RequestGateError` |
| `src/domain/index.rs` | `RepositoryStatus::Quarantined` variant |
| `src/domain/repository.rs` | `quarantined_at_unix_ms` + `quarantine_reason` fields |
| `src/domain/mod.rs` | Re-export `NextAction` |
| `src/application/search.rs` | Gate check update, `next_action` on all `ResultEnvelope` constructions, `gate_error()` message enrichment, ~15 new unit tests |
| `src/protocol/mcp.rs` | Server instructions text update |
| `tests/retrieval_conformance.rs` | Updated constructions + ~4 new conformance tests |
| `tests/retrieval_integration.rs` | Updated assertions + ~4 new integration tests |

**Alignment**: All changes follow established patterns. No new modules, no new architectural layers.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.6] — ACs: block/quarantine suspect retrieval, expose actionable trust state
- [Source: _bmad-output/planning-artifacts/epics.md#Epic 3 Execution Narrative] — Phase 3 entry gate, story ownership: "3.6 owns blocked/quarantined behavior, explicit suspect outcomes, and associated tests"
- [Source: _bmad-output/planning-artifacts/epics.md#Contract Gaps] — "Epic 3→4 shared next\_action vocabulary: reindex, repair, wait, resolve\_context"
- [Source: _bmad-output/planning-artifacts/epics.md#Failure Mode Guidance] — Request-fatal vs item-local; repo-level vs file-level quarantine policy
- [Source: _bmad-output/planning-artifacts/architecture.md#Retrieval Trust Model] — "This is the core trust contract of the product"
- [Source: _bmad-output/planning-artifacts/architecture.md#Integrity / Quarantine Model] — "quarantine state must be inspectable and repairable"
- [Source: _bmad-output/planning-artifacts/architecture.md#Error / Result Semantics] — "domain-level states should be represented explicitly in result models"
- [Source: _bmad-output/planning-artifacts/architecture.md#Anti-Patterns] — "generic success: false for a repair-required or quarantined domain outcome"
- [Source: _bmad-output/planning-artifacts/architecture.md#Good Examples] — "retrieval span verification failure becomes quarantined or blocked, not silent fallback"
- [Source: _bmad-output/implementation-artifacts/3-5-retrieve-verified-source-for-a-symbol-or-code-slice.md] — Previous story patterns, FakeBlobStore, test conventions, 549 passing tests
- [Source: docs/project-context.md#Epic 3 Rules] — Six mandatory retrieval architecture rules
- [Source: src/domain/retrieval.rs] — Current ResultEnvelope, RetrievalOutcome, TrustLevel, RequestGateError definitions
- [Source: src/domain/repository.rs] — Current Repository struct with invalidation fields pattern
- [Source: src/application/search.rs] — Current check_request_gate, gate_error helper, all retrieval functions

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- Compiler-guided refactoring: added `next_action` field to `ResultEnvelope`, then fixed ~100 construction sites across production and test code using `cargo build` error output
- Parallelized test writing with 3 background agents for Phases 5, 6, and 7

### Completion Notes List

- All 7 phases complete: domain types, application layer, MCP layer, existing test fixes, unit tests, integration tests, conformance tests
- 577 tests pass after review hardening (572 prior + 5 new review regression tests), 0 failures
- Self-audit checklist: 13/13 items verified PASS
- No new source files created — all changes in existing modules

### Change Log

| File | Changes |
|------|---------|
| `src/domain/retrieval.rs` | Added `NextAction` enum (4 variants), `next_action` field on `ResultEnvelope`, `RepositoryQuarantined` gate error variant, `next_action()` method on `RequestGateError`, and `Display` impls for exact gate-action formatting |
| `src/domain/repository.rs` | Added `RepositoryStatus::Quarantined`, `quarantined_at_unix_ms: Option<u64>`, `quarantine_reason: Option<String>`, and round-trip coverage for the new fields |
| `src/domain/mod.rs` | Re-exported `NextAction` |
| `src/application/search.rs` | Added `Quarantined` arm to `check_request_gate`, persisted queued/running mutation gating, exact `[next_action: ...]` gate message formatting, `next_action` on all `ResultEnvelope` constructions, and stronger unit coverage for quarantine reason and persisted-run gating |
| `src/application/init.rs` | Added quarantine fields to 2 `Repository` construction sites |
| `src/application/run_manager.rs` | Added quarantine fields to `Repository` construction sites and updated repository status transitions to preserve/clear quarantine metadata explicitly |
| `src/storage/registry_persistence.rs` | Added quarantine fields to `Repository` construction sites and extended repository status persistence to write and clear quarantine metadata deterministically |
| `src/protocol/mcp.rs` | Updated server instructions text to mention quarantine handling and `next_action` |
| `tests/retrieval_conformance.rs` | Added `next_action: None` to 7 existing `ResultEnvelope` constructions, added 4 new conformance tests |
| `tests/retrieval_integration.rs` | Added quarantine fields to `register_repo` helper and strengthened integration assertions to verify exact quarantine reason and `next_action` rendering across all retrieval surfaces |
| `tests/indexing_integration.rs` | Added quarantine fields to `seed_integration_repo` helper |

### File List

- `src/domain/retrieval.rs`
- `src/domain/repository.rs`
- `src/domain/mod.rs`
- `src/application/search.rs`
- `src/application/init.rs`
- `src/application/run_manager.rs`
- `src/storage/registry_persistence.rs`
- `src/protocol/mcp.rs`
- `tests/retrieval_conformance.rs`
- `tests/retrieval_integration.rs`
- `tests/indexing_integration.rs`

## Senior Developer Review (AI)

### Reviewer

Codex (GPT-5) — 2026-03-08

### Outcome

Approved after fixes

### Review Notes

- Fixed the read-side gate so persisted `Queued` and `Running` runs block retrieval even before an in-memory active-run handle is registered.
- Tightened `next_action` rendering to the documented contract format (`[next_action: repair]`, `[next_action: reindex]`, `[next_action: wait]`) instead of JSON-quoted strings.
- Extended repository status persistence so quarantine timestamp/reason fields are actually written and cleared through the normal update path rather than being write-only struct fields.
- Strengthened unit and integration tests to assert exact quarantine reason propagation and gate-message formatting, not just substring presence.
- Re-ran `cargo test --workspace --quiet`; 577 tests passed.
