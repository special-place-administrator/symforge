# Story 3.3: Retrieve File and Repository Outlines

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an AI coding user,
I want to retrieve structural outlines for files and repositories,
so that I can understand code organization quickly before reading raw files.

**FRs implemented:** FR19, FR20, FR23

## Acceptance Criteria

1. **Given** indexed file and repository structure metadata exists **When** I request a file outline **Then** Tokenizor returns the symbol tree for the requested file in the active context **And** the response includes provenance metadata (run_id + committed_at_unix_ms) (AC: 1)
2. **Given** indexed file and repository structure metadata exists **When** I request a repository outline **Then** Tokenizor returns the structural overview of all files in the repository **And** the response includes coverage metadata distinguishing files with symbols, without symbols, quarantined, and failed (AC: 2)
3. **Given** a requested file exists in the index but has no extracted symbols **When** I request a file outline **Then** Tokenizor returns a valid response with an empty symbol list **And** the response distinguishes this from a missing/not-found file (AC: 3, missing-vs-empty disambiguation)
4. **Given** a requested file is not found in the index **When** I request a file outline **Then** Tokenizor returns an explicit not-found error **And** it does not silently fall back to unrelated scope (AC: 4)
5. **Given** a repository is invalidated, failed, or has an active mutation in progress **When** I request a file or repository outline **Then** the entire request fails with an explicit status-based rejection before any outline processing executes (AC: 5, request-fatal gate — reuses `check_request_gate` from Story 3.1)
6. **Given** a repository has no completed index runs **When** I request a file or repository outline **Then** the request fails with an explicit "not indexed" / missing state, not a generic empty result (AC: 6, disambiguation)
7. **Given** a requested file is quarantined **When** I request a file outline **Then** Tokenizor returns the result with `outcome: Quarantined` and `trust: Quarantined` **And** the response makes the quarantine state explicit for targeted diagnosis (AC: 7, quarantine transparency for targeted requests)
8. **Given** a repository outline is requested **When** the repository has quarantined files **Then** those files appear in the listing with their quarantine status **And** the coverage metadata reports them separately (AC: 8, quarantine visibility in repo outline)
9. **Given** an outline request returns results **When** the response is constructed **Then** every file outline carries `run_id` and `committed_at_unix_ms` provenance from the originating `FileRecord` (AC: 9, provenance)

## Tasks / Subtasks

### Phase 1: Outline Domain Types

- [x] Task 1.1: Define `OutlineSymbol` struct in `src/domain/retrieval.rs` (AC: 1, 9)
  - [x] 1.1.1: Create `OutlineSymbol` with fields: `name: String`, `kind: SymbolKind`, `line_range: (u32, u32)`, `byte_range: (u32, u32)`, `depth: u32`, `sort_order: u32`
  - [x] 1.1.2: Derive `Debug, Clone, Serialize, Deserialize, PartialEq, Eq` (match existing type derives)

- [x] Task 1.2: Define `FileOutlineResponse` struct in `src/domain/retrieval.rs` (AC: 1, 3, 9)
  - [x] 1.2.1: Create `FileOutlineResponse` with fields: `relative_path: String`, `language: LanguageId`, `byte_len: u64`, `symbols: Vec<OutlineSymbol>`, `has_symbol_support: bool`
  - [x] 1.2.2: `has_symbol_support` distinguishes "language not supported for symbol extraction" (false) from "supported but no symbols found" (true with empty vec). This satisfies AC 3.
  - [x] 1.2.3: Derive `Debug, Clone, Serialize, Deserialize, PartialEq, Eq`

- [x] Task 1.3: Define `FileOutcomeStatus` enum in `src/domain/retrieval.rs` (AC: 2, 8)
  - [x] 1.3.1: Create `FileOutcomeStatus` with variants: `Committed`, `EmptySymbols`, `Failed`, `Quarantined`
  - [x] 1.3.2: Derive `Debug, Clone, Serialize, Deserialize, PartialEq, Eq` with `#[serde(rename_all = "snake_case")]`
  - [x] 1.3.3: Implement `From<&PersistedFileOutcome> for FileOutcomeStatus` to simplify conversion (strip internal error/reason details)

- [x] Task 1.4: Define `RepoOutlineEntry` struct in `src/domain/retrieval.rs` (AC: 2, 8)
  - [x] 1.4.1: Create `RepoOutlineEntry` with fields: `relative_path: String`, `language: LanguageId`, `byte_len: u64`, `symbol_count: u32`, `status: FileOutcomeStatus`
  - [x] 1.4.2: Derive `Debug, Clone, Serialize, Deserialize, PartialEq, Eq`

- [x] Task 1.5: Define `RepoOutlineCoverage` struct in `src/domain/retrieval.rs` (AC: 2, 8)
  - [x] 1.5.1: Create `RepoOutlineCoverage` with fields: `total_files: u32`, `files_with_symbols: u32`, `files_without_symbols: u32`, `files_quarantined: u32`, `files_failed: u32`
  - [x] 1.5.2: Derive `Debug, Clone, Serialize, Deserialize, PartialEq, Eq`

- [x] Task 1.6: Define `RepoOutlineResponse` struct in `src/domain/retrieval.rs` (AC: 2, 8)
  - [x] 1.6.1: Create `RepoOutlineResponse` with fields: `files: Vec<RepoOutlineEntry>`, `coverage: RepoOutlineCoverage`
  - [x] 1.6.2: Derive `Debug, Clone, Serialize, Deserialize, PartialEq, Eq`

