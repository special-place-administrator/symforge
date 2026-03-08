# Story 3.7: Retrieve Multiple Symbols or Code Slices in One Workflow

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an AI coding workflow,
I want to retrieve multiple symbols or code slices in one request path,
so that I can gather grounded code context efficiently.

**FRs implemented:** FR22

- **FR22**: Users can retrieve multiple symbols or code slices in one workflow when needed.

## Acceptance Criteria

1. **Given** multiple valid retrieval targets exist in the active context **When** I request batched retrieval **Then** Tokenizor returns each result with independent trust or integrity state where relevant **And** one failed item does not silently invalidate unrelated successful items (AC: 1, per-item independence)
2. **Given** some requested items are missing or suspect **When** batched retrieval completes **Then** Tokenizor reports mixed outcomes explicitly **And** it preserves determinism about which items were trusted, blocked, or absent (AC: 2, mixed outcome reporting)
3. **Given** request-level gating fails (unhealthy repo, no context, active mutation) **When** batched retrieval is attempted **Then** the entire batch fails uniformly before any item processing **And** the gate error includes `next_action` guidance (AC: 3, request-level gating)
4. **Given** the batch request passes gating **When** individual items fail verification (blob mismatch, quarantined file, byte range error) **Then** those items report their own outcome/trust/next_action independently **And** other items in the batch succeed normally (AC: 4, mixed per-item outcomes after gate pass)
5. **Given** a batched retrieval request with zero items **When** the request is processed **Then** Tokenizor returns an empty result (not an error) with appropriate outcome (AC: 5, empty batch handling)

## Tasks / Subtasks

### Phase 1: Domain Types

- [x] Task 1.1: Add batched retrieval request types to `src/domain/retrieval.rs` (AC: 1, 2)
  - [x]1.1.1: Define `SymbolRequest`, `CodeSliceRequest`, and tagged `BatchRetrievalRequest` variants for verified symbol and byte-range targets
  - [x]1.1.2: Add doc comments clarifying symbol vs code-slice target semantics within a batched `get_symbols` request
  - [x]1.1.3: Derive `schemars::JsonSchema` for the batched request model and dependent enums needed for MCP parameter schema generation

- [x] Task 1.2: Add `BatchRetrievalResultItem` and `BatchRetrievalResponseData` to `src/domain/retrieval.rs` (AC: 1, 2, 4)
  - [x]1.2.1: Define result variants that preserve request identity and wrap `ResultEnvelope<BatchRetrievalResponseData>` for symbol and code-slice items
  - [x]1.2.2: Use explicit enum variants instead of `#[serde(flatten)]` so symbol and code-slice batch items serialize deterministically

- [x] Task 1.3: Add `GetSymbolsResponse` struct to `src/domain/retrieval.rs` (AC: 1, 2, 5)
  - [x]1.3.1: Define:
    ```rust
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct GetSymbolsResponse {
        pub results: Vec<BatchRetrievalResultItem>,
    }
    ```
  - [x]1.3.2: This is the `T` in `ResultEnvelope<GetSymbolsResponse>` — the batch envelope wraps the item-level envelopes

- [x] Task 1.4: Re-export new types from `src/domain/mod.rs` (AC: 1)
  - [x]1.4.1: Add `SymbolRequest`, `CodeSliceRequest`, `BatchRetrievalRequest`, `BatchRetrievalResultItem`, `BatchRetrievalResponseData`, `VerifiedCodeSliceResponse`, and `GetSymbolsResponse` to the `pub use retrieval::{...}` list

### Phase 2: Application Layer

