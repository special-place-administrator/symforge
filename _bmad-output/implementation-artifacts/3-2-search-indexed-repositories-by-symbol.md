# Story 3.2: Search Indexed Repositories by Symbol

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an AI coding user,
I want to search indexed repositories by symbol,
so that I can navigate to relevant code structures quickly.

**FRs implemented:** FR18, FR23

## Acceptance Criteria

1. **Given** symbol metadata exists for indexed files **When** I search by symbol **Then** Tokenizor returns matching symbol results for the correct project or workspace context **And** symbol results include enough metadata to support further retrieval or navigation (name, kind, file path, line/span) (AC: 1)
2. **Given** symbol extraction is incomplete or unavailable for some files **When** I search by symbol **Then** Tokenizor returns the best valid results available **And** it does not overstate coverage for missing symbol data (AC: 2, coverage transparency)
3. **Given** a repository is invalidated, failed, or has an active mutation in progress **When** I perform a symbol search **Then** the entire request fails with an explicit status-based rejection before any search executes (AC: 3, request-fatal gate — reuses `check_request_gate` from Story 3.1)
4. **Given** a repository has no completed index runs **When** I perform a symbol search **Then** the request fails with an explicit "not indexed" / missing state, not a generic empty result (AC: 4, disambiguation)
5. **Given** a repository has quarantined files **When** symbol search matches content in quarantined files **Then** those files are excluded unconditionally from search results (AC: 5, quarantine exclusion)
6. **Given** a search returns results **When** the response is constructed **Then** every result carries `run_id` and `committed_at_unix_ms` provenance from the originating `FileRecord` (AC: 6, provenance)

## Tasks / Subtasks

### Phase 1: Symbol Search Domain Types

- [x] Task 1.1: Define `SymbolResultItem` struct in `src/domain/retrieval.rs` (AC: 1, 6)
  - [x] 1.1.1: Create `SymbolResultItem` with fields: `symbol_name: String`, `symbol_kind: SymbolKind`, `relative_path: String`, `language: LanguageId`, `line_range: (u32, u32)`, `byte_range: (u32, u32)`, `depth: u32`, `provenance: Provenance`
  - [x] 1.1.2: Derive `Debug, Clone, Serialize, Deserialize, PartialEq` (match `SearchResultItem` derives)
  - [x] 1.1.3: Re-export from `src/domain/mod.rs`

- [x] Task 1.2: Define `SymbolCoverage` struct in `src/domain/retrieval.rs` (AC: 2)
  - [x] 1.2.1: Create `SymbolCoverage` with fields: `files_searched: u32` (files with symbol data that were searched), `files_without_symbols: u32` (files where extraction produced nothing or language unsupported), `files_skipped_quarantined: u32`
  - [x] 1.2.2: Derive `Debug, Clone, Serialize, Deserialize, PartialEq`

- [x] Task 1.3: Define `SymbolSearchResponse` wrapper in `src/domain/retrieval.rs` (AC: 1, 2)
  - [x] 1.3.1: Create `SymbolSearchResponse` with fields: `matches: Vec<SymbolResultItem>`, `coverage: SymbolCoverage`
  - [x] 1.3.2: Derive `Debug, Clone, Serialize, Deserialize, PartialEq`
  - [x] 1.3.3: Re-export from `src/domain/mod.rs`

### Phase 2: Symbol Search Implementation

- [x] Task 2.1: Implement `search_symbols()` in `src/application/search.rs` (AC: 1, 2, 3, 4, 5, 6)
  - [x] 2.1.1: Public function signature: `pub fn search_symbols(repo_id: &str, query: &str, kind_filter: Option<SymbolKind>, persistence: &RegistryPersistence, run_manager: &RunManager) -> Result<ResultEnvelope<SymbolSearchResponse>>`
  - [x] 2.1.2: Reject empty query with `TokenizorError::InvalidArgument` (match text search behavior)
  - [x] 2.1.3: Call `check_request_gate()` (reuse from 3.1 — NO changes needed)
  - [x] 2.1.4: Delegate to `search_symbols_ungated()` after gate passes