- [x] Task 1.7: Re-export new types from `src/domain/mod.rs` (AC: 1, 2)
  - [x] 1.7.1: Add `FileOutcomeStatus`, `FileOutlineResponse`, `OutlineSymbol`, `RepoOutlineCoverage`, `RepoOutlineEntry`, `RepoOutlineResponse` to `pub use retrieval::{...}`

### Phase 2: Outline Implementation

- [x] Task 2.1: Implement `get_file_outline()` in `src/application/search.rs` (AC: 1, 3, 4, 5, 6, 7, 9)
  - [x] 2.1.1: Public function signature: `pub fn get_file_outline(repo_id: &str, relative_path: &str, persistence: &RegistryPersistence, run_manager: &RunManager) -> Result<ResultEnvelope<FileOutlineResponse>>`
  - [x] 2.1.2: Call `check_request_gate()` (reuse from 3.1 — NO changes needed)
  - [x] 2.1.3: Delegate to `get_file_outline_ungated()` after gate passes

- [x] Task 2.2: Implement `get_file_outline_ungated()` (AC: 1, 3, 4, 7, 9)
  - [x] 2.2.1: Get latest completed run via `persistence.get_latest_completed_run(repo_id)`
  - [x] 2.2.2: If `None`, return `ResultEnvelope { outcome: NotIndexed, trust: Verified, provenance: None, data: None }` (defense-in-depth — gate should catch first)
  - [x] 2.2.3: Build `Provenance` from the run's metadata
  - [x] 2.2.4: Get file records via `persistence.get_file_records(run_id)`
  - [x] 2.2.5: Find the file record matching `relative_path` (case-sensitive exact match)
  - [x] 2.2.6: If no matching file record found → return `Err(TokenizorError::InvalidArgument(format!("file not found in index: {relative_path}")))` (AC 4)
  - [x] 2.2.7: If file is quarantined → return `ResultEnvelope { outcome: Quarantined, trust: Quarantined, provenance: Some(provenance), data: None }` (AC 7 — targeted requests surface quarantine state, unlike search which silently excludes)
  - [x] 2.2.8: Determine `has_symbol_support` from `FileRecord`: `true` if `outcome` is `Committed` and the file's `LanguageId` has a symbol extractor (Rust, Python, JavaScript, TypeScript, Go, Java), `false` otherwise
  - [x] 2.2.9: Build `Vec<OutlineSymbol>` from `record.symbols`, preserving `sort_order` (document order) — copy fields directly from `SymbolRecord`
  - [x] 2.2.10: Build `FileOutlineResponse` with `relative_path`, `language`, `byte_len`, `symbols`, `has_symbol_support`
  - [x] 2.2.11: Return `ResultEnvelope { outcome: Success, trust: Verified, provenance: Some(provenance), data: Some(response) }`

- [x] Task 2.3: Implement `get_repo_outline()` in `src/application/search.rs` (AC: 2, 5, 6, 8, 9)
  - [x] 2.3.1: Public function signature: `pub fn get_repo_outline(repo_id: &str, persistence: &RegistryPersistence, run_manager: &RunManager) -> Result<ResultEnvelope<RepoOutlineResponse>>`
  - [x] 2.3.2: Call `check_request_gate()` (reuse from 3.1)
  - [x] 2.3.3: Delegate to `get_repo_outline_ungated()` after gate passes

- [x] Task 2.4: Implement `get_repo_outline_ungated()` (AC: 2, 8, 9)
  - [x] 2.4.1: Get latest completed run via `persistence.get_latest_completed_run(repo_id)`
  - [x] 2.4.2: If `None`, return `ResultEnvelope { outcome: NotIndexed, trust: Verified, provenance: None, data: None }` (defense-in-depth)
  - [x] 2.4.3: Build `Provenance` from the run's metadata
  - [x] 2.4.4: Get file records via `persistence.get_file_records(run_id)`
  - [x] 2.4.5: Initialize coverage counters: `total_files`, `files_with_symbols`, `files_without_symbols`, `files_quarantined`, `files_failed`
  - [x] 2.4.6: For each file record:
    - Convert `PersistedFileOutcome` to `FileOutcomeStatus`
    - Count: `Committed` with non-empty symbols → `files_with_symbols`; `Committed` with empty symbols → `files_without_symbols`; `EmptySymbols` → `files_without_symbols`; `Failed` → `files_failed`; `Quarantined` → `files_quarantined`
    - Build `RepoOutlineEntry` with `relative_path`, `language`, `byte_len`, `symbol_count = record.symbols.len() as u32`, `status`
  - [x] 2.4.7: Sort entries by `relative_path` (alphabetical order for consistent output)
  - [x] 2.4.8: Build `RepoOutlineCoverage` from counters
  - [x] 2.4.9: If no file records exist → `ResultEnvelope { outcome: Empty, trust: Verified, provenance: Some(provenance), data: Some(RepoOutlineResponse { files: vec![], coverage }) }` — empty index is a valid state (repo was indexed but had no files)
  - [x] 2.4.10: If file records exist → `ResultEnvelope { outcome: Success, trust: Verified, provenance: Some(provenance), data: Some(response) }`