- [x] Task 2.1: Implement `get_symbols()` public function in `src/application/search.rs` (AC: 1, 3)
  - [x]2.1.1: Signature:
    ```rust
    pub fn get_symbols(
        repo_id: &str,
        requests: &[BatchRetrievalRequest],
        persistence: &RegistryPersistence,
        run_manager: &RunManager,
        blob_store: &dyn BlobStore,
    ) -> Result<ResultEnvelope<GetSymbolsResponse>>
    ```
  - [x]2.1.2: **Step 1 — Empty batch check**: If `requests.is_empty()`, return early with `ResultEnvelope { outcome: Empty, trust: Verified, provenance: None, data: Some(GetSymbolsResponse { results: vec![] }), next_action: None }`
  - [x]2.1.3: **Step 2 — Request-level gating**: Call `check_request_gate(repo_id, persistence, run_manager)?` — if this fails, the entire batch fails uniformly (existing gate behavior)
  - [x]2.1.4: **Step 3 — Delegate to ungated**: Call `get_symbols_ungated(repo_id, requests, persistence, blob_store)`

- [x] Task 2.2: Implement `get_symbols_ungated()` private function in `src/application/search.rs` (AC: 1, 2, 4)
  - [x]2.2.1: Signature:
    ```rust
    fn get_symbols_ungated(
        repo_id: &str,
        requests: &[BatchRetrievalRequest],
        persistence: &RegistryPersistence,
        blob_store: &dyn BlobStore,
    ) -> Result<ResultEnvelope<GetSymbolsResponse>>
    ```
  - [x]2.2.2: **Fetch latest completed run once** (not per-item): Same pattern as `get_symbol_ungated` lines 554-570 — `persistence.load_latest_completed_run(repo_id)?`. If no completed run, return `ResultEnvelope { outcome: NotIndexed, trust: Verified, provenance: None, data: None, next_action: None }`
  - [x]2.2.3: **Load file records once**: Load `run.file_records` (or access the run's file records). This avoids repeated I/O per item.
  - [x]2.2.4: **Process each request independently**: Dispatch each `BatchRetrievalRequest` to verified symbol or verified code-slice retrieval using the already-loaded run and file-record set.
  - [x]2.2.5: **Avoid repeated registry loads**: Reuse the completed run and file records across the batch instead of calling the cold `get_symbol_ungated()` path per item.
  - [x]2.2.6: **Collect results**: Build `Vec<BatchRetrievalResultItem>` by wrapping each per-item `ResultEnvelope<BatchRetrievalResponseData>` with its request identity and preserving request order.
  - [x]2.2.7: **Determine batch-level outcome**:
    - If all items are `Success` → batch outcome `Success`
    - If all items are failures (Blocked/Quarantined/NotIndexed) → batch outcome `Blocked { reason: "all items failed" }` or appropriate
    - If mixed → batch outcome `Success` (the batch itself succeeded; per-item outcomes tell the full story)
    - **Simplest correct approach**: Always return batch outcome `Success` with `trust: Verified` when the gate passes and at least one item exists. The per-item `ResultEnvelope` values carry the true outcome/trust/next_action for each symbol. The batch envelope signals "the operation completed" not "all items succeeded."
  - [x]2.2.8: **Build provenance**: Use the run's provenance for the batch envelope (same pattern as other functions — `file_record_provenance` from the first file record, or a run-level provenance).
  - [x]2.2.9: **Preserve request order**: Results array MUST match the order of the input `requests` array. Deterministic, request-order preserved (per Epic 3 narrative Phase 4).

- [x] Task 2.3: Add `get_symbols()` method to `ApplicationContext` in `src/application/mod.rs` (AC: 1)
  - [x]2.3.1: Follow the existing `get_symbol()` delegation pattern:
    ```rust
    pub fn get_symbols(
        &self,
        repo_id: &str,
        requests: &[BatchRetrievalRequest],
    ) -> Result<ResultEnvelope<GetSymbolsResponse>> {
        search::get_symbols(
            repo_id,
            requests,
            &self.persistence,
            &self.run_manager,
            self.blob_store.as_ref(),
        )
    }
    ```

### Phase 3: MCP Layer

- [x] Task 3.1: Add `get_symbols` tool to `#[tool_router]` in `src/protocol/mcp.rs` (AC: 1, 2)
  - [x]3.1.1: Add tool definition following the `get_symbol` pattern:
    ```rust
    #[tool(
        description = "Retrieve verified source code for multiple symbols from an indexed repository in a single request. Each symbol is verified independently — one failure does not affect others. Returns per-item outcomes with trust and provenance metadata."
    )]
    fn get_symbols(&self, params: rmcp::model::JsonObject) -> Result<CallToolResult, McpError>
    ```
  - [x]3.1.2: Tool handler implementation:
    - Parse params using `parse_get_symbols_params()`
    - Call `self.application.get_symbols(repo_id, &requests)`
    - Serialize result with `serde_json::to_string_pretty()`
    - Wrap in `CallToolResult::success()` or map errors with `to_mcp_error()`

- [x] Task 3.2: Implement `parse_get_symbols_params()` in `src/protocol/mcp.rs` (AC: 1)
  - [x]3.2.1: Define params structure:
    ```rust
    struct GetSymbolsParams {
        repo_id: String,
        requests: Vec<BatchRetrievalRequest>,
    }
    ```
  - [x]3.2.2: Parse `repo_id` using `required_non_empty_string_param(params, "repo_id")`
  - [x]3.2.3: Parse preferred `targets` entries for mixed symbol/code-slice requests and accept legacy `symbols` as a backward-compatible symbol-only alias.
  - [x]3.2.4: Validate: return `McpError` if a target is malformed, has an invalid `kind_filter`, or supplies an invalid `byte_range`
  - [x]3.2.5: **Batch size limit**: Enforce a maximum batch size (50 items). Return `invalid_params` if exceeded.

- [x] Task 3.3: Update server instructions in `get_info()` (AC: 1, 2)
  - [x]3.3.1: Add to instructions text: "Use `get_symbols` to retrieve multiple symbols or code slices in a single request for efficiency. Each target is verified independently and request-level gating applies to the entire batch."
  - [x]3.3.2: Mention the preferred `targets` parameter format, the legacy `symbols` alias, and the batch size limit

### Phase 4: Unit Tests

- [x] Task 4.1: Batch gate tests in `src/application/search.rs` `#[cfg(test)] mod tests` (AC: 3)
  - [x]4.1.1: `test_get_symbols_rejects_invalidated_repo` — batch request against invalidated repo returns `RequestGated` error
  - [x]4.1.2: `test_get_symbols_rejects_quarantined_repo` — batch request against quarantined repo returns `RequestGated` error
  - [x]4.1.3: `test_get_symbols_rejects_active_mutation` — batch request during active mutation returns `RequestGated` error
  - [x]4.1.4: `test_get_symbols_rejects_never_indexed` — batch request against never-indexed repo returns `RequestGated` error

- [x] Task 4.2: Batch success tests in `src/application/search.rs` (AC: 1)
  - [x]4.2.1: `test_get_symbols_returns_multiple_verified_results` — batch of 3 valid symbols returns 3 `Success` results with `trust: Verified` and correct source text
  - [x]4.2.2: `test_get_symbols_preserves_request_order` — results array order matches input requests array order
  - [x]4.2.3: `test_get_symbols_supports_code_slice_targets` / `test_get_symbols_with_kind_filter` — mixed symbol and code-slice batches verify correctly and symbol filters still work

- [x] Task 4.3: Mixed outcome tests in `src/application/search.rs` (AC: 2, 4)
  - [x]4.3.1: `test_get_symbols_mixed_outcomes_valid_and_quarantined` — one valid symbol + one quarantined file → first is `Success`, second is `Quarantined` with `next_action: Repair`
  - [x]4.3.2: `test_get_symbols_mixed_outcomes_valid_and_missing` — one valid symbol + one non-existent path → first is `Success`, second is explicit `Missing`
  - [x]4.3.3: `test_get_symbols_mixed_outcomes_valid_and_blob_mismatch` — one valid + one with corrupted blob → first `Success`, second `Blocked` with `next_action: Reindex`
  - [x]4.3.4: `test_get_symbols_one_failure_does_not_affect_others` — explicitly verify that a failing item mid-batch does not cause subsequent items to fail

- [x] Task 4.4: Edge case tests in `src/application/search.rs` (AC: 5)
  - [x]4.4.1: `test_get_symbols_empty_batch_returns_empty` — empty `requests` array returns `outcome: Empty` with empty results vec
  - [x]4.4.2: `test_get_symbols_single_item_batch` — batch with one item behaves identically to `get_symbol`
  - [x]4.4.3: `test_get_symbols_duplicate_requests` — same symbol requested twice returns two independent results (both `Success`)
  - [x]4.4.4: `test_get_symbols_not_indexed_returns_not_indexed` — batch against repo with no completed runs returns `NotIndexed`

### Phase 5: Integration Tests

- [x] Task 5.1: Batch integration tests in `tests/retrieval_integration.rs` (AC: 1, 2, 3)
  - [x]5.1.1: `test_get_symbols_batch_verified_retrieval` — full pipeline: index repo → batch retrieve mixed symbol/code-slice targets → verify all return `Success` with correct source and `trust: Verified`
  - [x]5.1.2: `test_get_symbols_batch_gate_failure_blocks_all` — index repo → set status to `Invalidated` → batch request fails uniformly with `RequestGated`
  - [x]5.1.3: `test_get_symbols_batch_mixed_outcomes_in_json` — serialize a batch response with mixed symbol/code-slice outcomes and verify JSON structure includes per-item `outcome`, `trust`, `next_action` fields

### Phase 6: Conformance Tests

- [x] Task 6.1: Batch conformance tests in `tests/retrieval_conformance.rs` (AC: 1, 2)
  - [x]6.1.1: `test_symbol_request_serializes` / `test_code_slice_request_serializes` — symbol and code-slice requests round-trip through JSON correctly
  - [x]6.1.2: `test_batch_retrieval_request_serializes` — mixed batch request targets serialize deterministically
  - [x]6.1.3: `test_verified_code_slice_response_serializes` / `test_batch_retrieval_result_item_serializes` — code-slice responses and mixed batch items round-trip correctly
  - [x]6.1.4: `test_get_symbols_response_serializes` / `test_batch_envelope_success_omits_next_action` — batch response envelopes serialize correctly and omit `next_action` when `None`

### Review Follow-ups (AI)

- [x] [AI-Review][HIGH] Extended the batch retrieval contract to support verified code-slice targets while preserving symbol batching.
- [x] [AI-Review][HIGH] Preserved an explicit absent outcome for missing files or symbols via `RetrievalOutcome::Missing`.
- [x] [AI-Review][MEDIUM] Implemented `schemars::JsonSchema` support for the batched request model and dependent enums used in MCP params.
- [x] [AI-Review][MEDIUM] Removed repeated registry loads from the batch path by sharing the completed run and file-record set across all items.
- [x] [AI-Review][MEDIUM] Reconciled the story record with the actual set of modified and newly added source files.

## Dev Notes

### What Already Exists (from Stories 3.1–3.6)

Story 3.5 implemented the core verified retrieval path (`get_symbol` / `get_symbol_ungated`) plus verified code slices. Story 3.6 added `NextAction`, `RepositoryStatus::Quarantined`, and enriched blocked/quarantined responses. **Story 3.7 builds on top of both** — it does NOT re-implement verification or gating, it orchestrates existing verified symbol and code-slice retrieval into a batch operation.

**Existing functions to reuse/delegate to:**

| Function | Location | Role in 3.7 |
|----------|----------|-------------|
| `check_request_gate()` | `search.rs` | Single gate check for entire batch |
| `get_symbol_ungated()` | `search.rs` | Per-item retrieval with full verification |
| `gate_error()` | `search.rs` | Wraps `RequestGateError` into `TokenizorError::RequestGated` |
| `file_record_provenance()` | `search.rs` | Builds provenance from file record |
| `to_mcp_error()` | `mcp.rs` | Error mapping at MCP boundary |
| `required_non_empty_string_param()` | `mcp.rs` | Parameter validation helper |
| `parse_kind_filter()` | `mcp.rs` | Symbol kind parsing helper |

**Current test infrastructure:**
- `FakeBlobStore` — controls blob content for verification tests
- `setup_test_env()` / `setup_verified_env()` — standard test environment setup
- Test helper patterns for registering repos, creating runs, persisting file records
- 577 tests passing at story start (442 lib + 64 integration + 31 conformance + 31 retrieval + 6 grammar + 3 other)

### Batched Retrieval Architecture (from Epic 3 Narrative)

**Phase 4 (Batched Retrieval)** is defined in the Epic 3 execution narrative:
- Per-item outcomes with batch summary
- Request-level gating passes first — if it fails, the whole batch fails uniformly before item processing
- If it passes, mixed per-item outcomes are returned
- Deterministic, request-order preserved

**ADR-4**: Per-item outcomes in batch, only after request gating passes. Preserves valid work; gate failure = uniform batch failure.

**Phase 4 Entry Gate** (from narrative): 3.5 done (YES); batch size limits decided; per-item outcome model confirmed.

### Design Decisions for Story 3.7

**D1: Batch operates within a single repo.** The `repo_id` is a top-level parameter, not per-item. Request gating is a single check. This matches the existing single-repo pattern of all Epic 3 operations and the MCP context integrity rule (no repo/workspace override per-item).

**D2: Per-item delegation to `get_symbol_ungated`.** The simplest correct approach is to call `get_symbol_ungated` for each item in the batch. This reuses all existing verification, quarantine, and blocking logic. If performance becomes a concern (repeated run/file-record loading), a future optimization can factor out the inner loop — but correctness first.

**D3: Batch-level envelope outcome.** The outer `ResultEnvelope<GetSymbolsResponse>` uses:
- `outcome: Success` — when the gate passes and processing completes (regardless of per-item outcomes)
- `outcome: Empty` — when the requests array is empty
- `outcome: NotIndexed` — when no completed runs exist
- `trust: Verified` — batch-level trust (per-item trust varies)
- `next_action: None` — batch-level (per-item next_action varies)

**D4: Batch size limit.** Enforce a maximum (suggest 50). This prevents DoS via unbounded batch size. Return `invalid_params` error if exceeded.

**D5: Error handling for per-item failures.** Missing files or symbols must surface as explicit `RetrievalOutcome::Missing`, while quarantined files, blob mismatches, and invalid code-slice ranges must be captured as item-level outcomes without aborting unrelated items. The key implementation requirement is to convert per-item failures into `BatchRetrievalResultItem` values rather than propagating them as batch-level errors after the gate passes.

### Key Implementation Pattern

```rust
// Pseudocode for get_symbols_ungated
fn get_symbols_ungated(...) -> Result<ResultEnvelope<GetSymbolsResponse>> {
    let run = persistence.load_latest_completed_run(repo_id)?;
    // if no run → return NotIndexed

    let results: Vec<BatchRetrievalResultItem> = requests.iter().map(|req| {
        match req {
            BatchRetrievalRequest::Symbol { .. } => {
                // Verify symbol against shared run/file-record data.
                // Missing targets map to RetrievalOutcome::Missing.
            }
            BatchRetrievalRequest::CodeSlice { .. } => {
                // Verify raw byte range against shared run/file-record data.
            }
        }
    }).collect();

    Ok(ResultEnvelope {
        outcome: Success,
        trust: Verified,
        provenance: /* from run */,
        data: Some(GetSymbolsResponse { results }),
        next_action: None,
    })
}
```

### NextAction Mapping for Batch (Inherited from 3.6)

Per-item `next_action` is inherited from `get_symbol_ungated` — no new mappings needed:

| Per-Item Outcome | Trust | Next Action | Source |
|-----------------|-------|-------------|--------|
| `Success` | Verified | `None` | 3.5 |
| `Quarantined` (file-level) | Quarantined | `Repair` | 3.6 |
| `Blocked { "blob read failed..." }` | Suspect | `Repair` | 3.6 |
| `Blocked { "blob integrity...hash mismatch" }` | Suspect | `Reindex` | 3.6 |
| `Blocked { "...byte range..." }` | Suspect | `Reindex` | 3.6 |
| `Blocked { "...non-UTF-8..." }` | Suspect | `Repair` | 3.6 |
| Error (InvalidArgument) → Blocked | Suspect | `Repair` | 3.7 (new wrapping) |

### MCP Parameter Schema

```json
{
  "repo_id": "string (required, non-empty)",
  "symbols": [
    {
      "relative_path": "string (required, non-empty)",
      "symbol_name": "string (required, non-empty)",
      "kind_filter": "string (optional) — function, struct, class, etc."
    }
  ]
}
```

### Scope Boundaries — What Story 3.7 Does NOT Cover

- Cross-repo batching (all items must be in the same repo)
- Streaming/chunked responses (entire result returned at once)
- Batch-level deduplication (duplicate requests are processed independently)
- Parallel per-item processing (sequential is correct first; parallel is optimization)
- Write-side operations (read-only)
- New retrieval verification logic (delegates to existing `get_symbol_ungated`)
- Freshness policy enforcement (Epic 4/5)

### Guard Against Failure Modes

1. **Don't let per-item errors abort the batch** — `get_symbol_ungated` returning `Err` for one item must NOT propagate as a batch error. Catch and wrap.
2. **Don't create separate gating per item** — single `check_request_gate` for the entire batch. Per-item gating wastes CPU and violates the "gate first, then process" rule.
3. **Don't reorder results** — output array must match input array order. Use `.iter().map()` not `.par_iter()` or hash-based collection.
4. **Don't skip empty batch validation** — empty `requests` array is valid, returns `Empty` not an error.
5. **Don't add `next_action` to the batch-level envelope** — it's always `None` at batch level. Per-item `next_action` carries the guidance.
6. **Don't create new files for types** — all domain types go in existing `src/domain/retrieval.rs`.
7. **Don't change existing `get_symbol` or `get_symbol_ungated`** — 3.7 is additive, not modifying.
8. **Don't skip batch size limit** — unbounded batches are a DoS vector.
9. **Don't use `#[serde(flatten)]` without testing** — verify JSON serialization is clean; if flatten causes issues, nest instead.

### Epic 3 Retrieval Architecture Rules (Mandatory)

1. **Rule 1 (blob verification)**: Inherited from `get_symbol_ungated` — each item verified independently.
2. **Rule 2 (repo status check)**: Single `check_request_gate` for entire batch.
3. **Rule 3 (provenance)**: Each per-item result includes provenance from its file record.
4. **Rule 4 (disambiguation)**: Per-item outcomes distinguish Success/Empty/NotIndexed/Stale/Quarantined/Blocked.
5. **Rule 5 (early gating)**: Gate runs FIRST before any item processing.
6. **Rule 6 (quarantine in search)**: Quarantined files return `Quarantined` outcome per-item, not excluded.

### Previous Story Intelligence (from Story 3.6)

**Key patterns established in 3.6:**
- `NextAction` enum with 4 variants — reuse directly
- `next_action` on `ResultEnvelope` — inherited for per-item results
- `RepositoryStatus::Quarantined` — gate blocks batch if repo is quarantined
- `gate_error()` message includes `[next_action: ...]` — inherited for batch gate failures
- Compiler-guided refactoring approach for `ResultEnvelope` construction sites
- 577 tests at 3.6 completion — 3.7 should add ~20-30 more

**Story 3.6 review findings to carry forward:**
- Every test assertion MUST be able to fail (no `assert!(true)`)
- Weak assertions (`assert!(result.is_ok())`) are insufficient — check data contents
- Tools go in `#[tool_router]`, NOT `#[tool_handler]`
- `to_mcp_error()` in `src/protocol/mcp.rs` maps `RequestGated` → `invalid_params`
- Exact `[next_action: ...]` format in gate error messages (not JSON-quoted)

**Git intelligence:**
- Recent commits are all Epic 2/3 completion docs — no ongoing code changes to conflict with
- Story 3.6 was the most recent code implementation commit

### Self-Audit Checklist (mandatory before requesting review)

_Run this checklist after all tasks are complete. This is a blocking step — do not request review until every item is verified._

#### Generic Verification
- [ ] For every task marked `[x]`, cite the specific test that verifies it
- [ ] For every new error variant or branch, confirm a test exercises it
- [ ] For every computed value, trace it to where it surfaces (log, return value, persistence)
- [ ] For every test, verify the assertion can actually fail (no `assert!(true)`, no conditionals that always pass)

#### Epic 3-Specific Trust Verification
- [ ] For every retrieval path, confirm a test exercises blob_id trust verification
- [ ] For every query, confirm a test exercises the invalidated/unhealthy rejection path
- [ ] For every "no results" path, confirm the response distinguishes empty vs missing vs stale

#### Story 3.7-Specific Verification
- [ ] Confirm `SymbolRequest`, `CodeSliceRequest`, `BatchRetrievalRequest`, `BatchRetrievalResultItem`, and `GetSymbolsResponse` are defined and exported
- [ ] Confirm `get_symbols()` delegates to `check_request_gate()` before item processing
- [ ] Confirm per-item missing, quarantined, and blocked states are wrapped into item results, not propagated as batch failures
- [ ] Confirm results array preserves input request order
- [ ] Confirm empty batch returns `outcome: Empty` (not error)
- [ ] Confirm batch size limit is enforced at MCP layer
- [ ] Confirm mixed outcomes are reported correctly (valid + missing/blocked/quarantined in the same batch)
- [ ] Confirm one failed item does NOT invalidate other successful items
- [ ] Confirm gate failure blocks entire batch uniformly
- [ ] Confirm `get_symbols` MCP tool is registered in `#[tool_router]`
- [ ] Confirm JSON serialization of batch response includes per-item `outcome`, `trust`, `next_action`
- [ ] Confirm story metadata matches the actual set of modified and newly added source files
- [ ] Confirm full test suite passes (`cargo test --workspace`)

### Project Structure Notes

**Modified / Added Files:**

| File | Changes |
|------|---------|
| `src/domain/retrieval.rs` | Mixed batch request/result types, verified code-slice response, `RetrievalOutcome::Missing` |
| `src/domain/index.rs` | `schemars::JsonSchema` derive for `SymbolKind` |
| `src/domain/mod.rs` | Re-export expanded batch retrieval types |
| `src/application/search.rs` | Mixed-target `get_symbols()`, `get_symbols_ungated()`, shared run/file-record reuse, unit tests |
| `src/application/mod.rs` | `get_symbols()` delegation method on `ApplicationContext` |
| `src/protocol/mcp.rs` | `get_symbols` tool definition, mixed-target parser, legacy `symbols` compatibility, instructions update |
| `tests/retrieval_integration.rs` | Mixed symbol/code-slice batch integration tests |
| `tests/retrieval_conformance.rs` | Mixed batch request/result serialization tests |
| `tests/retrieval_conformance.rs` | Batch type serialization conformance tests |
| `tests/retrieval_integration.rs` | Batch retrieval integration tests |

**Alignment**: All changes follow established patterns from 3.5/3.6. No new modules, no new architectural layers. Batch is purely compositional over existing single-symbol retrieval.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.7] — ACs: batched retrieval with per-item outcomes
- [Source: _bmad-output/planning-artifacts/epics.md#Epic 3 Execution Narrative] — Phase 4 entry gate, ADR-4, batch size limits, per-item outcome model
- [Source: _bmad-output/planning-artifacts/epics.md#Failure Mode Guidance] — Request-fatal vs item-local; one failure must not poison batch
- [Source: _bmad-output/planning-artifacts/epics.md#Adversarial Findings] — MCP context integrity (no repo override per-item)
- [Source: _bmad-output/planning-artifacts/prd.md#FR22] — "Users can retrieve multiple symbols or code slices in one workflow when needed"
- [Source: _bmad-output/planning-artifacts/architecture.md#Retrieval Trust Model] — byte-exact verification per retrieval path
- [Source: _bmad-output/planning-artifacts/architecture.md#Error / Result Semantics] — domain states must be explicit, not collapsed
- [Source: _bmad-output/planning-artifacts/architecture.md#Anti-Patterns] — "generic success: false for repair-required domain outcome"
- [Source: _bmad-output/project-context.md#Epic 3 Retrieval Architecture] — 6 mandatory retrieval rules
- [Source: _bmad-output/implementation-artifacts/3-6-block-or-quarantine-suspect-retrieval.md] — Previous story patterns, NextAction mapping, test conventions, 577 passing tests
- [Source: src/application/search.rs] — get_symbol/get_symbol_ungated patterns, check_request_gate, gate_error helper
- [Source: src/protocol/mcp.rs] — MCP tool registration pattern, parameter parsing, error mapping
- [Source: src/domain/retrieval.rs] — ResultEnvelope, VerifiedSourceResponse, RetrievalOutcome, TrustLevel, NextAction types

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

None — clean implementation with no debugging issues.

### Completion Notes List

- Implemented a mixed batch target model in `src/domain/retrieval.rs`: `SymbolRequest`, `CodeSliceRequest`, `BatchRetrievalRequest`, `BatchRetrievalResultItem`, `BatchRetrievalResponseData`, and `VerifiedCodeSliceResponse`
- Added `RetrievalOutcome::Missing` and preserved explicit absent outcomes for missing symbols and files
- Derived `schemars::JsonSchema` for the batched request model and `SymbolKind`
- Re-exported the expanded batch retrieval types from `src/domain/mod.rs`
- Refactored `get_symbols()` / `get_symbols_ungated()` to share the latest completed run and file-record set across the batch while preserving input order
- Added mixed symbol/code-slice batch verification, explicit missing outcomes, and helper coverage in `src/application/search.rs`
- Updated `ApplicationContext::get_symbols()` to accept `BatchRetrievalRequest`
- Extended the MCP parser and tool docs to support preferred `targets`, legacy `symbols`, mixed symbol/code-slice batches, and byte-range validation
- Added conformance and integration coverage for mixed batch targets and JSON structure
- `cargo test --workspace --quiet` passed with 607 tests

### Change Log

- 2026-03-08: Story 3.7 implementation — batch retrieval with per-item outcomes, request-level gating, mixed outcome support, batch size limit, and MCP tool registration
- 2026-03-08: Senior Developer Review (AI) requested changes; story returned to in-progress with five follow-up items
- 2026-03-08: Addressed review follow-ups by adding mixed symbol/code-slice batching, explicit `Missing` outcomes, `JsonSchema` derives, shared run/file-record reuse, and aligned workflow metadata

### File List

- `src/domain/retrieval.rs` — Added mixed batch request/result types, verified code-slice response support, and `RetrievalOutcome::Missing`
- `src/domain/index.rs` — Added `schemars::JsonSchema` support for `SymbolKind`
- `src/domain/mod.rs` — Re-exported expanded batch retrieval types
- `src/application/search.rs` — Added mixed-target `get_symbols()` / `get_symbols_ungated()` logic, shared run/file-record reuse, and updated unit tests
- `src/application/mod.rs` — Updated `get_symbols()` delegation method on `ApplicationContext`
- `src/protocol/mcp.rs` — Added mixed-target parsing, legacy `symbols` compatibility, and updated `get_symbols` tool instructions
- `tests/retrieval_integration.rs` — Added mixed symbol/code-slice batch integration coverage
- `tests/retrieval_conformance.rs` — Added mixed batch request/result serialization conformance coverage

## Senior Developer Review (AI)

### Reviewer

Codex (GPT-5) — 2026-03-08

### Outcome

Approved after fixes

### Review Notes

- Added a real mixed-target batch contract: `targets` can now carry verified symbol or verified code-slice requests, while legacy `symbols` remains supported for backward compatibility.
- Restored the explicit absent state by mapping missing files and symbols to `RetrievalOutcome::Missing` instead of collapsing them into `Blocked` / `Suspect`.
- Added `schemars::JsonSchema` derives for the batch request surface and `SymbolKind`, so the task record now matches the implementation.
- Refactored the batch path to load the latest completed run and file records once, then dispatch per-item verification without repeated registry reloads.
- Reconciled the story metadata, change log, and file list with the actual implementation scope and touched files.

### Verification

- `cargo test --workspace --quiet`