- [x] Task 2.2: Implement `search_symbols_ungated()` (AC: 1, 2, 5, 6)
  - [x] 2.2.1: Get latest completed run via `persistence.get_latest_completed_run(repo_id)`
  - [x] 2.2.2: If `None`, return `ResultEnvelope { outcome: NotIndexed, trust: Verified, provenance: None, data: None }` (defense-in-depth — gate should catch first)
  - [x] 2.2.3: Build `Provenance` from the run's metadata
  - [x] 2.2.4: Get file records via `persistence.get_file_records(run_id)`
  - [x] 2.2.5: Iterate file records. For each file:
    - Skip `Quarantined` files (increment `files_skipped_quarantined` counter)
    - For files with empty `symbols` vec: increment `files_without_symbols` counter, continue
    - For files with symbols: increment `files_searched` counter
    - For each symbol in `record.symbols`: match query using **case-insensitive substring** (`symbol.name.to_lowercase().contains(&query.to_lowercase())`)
    - If `kind_filter` is `Some(kind)`, additionally filter by `symbol.kind == kind`
    - Build `SymbolResultItem` for each match with per-file provenance (`record.run_id`, `record.committed_at_unix_ms`, `record.repo_id`)
  - [x] 2.2.6: Build `SymbolCoverage` from counters
  - [x] 2.2.7: If matches empty AND index has symbol data → `ResultEnvelope { outcome: Empty, trust: Verified, provenance: Some(run_provenance), data: Some(SymbolSearchResponse { matches: vec![], coverage }) }`. **Deliberate deviation**: Empty results still carry `data: Some(...)` with coverage metadata so callers know coverage was partial. Document with code comment.
  - [x] 2.2.8: If matches non-empty → `ResultEnvelope { outcome: Success, trust: Verified, provenance: Some(run_provenance), data: Some(SymbolSearchResponse { matches, coverage }) }`

- [x] Task 2.3: Wire search into application layer (AC: 1)
  - [x] 2.3.1: Add `search_symbols` method to `ApplicationContext` in `src/application/mod.rs`
  - [x] 2.3.2: Delegate to `search::search_symbols()` with `self.run_manager.persistence()` and `&self.run_manager`
  - [x] 2.3.3: Note: does NOT need `blob_store` — symbol search reads metadata only, no CAS I/O

### Phase 3: Unit Tests

- [x] Task 3.1: Unit tests for symbol search (AC: 1, 2, 3, 4, 5, 6)
  - [x] 3.1.1: `test_search_symbols_returns_matching_results` — happy path with symbol matches
  - [x] 3.1.2: `test_search_symbols_returns_empty_for_no_matches` — explicit Empty, not generic; includes coverage metadata
  - [x] 3.1.3: `test_search_symbols_case_insensitive` — "hashmap" matches "HashMap"
  - [x] 3.1.4: `test_search_symbols_filters_by_kind` — kind_filter narrows results
  - [x] 3.1.5: `test_search_symbols_rejects_invalidated_repo` — request-fatal gate
  - [x] 3.1.6: `test_search_symbols_rejects_failed_repo` — request-fatal gate
  - [x] 3.1.7: `test_search_symbols_rejects_active_mutation` — request-fatal gate (via RunManager::has_active_run)
  - [x] 3.1.8: `test_search_symbols_rejects_never_indexed_repo` — no runs exist at all → NeverIndexed
  - [x] 3.1.9: `test_search_symbols_rejects_no_successful_runs` — runs exist but all Failed/Interrupted → NoSuccessfulRuns with latest_status
  - [x] 3.1.10: `test_search_symbols_excludes_quarantined_files` — quarantine exclusion
  - [x] 3.1.11: `test_search_symbols_includes_provenance_metadata` — run_id + committed_at_unix_ms on every result
  - [x] 3.1.12: `test_search_symbols_allows_degraded_repo` — NOT request-fatal, returns results with provenance
  - [x] 3.1.13: `test_search_symbols_rejects_empty_query` — InvalidArgument for empty string
  - [x] 3.1.14: `test_search_symbols_scopes_to_repo_context` — no cross-repo leakage
  - [x] 3.1.15: `test_search_symbols_coverage_reports_files_without_symbols` — coverage metadata counts correctly
  - [x] 3.1.16: `test_search_symbols_skips_files_with_empty_symbols` — files with `EmptySymbols` outcome or empty symbols vec are counted as without symbols
  - [x] 3.1.17: `test_search_symbols_latency_within_bounds` — sanity check for p50 ≤ 100ms target (not full benchmark)
  - [x] 3.1.18: `test_search_symbols_skips_failed_files_and_counts_coverage` — failed files are excluded from matches and counted in `files_without_symbols`