- [x] Task 2.5: Wire outlines into application layer (AC: 1, 2)
  - [x] 2.5.1: Add `get_file_outline` method to `ApplicationContext` in `src/application/mod.rs`
  - [x] 2.5.2: Add `get_repo_outline` method to `ApplicationContext` in `src/application/mod.rs`
  - [x] 2.5.3: Both delegate to `search::get_file_outline()` / `search::get_repo_outline()` with `self.run_manager.persistence()` and `&self.run_manager`
  - [x] 2.5.4: Note: neither needs `blob_store` — outline reads metadata only, no CAS I/O

### Phase 3: Unit Tests

- [x] Task 3.1: Unit tests for file outline (AC: 1, 3, 4, 5, 6, 7, 9)
  - [x] 3.1.1: `test_get_file_outline_returns_symbols` — happy path with file that has symbols
  - [x] 3.1.2: `test_get_file_outline_returns_empty_symbols_for_supported_language` — file with supported language but no symbols (has_symbol_support=true, symbols=[])
  - [x] 3.1.3: `test_get_file_outline_returns_empty_symbols_for_unsupported_language` — file with unsupported language (has_symbol_support=false, symbols=[])
  - [x] 3.1.4: `test_get_file_outline_error_for_missing_file` — file not in index → InvalidArgument error
  - [x] 3.1.5: `test_get_file_outline_returns_quarantined_for_quarantined_file` — outcome=Quarantined, trust=Quarantined (targeted request, NOT silently excluded like search)
  - [x] 3.1.6: `test_get_file_outline_rejects_invalidated_repo` — request-fatal gate
  - [x] 3.1.7: `test_get_file_outline_rejects_failed_repo` — request-fatal gate
  - [x] 3.1.8: `test_get_file_outline_rejects_active_mutation` — request-fatal gate
  - [x] 3.1.9: `test_get_file_outline_rejects_never_indexed_repo` — NeverIndexed
  - [x] 3.1.10: `test_get_file_outline_rejects_no_successful_runs` — NoSuccessfulRuns
  - [x] 3.1.11: `test_get_file_outline_includes_provenance_metadata` — run_id + committed_at_unix_ms
  - [x] 3.1.12: `test_get_file_outline_allows_degraded_repo` — NOT request-fatal
  - [x] 3.1.13: `test_get_file_outline_preserves_sort_order` — symbols returned in document order (sort_order field)
  - [x] 3.1.14: `test_get_file_outline_defense_in_depth_not_indexed` — no completed runs after gate passes → NotIndexed

- [x] Task 3.2: Unit tests for repository outline (AC: 2, 5, 6, 8, 9)
  - [x] 3.2.1: `test_get_repo_outline_returns_file_listing` — happy path with multiple files
  - [x] 3.2.2: `test_get_repo_outline_includes_quarantined_files` — quarantined files appear with status (not silently excluded)
  - [x] 3.2.3: `test_get_repo_outline_includes_failed_files` — failed files appear with status
  - [x] 3.2.4: `test_get_repo_outline_coverage_counts_correctly` — verify all coverage counter fields
  - [x] 3.2.5: `test_get_repo_outline_sorts_by_path` — entries sorted alphabetically by relative_path
  - [x] 3.2.6: `test_get_repo_outline_rejects_invalidated_repo` — request-fatal gate
  - [x] 3.2.7: `test_get_repo_outline_rejects_never_indexed_repo` — NeverIndexed
  - [x] 3.2.8: `test_get_repo_outline_empty_index` — indexed repo with no files → Empty with coverage
  - [x] 3.2.9: `test_get_repo_outline_includes_provenance_metadata` — run_id + committed_at_unix_ms
  - [x] 3.2.10: `test_get_repo_outline_allows_degraded_repo` — NOT request-fatal
  - [x] 3.2.11: `test_get_repo_outline_defense_in_depth_not_indexed` — no completed runs after gate passes → NotIndexed
  - [x] 3.2.12: `test_get_file_outline_latency_within_bounds` — sanity check for p50 ≤ 120ms
  - [x] 3.2.13: `test_get_repo_outline_latency_within_bounds` — sanity check for repo outline performance

### Phase 4: Integration Tests

- [x] Task 4.1: Integration tests in `tests/retrieval_integration.rs` (AC: 1, 2, 3, 4, 8)
  - [x] 4.1.1: End-to-end: index a fixture repo with Rust/Python files, then `get_file_outline`, verify symbols include name/kind/line_range/sort_order
  - [x] 4.1.2: End-to-end: `get_file_outline` for file with no symbols → verify explicit empty (symbols=[], has_symbol_support disambiguates)
  - [x] 4.1.3: End-to-end: `get_file_outline` for missing file → verify explicit InvalidArgument error
  - [x] 4.1.4: End-to-end: `get_repo_outline` → verify file listing with coverage metadata
  - [x] 4.1.5: End-to-end: `get_repo_outline` against invalidated repo → verify rejection
  - [x] 4.1.6: End-to-end: `get_repo_outline` with no completed runs → verify explicit never-indexed rejection

### Phase 5: Extend Contract Conformance Tests