### Phase 4: Integration Tests

- [x] Task 4.1: Integration tests in `tests/retrieval_integration.rs` (AC: 1, 2, 3, 5)
  - [x] 4.1.1: End-to-end: index a fixture repo with Rust/Python files, then symbol search, verify results include name/kind/path/line_range
  - [x] 4.1.2: End-to-end: search for non-existent symbol → verify explicit Empty with coverage metadata
  - [x] 4.1.3: End-to-end: search against invalidated repo, verify rejection
  - [x] 4.1.4: End-to-end: search with quarantined files, verify exclusion
  - [x] 4.1.5: End-to-end: search with no completed runs, verify explicit never-indexed rejection

### Phase 5: Extend Contract Conformance Tests

- [x] Task 5.1: Extend `tests/retrieval_conformance.rs` (AC: 1, 6)
  - [x] 5.1.1: Conformance test: `SymbolResultItem` is constructable and serializable
  - [x] 5.1.2: Conformance test: `SymbolSearchResponse` is constructable and serializable
  - [x] 5.1.3: Conformance test: `SymbolCoverage` is constructable and serializable
  - [x] 5.1.4: Conformance test: `SymbolResultItem` includes all required provenance fields

## Dev Notes

### Critical Design Decision: No Blob I/O Required

**Symbol search does NOT need CAS blob reads or blob integrity verification.** This is a key performance advantage over text search.

Rationale: Text search reads raw blob bytes from CAS at query time and must re-hash to verify integrity. Symbol search reads pre-extracted symbol metadata from `FileRecord.symbols`, which was stored in the registry during indexing. The symbols were extracted from verified blobs at index time and persisted durably. Re-verifying blobs at query time would be redundant and defeat the purpose of pre-extracted metadata.

This means the `search_symbols()` function does NOT take a `blob_store: &dyn BlobStore` parameter.

Rule 1 ("Never return blob content without verifying blob_id matches") does not apply because symbol search returns symbol metadata, not blob content. Trust is inherited from the run's completion status, conveyed through provenance metadata.

### Coverage Transparency (AC 2)

AC 2 requires: "does not overstate coverage for missing symbol data."

Symbol extraction is incomplete when:
- File language is not supported for symbol extraction (only Rust, Python, JavaScript, TypeScript, Go, Java have extractors)
- File processed but no symbols found (`EmptySymbols` outcome or empty symbols vec)
- File was quarantined (excluded entirely)

**Design**: Include `SymbolCoverage` metadata in the response. On Empty results, `data` is `Some(SymbolSearchResponse { matches: vec![], coverage })` — a deliberate deviation from text search where Empty → data: None. Rationale: coverage transparency is mandated by AC 2 and is meaningful even (especially) when there are no matches.

### Contract Requirements (Shared Read-Side Contract — Phase 0, established in Story 3.1)

The shared contract types in `src/domain/retrieval.rs` are ALREADY IMPLEMENTED. Story 3.2 extends them with `SymbolResultItem`, `SymbolCoverage`, and `SymbolSearchResponse`. No changes to existing contract types.

- **Result envelope**: `ResultEnvelope<SymbolSearchResponse>` wraps outcome + trust + provenance + data
- **Request-level gating**: Reuse `check_request_gate()` from Story 3.1 — NO changes needed
- **Result-state disambiguation**: Empty vs NotIndexed vs Success (no Stale — that's for targeted retrieval)
- **Quarantined-file exclusion**: Files with `PersistedFileOutcome::Quarantined` skipped unconditionally
- **Active-context resolution**: Scoped to `repo_id` parameter

### Request-Fatal vs Item-Local Rules

**Identical to Story 3.1** — reuse `check_request_gate()` unchanged.

**Request-fatal (entire request fails before any search):**
- No active context / unknown `repo_id` → `get_repository()` returns `None`
- `RepositoryStatus::Invalidated` — trust explicitly revoked
- `RepositoryStatus::Failed` — index failed
- Active mutation in progress — `RunManager::has_active_run(repo_id)` or `get_active_run_id(repo_id)`
- No completed (`Succeeded`) runs — `get_latest_completed_run()` returns `None`, disambiguated as `NeverIndexed` or `NoSuccessfulRuns`

**NOT request-fatal:**
- `RepositoryStatus::Degraded` — soft warning, returns results with provenance (same tension with project-context.md Rule 2 documented in 3.1)

**Item-local:**
- Individual file quarantined → excluded from results silently
- File has no symbols (EmptySymbols or unsupported language) → skipped, counted in coverage

### Symbol Search Result Schema

```rust
/// A single symbol search match
pub struct SymbolResultItem {
    pub symbol_name: String,         // Symbol identifier (e.g., "main", "HashMap", "impl Default for MyStruct")
    pub symbol_kind: SymbolKind,     // Function, Method, Class, Struct, Enum, Trait, Impl, etc.
    pub relative_path: String,       // File path relative to repo root
    pub language: LanguageId,        // Language of the matched file
    pub line_range: (u32, u32),      // (start_line, end_line) — 0-based row indices from tree-sitter
    pub byte_range: (u32, u32),      // (start_byte, end_byte) for byte-exact positioning
    pub depth: u32,                  // Nesting depth (0 = top-level, 1 = inside class/impl, etc.)
    pub provenance: Provenance,      // run_id + committed_at_unix_ms + repo_id
}

/// Coverage metadata for symbol search transparency
pub struct SymbolCoverage {
    pub files_searched: u32,              // Files with symbol data that were searched
    pub files_without_symbols: u32,       // Files where extraction produced nothing or language unsupported
    pub files_skipped_quarantined: u32,   // Quarantined files excluded
}

/// Wrapper for symbol search results with coverage
pub struct SymbolSearchResponse {
    pub matches: Vec<SymbolResultItem>,   // Matching symbols
    pub coverage: SymbolCoverage,         // Coverage transparency
}
```

### Search Matching Strategy

- **Default**: Case-insensitive substring match on `symbol.name`
- **Kind filter**: Optional `SymbolKind` filter narrows results
- **Rationale**: Case-insensitive is more useful for AI coding workflows where casing may not be known. Substring match enables partial name search (e.g., "hash" finds "HashMap", "HashSet")
- **No regex/fuzzy**: Keep simple for 3.2. Advanced matching can be added later if needed.

### Latency Requirements

- **`search_symbols`**: p50 ≤ 100 ms, p95 ≤ 300 ms on representative medium-to-large repositories (warm local index) [Source: epics.md#NFR2]
- Symbol search should be inherently faster than text search because it reads only in-memory metadata (no CAS I/O, no blob hash verification). The 100ms p50 target is tighter than text search's 150ms precisely because of this.
- **"Done" criteria**: A basic latency sanity check against the NFR target is REQUIRED. Not the full benchmark suite, but a test-fixture assertion that the operation completes within a reasonable bound.

### Testing Requirements

- **Naming**: `test_verb_condition` (e.g., `test_search_symbols_returns_matching_results`)
- **Fakes**: Hand-written fakes inside `#[cfg(test)] mod tests`. No mock crates.
- **Assertions**: Plain `assert!`, `assert_eq!`. No assertion crates.
- **Test type**: `#[test]` by default. `#[tokio::test]` only for async fn tests.
- **Unit tests**: `#[cfg(test)]` blocks inside `src/application/search.rs`.
- **Integration tests**: Extend `tests/retrieval_integration.rs`.
- **Call verification**: `AtomicUsize` counters on fakes to verify interaction counts (if needed).
- **Fixture**: Use existing test setup patterns from Story 3.1. Extend or create fixture repos with multiple languages.
- **Setup**: Follow existing `setup_test_env()` pattern returning `(TempDir, Arc<RunManager>, TempDir, Arc<dyn BlobStore>)` — even though symbol search doesn't use blob_store directly, integration tests need it for the indexing pipeline.
- **Contract conformance**: Story must pass the evolving contract-conformance test skeleton before moving to `done`.

### Epic 3 Retrieval Architecture Rules (Mandatory)

From project-context.md — non-negotiable for ALL Epic 3 code:

1. **Rule 1 (blob verification)**: Does NOT apply to symbol search — symbol search returns metadata, not blob content. See "Critical Design Decision" section above.
2. **Rule 2 (repo status check)**: Enforced via `check_request_gate()`. Degraded passes with provenance (documented tension with project-context.md).
3. **Rule 3 (provenance)**: Every `SymbolResultItem` carries `provenance` (run_id + committed_at_unix_ms + repo_id).
4. **Rule 4 (disambiguation)**: Empty (no matches) vs NotIndexed (no completed runs) are distinct outcomes.
5. **Rule 5 (early gating)**: Gate check runs FIRST, before any file record iteration.
6. **Rule 6 (quarantine exclusion)**: Quarantined files excluded unconditionally from symbol search results.

### Scope Boundaries — What Story 3.2 Does NOT Cover

- MCP tool exposure (Story 3.4 — `search_symbols` tool)
- Text search (Story 3.1 — done)
- File/repo outlines (Story 3.3)
- Verified source retrieval (Story 3.5)
- Blocking/quarantine behavior for targeted retrieval (Story 3.6)
- Batched retrieval (Story 3.7)
- Advanced search modes (regex, fuzzy matching, scope-qualified search)
- Cross-repository or cross-workspace symbol search
- Live filesystem change detection or symbol re-extraction
- Write-side operations (re-indexing, repair, state mutation)
- Freshness policy enforcement (deferred to Epic 4/5)

### Existing Read-Side Interfaces from Epic 2 (Verified in Story 3.1)

Symbol search uses a SUBSET of the interfaces text search uses — specifically, it does NOT need `BlobStore::read_bytes`:

| Interface | Used? | Actual Signature | Quirks |
|-----------|-------|-----------------|--------|
| Repo status | YES | `RegistryPersistence::get_repository(&self, repo_id: &str) -> Result<Option<Repository>>` | Returns `None` for unknown repo_id. `Repository.status` has `Pending`, `Ready`, `Degraded`, `Failed`, `Invalidated`. |
| Active mutation | YES | `RunManager::has_active_run(&self, repo_id: &str) -> bool` / `get_active_run_id(&self, repo_id: &str) -> Option<String>` | In-memory HashMap check. |
| Latest completed run | YES | `RegistryPersistence::get_latest_completed_run(&self, repo_id: &str) -> Result<Option<IndexRun>>` | Filters ONLY by `IndexRunStatus::Succeeded`. Returns `None` if no Succeeded runs. |
| File records by run | YES | `RegistryPersistence::get_file_records(&self, run_id: &str) -> Result<Vec<FileRecord>>` | Returns empty `Vec` for unknown run_id. `FileRecord.symbols: Vec<SymbolRecord>` is the symbol data source. |
| Runs by repo | YES (gate) | `RegistryPersistence::get_runs_by_repo(&self, repo_id: &str) -> Result<Vec<IndexRun>>` | Used by gate to disambiguate NeverIndexed vs NoSuccessfulRuns. |
| Blob lookup | **NO** | `BlobStore::read_bytes` | NOT needed — symbol search reads metadata, not blob content. |

### SymbolRecord Source Data (from `src/domain/index.rs`)

Symbols are already extracted during indexing and stored in `FileRecord.symbols`:

```rust
pub struct SymbolRecord {
    pub name: String,              // "main", "HashMap", "impl Default for MyStruct"
    pub kind: SymbolKind,          // Function, Method, Class, Struct, Enum, etc.
    pub depth: u32,                // 0 = top-level, 1+ = nested
    pub sort_order: u32,           // Document order
    pub byte_range: (u32, u32),    // (start_byte, end_byte)
    pub line_range: (u32, u32),    // (start_line, end_line) — 0-based rows
}

pub enum SymbolKind {  // 13 variants, #[serde(rename_all = "snake_case")]
    Function, Method, Class, Struct, Enum, Interface, Module,
    Constant, Variable, Type, Trait, Impl, Other,
}
```

**Languages with symbol extraction**: Rust, Python, JavaScript, TypeScript, Go, Java (in `src/parsing/languages/`). Other `LanguageId` variants return empty symbol vecs.

### PersistedFileOutcome Handling for Symbol Search

```
Committed         → search symbols (may have empty vec if language unsupported at parsing level)
EmptySymbols       → skip (count as files_without_symbols)
Failed { error }   → skip (count as files_without_symbols)
Quarantined { .. } → skip (count as files_skipped_quarantined)
```

### Build Order (Mandatory — domain first, wiring last)

1. Domain types in `src/domain/retrieval.rs` — `SymbolResultItem`, `SymbolCoverage`, `SymbolSearchResponse`
2. Re-exports in `src/domain/mod.rs`
3. Search function in `src/application/search.rs` — `search_symbols()` + `search_symbols_ungated()`
4. Application wiring in `src/application/mod.rs` — `ApplicationContext::search_symbols()`
5. Unit tests in `src/application/search.rs` — 17 tests covering all ACs and gate conditions
6. Integration tests in `tests/retrieval_integration.rs` — 5 end-to-end tests
7. Conformance tests in `tests/retrieval_conformance.rs` — 4 new type conformance tests

### Architecture Compliance

- **Layer**: Search logic in `application/`. Domain types in `domain/`. MCP exposure deferred to Story 3.4 (`protocol/`).
- **Persistence model**: All reads via `RegistryPersistence`. No SpacetimeDB reads.
- **Error handling**: Reuse `TokenizorError::RequestGated { gate_error: String }` from Story 3.1. No new error variants needed.
- **No Mutex across .await**: Not applicable — symbol search is synchronous (no async I/O needed since no blob reads).
- **No mock crates**: Hand-written fakes with `AtomicUsize` counters.
- **No assertion crates**: Plain `assert!`/`assert_eq!`.

### Previous Story Intelligence (Story 3.1)

**Patterns established in Story 3.1 that MUST be followed:**
- `check_request_gate()` function is the single gate entry point — reuse as-is
- Gate returns `TokenizorError::RequestGated` which maps to MCP `invalid_params`
- `Degraded` repos pass gate with warning log
- Empty query rejection with `InvalidArgument`
- `ResultEnvelope` pattern with `outcome`, `trust`, `provenance`, `data`
- Defense-in-depth `NotIndexed` check inside ungated function (in case gate is bypassed by direct call)

**Dev agent failure modes from Epic 2 retrospective (guard against):**
1. **No-op/sentinel tests**: `assert!(true)` or conditional logic that silently passes. Every test assertion MUST be able to fail.
2. **`is_systemic()` misclassification**: No new `TokenizorError` variants in this story, but verify if touched.
3. **Data computed but dropped**: Ensure coverage counters are wired to the response, not computed and discarded.
4. **Missing tests for new code paths**: Every coverage counter (files_searched, files_without_symbols, files_skipped_quarantined) needs a test.

**Story 3.1 code review findings that apply:**
- Preserve line content and aligned offsets — for symbols, ensure `line_range` and `byte_range` come directly from `SymbolRecord` (don't recompute)
- Reject empty queries — symbol search must also reject `""` with `InvalidArgument`
- Integration tests should use run-status polling (not fixed sleeps) — follow `wait_for_run_success()` pattern

**Story 3.1 implementation stats:** 36 new tests (24 unit + 4 integration + 8 conformance). Total test count after 3.1: 405. Expect ~26 new tests from 3.2 (17 unit + 5 integration + 4 conformance).

### Self-Audit Checklist (mandatory before requesting review)

_Run this checklist after all tasks are complete. This is a blocking step — do not request review until every item is verified._

#### Generic Verification
- [x] For every task marked `[x]`, cite the specific test that verifies it
- [x] For every new error variant or branch, confirm a test exercises it
- [x] For every computed value, trace it to where it surfaces (log, return value, persistence)
- [x] For every test, verify the assertion can actually fail (no `assert!(true)`, no conditionals that always pass)

#### Epic 3-Specific Trust Verification
- [x] For every search result, confirm provenance metadata (run_id, committed_at_unix_ms) is populated
- [x] For every request gate condition, confirm a test exercises the rejection path
- [x] For every "no results" path, confirm the response distinguishes empty vs missing vs stale
- [x] For quarantined files, confirm they are excluded from search results (not returned with degraded trust)
- [x] For the request gate, confirm it runs BEFORE any file record iteration
- [x] Confirm the latency sanity check test exists and asserts a reasonable bound
- [x] Confirm contract-conformance tests exist and pass for new types (SymbolResultItem, SymbolCoverage, SymbolSearchResponse)

#### Story 3.2-Specific Verification
- [x] Confirm NO blob reads occur during symbol search (no `BlobStore::read_bytes` calls)
- [x] Confirm coverage metadata is populated on both Success AND Empty outcomes
- [x] Confirm case-insensitive matching works (e.g., "hashmap" matches "HashMap")
- [x] Confirm kind filter narrows results correctly when provided
- [x] Confirm files with `EmptySymbols` outcome are counted in `files_without_symbols`, not `files_searched`
- [x] Confirm `depth`, `line_range`, and `byte_range` fields come directly from `SymbolRecord` without recomputation

### Project Structure Notes

- Extended: `src/domain/retrieval.rs` — add `SymbolResultItem`, `SymbolCoverage`, `SymbolSearchResponse`
- Extended: `src/domain/mod.rs` — re-export new types
- Extended: `src/application/search.rs` — add `search_symbols()` + `search_symbols_ungated()`
- Extended: `src/application/mod.rs` — add `search_symbols()` method to `ApplicationContext`
- Extended: `src/application/run_manager.rs` — add `get_active_run_id()` for explicit request-gate rejection
- Extended: `src/error.rs` — add `RequestGated` error variant used by shared retrieval gating
- Extended: `src/protocol/mcp.rs` — map `RequestGated` to MCP `invalid_params`
- Extended: `tests/retrieval_integration.rs` — add symbol search integration tests
- Extended: `tests/retrieval_conformance.rs` — add type conformance tests for new types
- NO changes to: `src/storage/` (reads use existing `RegistryPersistence`)

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story-3.2] — User story, ACs, BDD scenarios
- [Source: _bmad-output/planning-artifacts/epics.md#Epic-3-Execution-Narrative] — Phase model, ADRs, failure modes, gating rules
- [Source: _bmad-output/planning-artifacts/epics.md#Contract-Gaps] — Symbol result payload and coverage semantics requirements
- [Source: _bmad-output/planning-artifacts/epics.md#NFR2] — search_symbols latency: p50<=100ms, p95<=300ms
- [Source: _bmad-output/project-context.md#Epic-3-Retrieval-Architecture] — 6 mandatory retrieval rules
- [Source: _bmad-output/planning-artifacts/architecture.md#Retrieval-Trust-Model] — Trust verification model
- [Source: _bmad-output/planning-artifacts/architecture.md#Result-Format-Rules] — Result format constraints
- [Source: _bmad-output/implementation-artifacts/3-1-search-indexed-repositories-by-text.md] — Previous story patterns, code review findings, implementation notes
- [Source: src/domain/index.rs] — SymbolRecord, SymbolKind, FileRecord, PersistedFileOutcome definitions
- [Source: src/domain/retrieval.rs] — Existing contract types (ResultEnvelope, RetrievalOutcome, TrustLevel, Provenance, RequestGateError)
- [Source: src/application/search.rs] — Existing text search implementation and check_request_gate()
- [Source: src/parsing/languages/] — Language-specific symbol extractors (Rust, Python, JS, TS, Go, Java)

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

No debug issues encountered. All tests passed on first run.

### Completion Notes List

- Phase 1: Added `SymbolResultItem`, `SymbolCoverage`, `SymbolSearchResponse` to `src/domain/retrieval.rs` with correct derives (`Debug, Clone, Serialize, Deserialize, PartialEq, Eq`). Re-exported from `src/domain/mod.rs`.
- Phase 2: Implemented `search_symbols()` (public, gated) and `search_symbols_ungated()` (private, defense-in-depth) in `src/application/search.rs`. Case-insensitive substring matching on symbol name with optional `SymbolKind` filter. No blob I/O required — reads pre-extracted metadata only. Coverage metadata included on both Success and Empty outcomes (deliberate deviation from text search). Failed and `EmptySymbols` file outcomes are excluded from matches and counted in `files_without_symbols`.
- Phase 2: Wired `search_symbols()` method into `ApplicationContext` in `src/application/mod.rs`.
- Phase 2: Added `RunManager::get_active_run_id()`, `TokenizorError::RequestGated`, and MCP error mapping support required for request-level gating.
- Phase 3: Added 18 unit tests covering all ACs: happy path, empty results, case insensitive, kind filter, gate rejections (invalidated, failed, active mutation, never indexed, no successful runs), quarantine exclusion, provenance, degraded repo, empty query rejection, repo scoping, coverage reporting, empty symbols handling, failed-file exclusion, latency bounds.
- Phase 4: Added 5 integration tests: end-to-end index-then-search, empty search with coverage, invalidated repo rejection, quarantine exclusion, never-indexed rejection. Review hardening added explicit assertions for symbol kind, path, line range, success outcome, and quarantine coverage.
- Phase 5: Added 4 conformance tests: `SymbolResultItem` constructability/serialization, `SymbolSearchResponse` constructability/serialization, `SymbolCoverage` constructability/serialization, provenance field verification.
- Total: 27 new tests (18 unit + 5 integration + 4 conformance). Test count: 405 → 432.
- Reused `check_request_gate()` unchanged from Story 3.1.
- Added `RequestGated` in `src/error.rs`, mapped it in `src/protocol/mcp.rs`, and kept `src/storage/` unchanged.

### Change Log

- 2026-03-08: Implemented Story 3.2 — symbol search with coverage transparency, request gating, quarantine exclusion, and provenance metadata. 27 new tests added.
- 2026-03-08: Senior developer review fixes applied — failed-file exclusion enforced for symbol search, weak integration assertions hardened, and story metadata synced to the actual changed files.

### File List

- `src/domain/retrieval.rs` — Added `SymbolResultItem`, `SymbolCoverage`, `SymbolSearchResponse` structs; added `SymbolKind` import
- `src/domain/mod.rs` — Added re-exports for `SymbolCoverage`, `SymbolResultItem`, `SymbolSearchResponse`
- `src/application/search.rs` — Added `search_symbols()` and `search_symbols_ungated()` functions + 18 unit tests
- `src/application/mod.rs` — Added `search_symbols()` method to `ApplicationContext`; added `SymbolKind`, `SymbolSearchResponse` imports
- `src/application/run_manager.rs` — Added `run_id` tracking to `ActiveRun` and exposed `get_active_run_id()` for request-gate errors
- `src/error.rs` — Added `RequestGated` error variant and marked it non-systemic
- `src/protocol/mcp.rs` — Mapped `RequestGated` to MCP `invalid_params`
- `tests/retrieval_integration.rs` — Added 5 symbol search integration tests
- `tests/retrieval_conformance.rs` — Added 4 symbol search type conformance tests

## Senior Developer Review (AI)

### Reviewer

Sir on 2026-03-08

### Findings Fixed

- Enforced the story's failed-file contract in `search_symbols()` so `PersistedFileOutcome::Failed` files are excluded from matches and counted in coverage.
- Hardened the symbol-search integration tests so the happy path now proves symbol name/kind/path/line-range metadata and the quarantine test now fails if success/data/coverage regress.
- Synced the story's project structure notes, completion notes, change log, and file list to the actual implementation changes (`src/application/run_manager.rs`, `src/error.rs`, `src/protocol/mcp.rs` were all touched).

### Verification

- `cargo test search_symbols -- --nocapture`
- `cargo test --test retrieval_integration -- --nocapture`
- `cargo test --test retrieval_conformance -- --nocapture`

### Outcome

- Changes requested during review were fixed automatically.