- [x] Task 5.1: Extend `tests/retrieval_conformance.rs` (AC: 1, 2)
  - [x] 5.1.1: Conformance test: `OutlineSymbol` is constructable and serializable
  - [x] 5.1.2: Conformance test: `FileOutlineResponse` is constructable and serializable
  - [x] 5.1.3: Conformance test: `FileOutcomeStatus` is constructable and serializable (all 4 variants)
  - [x] 5.1.4: Conformance test: `RepoOutlineEntry` is constructable and serializable
  - [x] 5.1.5: Conformance test: `RepoOutlineCoverage` is constructable and serializable
  - [x] 5.1.6: Conformance test: `RepoOutlineResponse` is constructable and serializable

## Dev Notes

### Critical Design Decision: No Blob I/O Required

**Outline operations do NOT need CAS blob reads or blob integrity verification.** This is the same performance advantage as symbol search (Story 3.2).

Rationale: File outlines return pre-extracted symbol metadata from `FileRecord.symbols`, stored in the registry during indexing. Repository outlines return file-level metadata (path, language, size, symbol count) also from `FileRecord`. No raw source bytes are returned by either outline operation.

This means `get_file_outline()` and `get_repo_outline()` do NOT take a `blob_store` parameter.

Rule 1 ("Never return blob content without verifying blob_id matches") does not apply because outline operations return structural metadata, not blob content. Trust is inherited from the run's completion status, conveyed through provenance metadata.

### Quarantine Handling: Differs from Search

**Critical distinction from Stories 3.1 and 3.2:**

- **Search (3.1, 3.2)**: Quarantined files are **silently excluded** from search results. Search is a discovery operation — users don't know in advance what files match.
- **Outlines (3.3)**: Quarantined files are **surfaced with explicit trust state**. Outline operations are targeted requests — users request specific files or structural overviews.

This policy comes from the Epic 3 execution narrative (Failure Mode Guidance):
> *File-level*: individual file quarantined → excluded from search results; returned with `trust: quarantined` for targeted outline/retrieval requests.

**`get_file_outline` for quarantined file:**
- Returns `ResultEnvelope { outcome: Quarantined, trust: Quarantined, provenance: Some(...), data: None }`
- The caller knows the file is quarantined and can decide what to do (re-index, skip, etc.)

**`get_repo_outline` for repos with quarantined files:**
- Quarantined files **appear in the listing** with `status: Quarantined`
- Coverage metadata counts them in `files_quarantined`
- This gives operators visibility into what's quarantined vs healthy

Document this difference with a code comment referencing the narrative policy.

### Missing vs Empty Disambiguation (AC 3, AC 4)

Story 3.3 AC explicitly requires distinguishing two states:

1. **File not found in index** (AC 4): The file's `relative_path` does not match any `FileRecord` from the latest completed run. Return `TokenizorError::InvalidArgument("file not found in index: {path}")`. This is an explicit error, not an empty result.

2. **File found but no symbols** (AC 3): The file IS in the index but has no extracted symbols. This can happen because:
   - Language is not supported for symbol extraction (e.g., Markdown, TOML)
   - Language IS supported but file has no parseable symbols (empty file, comments only)

   Return `Success` with `FileOutlineResponse { symbols: vec![], has_symbol_support, ... }`. The `has_symbol_support` field distinguishes these sub-cases:
   - `has_symbol_support: true` → language has an extractor but found nothing
   - `has_symbol_support: false` → language has no extractor (coverage transparency)

### `has_symbol_support` Determination

Languages with symbol extractors (from `src/parsing/languages/`): **Rust, Python, JavaScript, TypeScript, Go, Java**.

`has_symbol_support` is `true` when:
- `record.outcome` is `Committed` AND `record.language` is one of the 6 supported languages

`has_symbol_support` is `false` when:
- `record.language` is not one of the 6 supported languages
- `record.outcome` is `Failed` (extraction failed — not the same as "no support")

Design note: `has_symbol_support` is a per-file signal, not a per-language guarantee. A supported language with a file that failed extraction is a different state than an unsupported language.

Implementation simplification: Since `PersistedFileOutcome::EmptySymbols` already signals "extraction ran but produced nothing," use it as the discriminant:
- `Committed` with non-empty symbols → `has_symbol_support: true`
- `Committed` with empty symbols → `has_symbol_support: true` (supported language, just no symbols in this file)
- `EmptySymbols` → `has_symbol_support: true` (extraction ran but found nothing)
- `Failed` → `has_symbol_support: false` (extraction failed, can't determine support)
- `Quarantined` → file outline returns quarantine envelope (has_symbol_support not applicable)

### Contract Requirements (Shared Read-Side Contract — Phase 0, established in Story 3.1)

The shared contract types in `src/domain/retrieval.rs` are ALREADY IMPLEMENTED. Story 3.3 extends them with `OutlineSymbol`, `FileOutlineResponse`, `FileOutcomeStatus`, `RepoOutlineEntry`, `RepoOutlineCoverage`, and `RepoOutlineResponse`. No changes to existing contract types.

- **Result envelope**: `ResultEnvelope<FileOutlineResponse>` and `ResultEnvelope<RepoOutlineResponse>` wrap outcome + trust + provenance + data
- **Request-level gating**: Reuse `check_request_gate()` from Story 3.1 — NO changes needed
- **Result-state disambiguation**: Success vs Empty vs NotIndexed vs Quarantined (outline-specific: Quarantined is a valid response for targeted file outline requests)
- **Active-context resolution**: Scoped to `repo_id` parameter

### Request-Fatal vs Item-Local Rules

**Identical to Stories 3.1 and 3.2** — reuse `check_request_gate()` unchanged.

**Request-fatal (entire request fails before any outline processing):**
- No active context / unknown `repo_id` → `get_repository()` returns `None`
- `RepositoryStatus::Invalidated` — trust explicitly revoked
- `RepositoryStatus::Failed` — index failed
- Active mutation in progress — `RunManager::has_active_run(repo_id)` or `get_active_run_id(repo_id)`
- No completed (`Succeeded`) runs — `get_latest_completed_run()` returns `None`, disambiguated as `NeverIndexed` or `NoSuccessfulRuns`

**NOT request-fatal:**
- `RepositoryStatus::Degraded` — soft warning, returns results with provenance (same tension with project-context.md Rule 2 documented in 3.1)

**Item-local (get_file_outline):**
- File not found in index → `InvalidArgument` error (AC 4)
- File quarantined → `Quarantined` result with explicit trust state (AC 7)
- File has no symbols → `Success` with empty symbols and `has_symbol_support` flag (AC 3)

**Item-local (get_repo_outline):**
- Quarantined files included in listing with status (AC 8)
- Failed files included in listing with status
- Files with no symbols included with `EmptySymbols` status

### Outline Response Schemas

```rust
/// A single symbol in a file outline (flattened from SymbolRecord)
pub struct OutlineSymbol {
    pub name: String,              // "main", "HashMap", "impl Default"
    pub kind: SymbolKind,          // Function, Struct, Impl, etc.
    pub line_range: (u32, u32),    // (start_line, end_line) — 0-based
    pub byte_range: (u32, u32),    // (start_byte, end_byte)
    pub depth: u32,                // 0 = top-level
    pub sort_order: u32,           // Document order for stable output
}

/// Response for get_file_outline — structural view of a single file
pub struct FileOutlineResponse {
    pub relative_path: String,
    pub language: LanguageId,
    pub byte_len: u64,
    pub symbols: Vec<OutlineSymbol>,
    pub has_symbol_support: bool,  // Distinguishes unsupported language from empty file
}

/// Simplified file outcome status for public API (strips internal details)
pub enum FileOutcomeStatus {
    Committed,       // Normal indexed file
    EmptySymbols,    // Indexed but no symbols extracted
    Failed,          // Processing failed (error details stripped)
    Quarantined,     // File quarantined
}

/// A file entry in a repository outline
pub struct RepoOutlineEntry {
    pub relative_path: String,
    pub language: LanguageId,
    pub byte_len: u64,
    pub symbol_count: u32,
    pub status: FileOutcomeStatus,
}

/// Coverage metadata for repository outline
pub struct RepoOutlineCoverage {
    pub total_files: u32,
    pub files_with_symbols: u32,
    pub files_without_symbols: u32,
    pub files_quarantined: u32,
    pub files_failed: u32,
}

/// Response for get_repo_outline — structural overview of entire repository
pub struct RepoOutlineResponse {
    pub files: Vec<RepoOutlineEntry>,
    pub coverage: RepoOutlineCoverage,
}
```

### Latency Requirements

- **`get_file_outline`**: p50 ≤ 120 ms, p95 ≤ 350 ms on representative medium-to-large repositories (warm local index) [Source: epics.md#NFR3]
- File outline should be inherently fast because it reads a single file's metadata from the registry (no CAS I/O, no iteration over all files).
- **`get_repo_outline`**: No explicit NFR, but should be comparable to symbol search latency since it iterates file records without blob I/O. Target: p50 ≤ 150 ms, p95 ≤ 500 ms (match text search target as reasonable upper bound).
- **"Done" criteria**: A basic latency sanity check against the NFR target is REQUIRED.

### Testing Requirements

- **Naming**: `test_verb_condition` (e.g., `test_get_file_outline_returns_symbols`)
- **Fakes**: Hand-written fakes inside `#[cfg(test)] mod tests`. No mock crates.
- **Assertions**: Plain `assert!`, `assert_eq!`. No assertion crates.
- **Test type**: `#[test]` by default. `#[tokio::test]` only for async fn tests.
- **Unit tests**: `#[cfg(test)]` blocks inside `src/application/search.rs`.
- **Integration tests**: Extend `tests/retrieval_integration.rs`.
- **Call verification**: `AtomicUsize` counters on fakes to verify interaction counts (if needed).
- **Fixture**: Use existing test setup patterns from Stories 3.1 and 3.2. Extend fixture repos if needed.
- **Setup**: Follow existing `setup_test_env()` pattern returning `(TempDir, Arc<RunManager>, TempDir, Arc<dyn BlobStore>)` — even though outlines don't use blob_store directly, integration tests need it for the indexing pipeline.
- **Contract conformance**: Story must pass the evolving contract-conformance test skeleton before moving to `done`.

### Epic 3 Retrieval Architecture Rules (Mandatory)

From project-context.md — non-negotiable for ALL Epic 3 code:

1. **Rule 1 (blob verification)**: Does NOT apply to outlines — outline operations return metadata, not blob content. See "Critical Design Decision" section above.
2. **Rule 2 (repo status check)**: Enforced via `check_request_gate()`. Degraded passes with provenance (documented tension with project-context.md).
3. **Rule 3 (provenance)**: Every `FileOutlineResponse` carries provenance (run_id + committed_at_unix_ms + repo_id). `RepoOutlineResponse` carries provenance at the envelope level.
4. **Rule 4 (disambiguation)**: Success (outline returned) vs Empty (indexed but no files) vs NotIndexed (no completed runs) vs Quarantined (file quarantined) are distinct outcomes. File not found is an explicit error.
5. **Rule 5 (early gating)**: Gate check runs FIRST, before any file record iteration.
6. **Rule 6 (quarantine in search)**: Rule 6 says "must never appear in **search** results." Outlines are NOT search — they are structural views. Quarantined files are **surfaced with explicit trust state** for outlines per the Epic 3 execution narrative's targeted-request policy. Document this distinction with a code comment.

### Scope Boundaries — What Story 3.3 Does NOT Cover

- MCP tool exposure (Story 3.4 — `get_file_outline` and `get_repo_outline` tools)
- Text search (Story 3.1 — done)
- Symbol search (Story 3.2 — done)
- Verified source retrieval (Story 3.5)
- Blocking/quarantine behavior for verified retrieval (Story 3.6)
- Batched retrieval (Story 3.7)
- Hierarchical/tree-structured outline reconstruction from flat symbol list (consumers do this from depth/sort_order fields)
- Advanced outline features (imports, exports, dependency graphs)
- Cross-repository outline
- Live filesystem change detection or outline refresh
- Write-side operations (re-indexing, repair, state mutation)
- Freshness policy enforcement (deferred to Epic 4/5)

### Existing Read-Side Interfaces from Epic 2 (Verified in Stories 3.1 and 3.2)

Outline operations use a SUBSET of the interfaces — specifically, NO `BlobStore::read_bytes`:

| Interface | Used? | Actual Signature | Quirks |
|-----------|-------|-----------------|--------|
| Repo status | YES | `RegistryPersistence::get_repository(&self, repo_id: &str) -> Result<Option<Repository>>` | Returns `None` for unknown repo_id. |
| Active mutation | YES | `RunManager::has_active_run(&self, repo_id: &str) -> bool` / `get_active_run_id(&self, repo_id: &str) -> Option<String>` | In-memory HashMap check. |
| Latest completed run | YES | `RegistryPersistence::get_latest_completed_run(&self, repo_id: &str) -> Result<Option<IndexRun>>` | Filters ONLY by `Succeeded`. Returns `None` if no Succeeded runs. |
| File records by run | YES | `RegistryPersistence::get_file_records(&self, run_id: &str) -> Result<Vec<FileRecord>>` | Returns empty `Vec` for unknown run_id. `FileRecord.symbols: Vec<SymbolRecord>` is the symbol data. |
| Runs by repo | YES (gate) | `RegistryPersistence::get_runs_by_repo(&self, repo_id: &str) -> Result<Vec<IndexRun>>` | Used by gate to disambiguate NeverIndexed vs NoSuccessfulRuns. |
| Blob lookup | **NO** | `BlobStore::read_bytes` | NOT needed — outline reads metadata only. |

### SymbolRecord Source Data (from `src/domain/index.rs`)

```rust
pub struct SymbolRecord {
    pub name: String,              // "main", "HashMap", "impl Default for MyStruct"
    pub kind: SymbolKind,          // 13 variants
    pub depth: u32,                // 0 = top-level, 1+ = nested
    pub sort_order: u32,           // Document order
    pub byte_range: (u32, u32),    // (start_byte, end_byte)
    pub line_range: (u32, u32),    // (start_line, end_line) — 0-based rows
}
```

**`OutlineSymbol` maps 1:1 from `SymbolRecord`** — all 6 fields are copied directly. No recomputation.

### PersistedFileOutcome Handling

```
For get_file_outline (targeted, single file):
  Committed         → return Success with symbols
  EmptySymbols       → return Success with empty symbols (has_symbol_support = true)
  Failed { error }   → return Success with empty symbols (has_symbol_support = false)
  Quarantined { .. } → return Quarantined envelope (data = None)

For get_repo_outline (listing, all files):
  Committed         → include with status=Committed, count symbols
  EmptySymbols       → include with status=EmptySymbols, files_without_symbols++
  Failed { error }   → include with status=Failed, files_failed++
  Quarantined { .. } → include with status=Quarantined, files_quarantined++
```

### Build Order (Mandatory — domain first, wiring last)

1. Domain types in `src/domain/retrieval.rs` — `OutlineSymbol`, `FileOutlineResponse`, `FileOutcomeStatus`, `RepoOutlineEntry`, `RepoOutlineCoverage`, `RepoOutlineResponse`
2. Re-exports in `src/domain/mod.rs`
3. Outline functions in `src/application/search.rs` — `get_file_outline()` + `get_file_outline_ungated()` + `get_repo_outline()` + `get_repo_outline_ungated()`
4. Application wiring in `src/application/mod.rs` — `ApplicationContext::get_file_outline()` + `ApplicationContext::get_repo_outline()`
5. Unit tests in `src/application/search.rs` — 27 tests covering all ACs and gate conditions
6. Integration tests in `tests/retrieval_integration.rs` — 6 end-to-end tests
7. Conformance tests in `tests/retrieval_conformance.rs` — 6 new type conformance tests

### Architecture Compliance

- **Layer**: Outline logic in `application/`. Domain types in `domain/`. MCP exposure deferred to Story 3.4 (`protocol/`).
- **Persistence model**: All reads via `RegistryPersistence`. No SpacetimeDB reads.
- **Error handling**: Reuse `TokenizorError::RequestGated { gate_error: String }` from Story 3.1 for gate failures. Use `TokenizorError::InvalidArgument` for file-not-found. No new error variants needed.
- **No Mutex across .await**: Not applicable — outline operations are synchronous (no async I/O needed since no blob reads).
- **No mock crates**: Hand-written fakes with `AtomicUsize` counters.
- **No assertion crates**: Plain `assert!`/`assert_eq!`.

### Previous Story Intelligence (Stories 3.1 and 3.2)

**Patterns established in Stories 3.1 and 3.2 that MUST be followed:**
- `check_request_gate()` function is the single gate entry point — reuse as-is
- Gate returns `TokenizorError::RequestGated` which maps to MCP `invalid_params`
- `Degraded` repos pass gate with warning log
- `ResultEnvelope` pattern with `outcome`, `trust`, `provenance`, `data`
- Defense-in-depth `NotIndexed` check inside ungated function (in case gate is bypassed by direct call)
- Case-sensitive path matching for file lookups (relative_path is exact)
- Coverage metadata on responses (3.2 established this pattern with `SymbolCoverage`)

**Story 3.2 code review findings that apply:**
- Failed files must be excluded from matches and counted in coverage (applicable to repo outline)
- Integration test assertions must be specific — verify symbol name/kind/path/line_range, not just success/data presence
- Weak assertions like `assert!(result.is_ok())` without checking data contents are insufficient
- Follow `wait_for_run_success()` pattern for integration tests (no fixed sleeps)

**Dev agent failure modes from Epic 2 retrospective (guard against):**
1. **No-op/sentinel tests**: `assert!(true)` or conditional logic that silently passes. Every test assertion MUST be able to fail.
2. **`is_systemic()` misclassification**: No new `TokenizorError` variants in this story, but verify if touched.
3. **Data computed but dropped**: Ensure coverage counters are wired to the response, not computed and discarded.
4. **Missing tests for new code paths**: Every outline path (success, empty, not-found, quarantined) needs a test.

**Story 3.1 + 3.2 implementation stats:** 63 new tests total (42 unit + 9 integration + 12 conformance). Total test count after 3.2: 432. Expect ~39 new tests from 3.3 (27 unit + 6 integration + 6 conformance).

### Self-Audit Checklist (mandatory before requesting review)

_Run this checklist after all tasks are complete. This is a blocking step — do not request review until every item is verified._

#### Generic Verification
- [x] For every task marked `[x]`, cite the specific test that verifies it
- [x] For every new error variant or branch, confirm a test exercises it
- [x] For every computed value, trace it to where it surfaces (log, return value, persistence)
- [x] For every test, verify the assertion can actually fail (no `assert!(true)`, no conditionals that always pass)

#### Epic 3-Specific Trust Verification
- [x] For every outline result, confirm provenance metadata (run_id, committed_at_unix_ms) is populated
- [x] For every request gate condition, confirm a test exercises the rejection path
- [x] For every "no results" path, confirm the response distinguishes empty vs missing vs stale vs quarantined
- [x] For the request gate, confirm it runs BEFORE any file record iteration
- [x] Confirm the latency sanity check test exists and asserts a reasonable bound
- [x] Confirm contract-conformance tests exist and pass for new types

#### Story 3.3-Specific Verification
- [x] Confirm NO blob reads occur during outline operations (no `BlobStore::read_bytes` calls)
- [x] Confirm `get_file_outline` for quarantined file returns `Quarantined` outcome with `trust: Quarantined` (NOT silently excluded like search)
- [x] Confirm `get_repo_outline` includes quarantined files in listing with status
- [x] Confirm `get_file_outline` for missing file returns explicit `InvalidArgument` error (not empty result)
- [x] Confirm `has_symbol_support` field correctly distinguishes unsupported language from supported-but-empty
- [x] Confirm `OutlineSymbol` fields come directly from `SymbolRecord` without recomputation
- [x] Confirm `FileOutcomeStatus::From<&PersistedFileOutcome>` conversion strips internal error/reason details
- [x] Confirm `RepoOutlineCoverage` counters match the actual file record iteration
- [x] Confirm repo outline entries are sorted by `relative_path`

### Project Structure Notes

- Extended: `src/domain/retrieval.rs` — add `OutlineSymbol`, `FileOutlineResponse`, `FileOutcomeStatus`, `RepoOutlineEntry`, `RepoOutlineCoverage`, `RepoOutlineResponse`
- Extended: `src/domain/mod.rs` — re-export new types
- Extended: `src/application/search.rs` — add `get_file_outline()`, `get_file_outline_ungated()`, `get_repo_outline()`, `get_repo_outline_ungated()`
- Extended: `src/application/mod.rs` — add `get_file_outline()` and `get_repo_outline()` methods to `ApplicationContext`
- Extended: `tests/retrieval_integration.rs` — add outline integration tests
- Extended: `tests/retrieval_conformance.rs` — add type conformance tests for new types
- NO changes to: `src/storage/` (reads use existing `RegistryPersistence`)
- NO changes to: `src/error.rs` (reuses existing `RequestGated` and `InvalidArgument`)
- NO changes to: `src/protocol/mcp.rs` (MCP exposure is Story 3.4)
- NO new files — all additions are extensions to existing files

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story-3.3] — User story, ACs, BDD scenarios
- [Source: _bmad-output/planning-artifacts/epics.md#Epic-3-Execution-Narrative] — Phase model, ADRs, failure modes, gating rules, quarantine policy for targeted requests
- [Source: _bmad-output/planning-artifacts/epics.md#Contract-Gaps] — Outline completeness indicators: "coverage metadata for outlines belongs in 3.3's story file"
- [Source: _bmad-output/planning-artifacts/epics.md#NFR3] — get_file_outline latency: p50<=120ms, p95<=350ms
- [Source: _bmad-output/project-context.md#Epic-3-Retrieval-Architecture] — 6 mandatory retrieval rules (Rule 6 applies to search, not outlines)
- [Source: _bmad-output/project-context.md#Symbol-storage-shape] — Flat `Vec<SymbolRecord>` with depth + sort_order enables hierarchical outline reconstruction
- [Source: _bmad-output/planning-artifacts/architecture.md#Retrieval-Trust-Model] — Trust verification model
- [Source: _bmad-output/planning-artifacts/architecture.md#MCP-tool-naming] — Canonical names: `get_file_outline`, `get_repo_outline`
- [Source: _bmad-output/implementation-artifacts/3-1-search-indexed-repositories-by-text.md] — Previous story patterns, code review findings
- [Source: _bmad-output/implementation-artifacts/3-2-search-indexed-repositories-by-symbol.md] — Symbol search patterns, coverage transparency, quarantine handling in search
- [Source: src/domain/index.rs] — SymbolRecord, SymbolKind, FileRecord, PersistedFileOutcome definitions
- [Source: src/domain/retrieval.rs] — Existing contract types
- [Source: src/application/search.rs] — Existing search implementation and check_request_gate()

## Dev Agent Record

### Agent Model Used

GPT-5 Codex

### Debug Log References

- Initial `jcodemunch` incremental re-index timed out once after the code edits; immediate retry succeeded with no additional source drift.

### Completion Notes List

- Added `OutlineSymbol`, `FileOutlineResponse`, `FileOutcomeStatus`, `RepoOutlineEntry`, `RepoOutlineCoverage`, and `RepoOutlineResponse` to `src/domain/retrieval.rs`, then re-exported them from `src/domain/mod.rs`.
- Implemented `get_file_outline()` / `get_file_outline_ungated()` and `get_repo_outline()` / `get_repo_outline_ungated()` in `src/application/search.rs` with shared request gating, explicit missing-vs-empty disambiguation, targeted quarantine surfacing for file outlines, and repository coverage counters.
- Wired `get_file_outline()` and `get_repo_outline()` into `ApplicationContext` in `src/application/mod.rs`.
- Added 29 outline-specific unit tests in `src/application/search.rs`, including request-gate rejection paths, provenance assertions, missing-file handling, quarantine behavior, and latency sanity checks.
- Added 6 end-to-end outline integration tests in `tests/retrieval_integration.rs` covering happy-path file outline, explicit empty file outline, missing file rejection, repository coverage output, invalidated repository rejection, and never-indexed rejection.
- Added 6 contract-conformance tests in `tests/retrieval_conformance.rs` for the new outline types.
- Validation passed with `cargo fmt`, `cargo test --test retrieval_conformance --test retrieval_integration`, and `cargo test`.

### Change Log

- 2026-03-08: Implemented Story 3.3 file/repository outline retrieval, including domain contracts, application wiring, unit coverage, integration coverage, and conformance coverage.
- 2026-03-08: Applied BMAD code-review fixes by completing the missing 3.3 implementation and syncing the story/sprint metadata to the validated code state.

### File List

- `src/domain/retrieval.rs` — added outline contract types and `FileOutcomeStatus::From<&PersistedFileOutcome>`
- `src/domain/mod.rs` — re-exported the new outline types
- `src/application/search.rs` — added file/repository outline retrieval functions and outline-specific unit tests
- `src/application/mod.rs` — added `ApplicationContext::get_file_outline()` and `ApplicationContext::get_repo_outline()`
- `tests/retrieval_integration.rs` — added 6 outline integration tests
- `tests/retrieval_conformance.rs` — added 6 outline type conformance tests

## Senior Developer Review (AI)

### Reviewer

Sir on 2026-03-08

### Findings Fixed

- Implemented the missing Story 3.3 outline contract types and application/query surface that were absent during the initial review.
- Added targeted unit, integration, and conformance coverage so the story now proves missing-vs-empty handling, quarantine transparency, request-gate behavior, provenance, coverage counters, and latency bounds.
- Synced the story artifact to the actual implementation by marking completed tasks/checklists and recording the real changed files plus validation commands.

### Verification

- `cargo fmt`
- `cargo test --test retrieval_conformance --test retrieval_integration`
- `cargo test`

### Outcome

- Review findings were fixed automatically. Story 3.3 now satisfies the implementation and validation gates and is ready to remain `done`.
