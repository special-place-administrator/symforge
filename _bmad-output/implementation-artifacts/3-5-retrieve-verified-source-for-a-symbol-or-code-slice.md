# Story 3.5: Retrieve Verified Source for a Symbol or Code Slice

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an AI coding user,
I want to retrieve source for a symbol or code slice from indexed content,
so that I can rely on returned code as trustworthy retrieval rather than guessed output.

**FRs implemented:** FR21, FR25, FR28, FR29

- **FR21**: Users can retrieve source for a symbol or equivalent code slice from indexed content.
- **FR25**: Users can rely on Tokenizor to verify source retrieval before trusted code is served.
- **FR28**: Users can rely on Tokenizor to preserve exact raw source fidelity for retrieval-sensitive content.
- **FR29**: Users can distinguish between trusted retrieval results and results that require repair or re-index before use.

## Acceptance Criteria

1. **Given** indexed symbol metadata points to stored raw bytes **When** I request source retrieval with `repo_id`, `relative_path`, and `symbol_name` **Then** Tokenizor reads bytes from CAS via `blob_store.read_bytes(blob_id)`, verifies `digest_hex(bytes) == blob_id`, extracts the symbol's byte range, and returns verified source **And** the result has `outcome: Success`, `trust: Verified`, and populated provenance (AC: 1, verification)
2. **Given** the requested source passes verification **When** retrieval completes **Then** Tokenizor returns the verified source slice as a UTF-8 string **And** it preserves exact raw source fidelity — no line-ending conversion, encoding normalization, or whitespace transformation (AC: 2, fidelity)
3. **Given** the symbol name does not match any symbol in the file **When** I request retrieval **Then** Tokenizor returns an explicit `InvalidArgument` error naming the symbol and file (AC: 3, symbol not found)
4. **Given** the file is not found in the index **When** I request symbol retrieval **Then** Tokenizor returns an explicit `InvalidArgument` error naming the file path (AC: 4, file not found)
5. **Given** the file is quarantined **When** I request symbol retrieval **Then** Tokenizor returns `outcome: Quarantined`, `trust: Quarantined`, `data: None` — consistent with `get_file_outline` targeted-request quarantine policy (AC: 5, quarantine)
6. **Given** blob integrity verification fails (hash mismatch) **When** retrieval is attempted **Then** Tokenizor returns `outcome: Blocked { reason }`, `trust: Suspect`, `data: None` **And** does not serve the unverified content as trusted (AC: 6, integrity failure)
7. **Given** the symbol's byte range exceeds the blob size or is malformed (start > end) **When** retrieval is attempted **Then** Tokenizor returns `outcome: Blocked { reason }`, `trust: Suspect`, `data: None` (AC: 7, span validation)
8. **Given** the extracted source bytes are not valid UTF-8 **When** retrieval is attempted **Then** Tokenizor returns `outcome: Blocked { reason }`, `trust: Suspect`, `data: None` (AC: 8, encoding)
9. **Given** the blob read from CAS fails (I/O error, missing blob) **When** retrieval is attempted **Then** Tokenizor returns `outcome: Blocked { reason }`, `trust: Suspect`, `data: None` (AC: 9, blob unavailable)
10. **Given** the repository is invalidated, failed, has an active mutation, or is never indexed **When** any retrieval is requested **Then** the request fails at the gate with explicit `RequestGated` error before any CAS I/O or symbol lookup (AC: 10, gate)
11. **Given** the MCP server is running **When** a client lists available tools **Then** `get_symbol` appears alongside existing tools with a description enabling AI client discovery (AC: 11, MCP registration)
12. **Given** `get_symbol` is invoked with missing required parameters **When** the request is processed **Then** Tokenizor returns `invalid_params` error naming the missing parameter (AC: 12, parameter validation)
13. **Given** `get_symbol` is invoked with an optional `kind_filter` that is not a valid `SymbolKind` **When** the request is processed **Then** Tokenizor returns `invalid_params` error listing valid kinds (AC: 13, kind_filter validation)
14. **Given** multiple symbols match the name (and optional kind_filter) in the same file **When** retrieval is requested **Then** Tokenizor returns the first match by `sort_order` (document order) — deterministic, repeatable (AC: 14, disambiguation)

**Scope note:** Story 3.5 implements the core verified-retrieval path with basic failure handling (Blocked/Suspect responses). Story 3.6 extends this with sophisticated blocking/quarantine behavior, actionable repair guidance, and re-index implications. Story 3.7 implements batched retrieval via `get_symbols`.

## Tasks / Subtasks

### Phase 1: Domain Type

- [x] Task 1.1: Define `VerifiedSourceResponse` struct in `src/domain/retrieval.rs` (AC: 1, 2)
  - [x] 1.1.1: Create `VerifiedSourceResponse` with fields: `relative_path: String`, `language: LanguageId`, `symbol_name: String`, `symbol_kind: SymbolKind`, `line_range: (u32, u32)`, `byte_range: (u32, u32)`, `source: String`
  - [x] 1.1.2: Derive `Debug, Clone, Serialize, Deserialize, PartialEq, Eq` (match existing type derives)
  - [x] 1.1.3: Add doc comment: `/// Response for get_symbol — verified source text for a single symbol`

- [x] Task 1.2: Re-export `VerifiedSourceResponse` from `src/domain/mod.rs` (AC: 1)
  - [x] 1.2.1: Add `VerifiedSourceResponse` to `pub use retrieval::{...}` in `src/domain/mod.rs`

### Phase 2: Verified Retrieval Implementation

- [x] Task 2.1: Implement `get_symbol()` in `src/application/search.rs` (AC: 1, 3, 4, 5, 10)
  - [x] 2.1.1: Public function signature: `pub fn get_symbol(repo_id: &str, relative_path: &str, symbol_name: &str, kind_filter: Option<SymbolKind>, persistence: &RegistryPersistence, run_manager: &RunManager, blob_store: &dyn BlobStore) -> Result<ResultEnvelope<VerifiedSourceResponse>>`
  - [x] 2.1.2: Validate `symbol_name` is non-empty: if empty, return `Err(TokenizorError::InvalidArgument("symbol_name must not be empty".to_string()))`
  - [x] 2.1.3: Validate `relative_path` is non-empty: if empty, return `Err(TokenizorError::InvalidArgument("relative_path must not be empty".to_string()))`
  - [x] 2.1.4: Call `check_request_gate(repo_id, persistence, run_manager)?` — reuse from Story 3.1, NO changes
  - [x] 2.1.5: Delegate to `get_symbol_ungated()` after gate passes

- [x] Task 2.2: Implement `get_symbol_ungated()` (AC: 1, 2, 3, 4, 5, 6, 7, 8, 9, 14)
  - [x] 2.2.1: Get latest completed run via `persistence.get_latest_completed_run(repo_id)?`
  - [x] 2.2.2: If `None`, return `Ok(ResultEnvelope { outcome: NotIndexed, trust: Verified, provenance: None, data: None })` (defense-in-depth — gate should catch first)
  - [x] 2.2.3: Build `Provenance` from the run's metadata using existing `file_record_provenance()` helper pattern
  - [x] 2.2.4: Get file records via `persistence.get_file_records(&latest_run.run_id)?`
  - [x] 2.2.5: Find the file record matching `relative_path` (case-sensitive exact match)
  - [x] 2.2.6: If no matching file record → return `Err(TokenizorError::InvalidArgument(format!("file not found in index: {relative_path}")))`
  - [x] 2.2.7: If file is quarantined (`PersistedFileOutcome::Quarantined { .. }`) → return `Ok(ResultEnvelope { outcome: Quarantined, trust: Quarantined, provenance: Some(provenance), data: None })` — targeted request surfaces quarantine (same policy as `get_file_outline`)
  - [x] 2.2.8: Find matching symbol in `record.symbols`:
    - Filter by exact `symbol_name` match: `symbol.name == symbol_name`
    - If `kind_filter` is `Some(kind)`, further filter by `symbol.kind == kind`
    - If no matches → return `Err(TokenizorError::InvalidArgument(format!("symbol not found: `{symbol_name}` in file: {relative_path}")))`
    - If multiple matches → select the first by `sort_order` (document order) for deterministic behavior
  - [x] 2.2.9: Read raw bytes from CAS: `let blob_bytes = blob_store.read_bytes(&record.blob_id)?` — wrap error to return `Blocked` instead of propagating
  - [x] 2.2.10: Verify blob integrity: `let computed_hash = digest_hex(&blob_bytes); if computed_hash != record.blob_id` → return `Blocked { reason: "blob integrity verification failed: content hash mismatch" }` with `trust: Suspect`
  - [x] 2.2.11: Validate byte range: `let (start, end) = symbol.byte_range;` — check `end as usize <= blob_bytes.len() && start <= end` — if invalid → return `Blocked { reason }` with `trust: Suspect`
  - [x] 2.2.12: Extract source bytes: `let source_bytes = &blob_bytes[start as usize..end as usize];`
  - [x] 2.2.13: Convert to UTF-8: `String::from_utf8(source_bytes.to_vec())` — if error → return `Blocked { reason: "symbol source contains non-UTF-8 bytes" }` with `trust: Suspect`
  - [x] 2.2.14: Build `VerifiedSourceResponse` with `relative_path`, `language`, `symbol_name`, `symbol_kind`, `line_range`, `byte_range`, `source`
  - [x] 2.2.15: Return `Ok(ResultEnvelope { outcome: Success, trust: Verified, provenance: Some(provenance), data: Some(response) })`

### Phase 3: Application Wiring

- [x] Task 3.1: Add `get_symbol` method to `ApplicationContext` in `src/application/mod.rs` (AC: 1)
  - [x] 3.1.1: Method signature: `pub fn get_symbol(&self, repo_id: &str, relative_path: &str, symbol_name: &str, kind_filter: Option<SymbolKind>) -> Result<ResultEnvelope<VerifiedSourceResponse>>`
  - [x] 3.1.2: Delegate to `search::get_symbol(repo_id, relative_path, symbol_name, kind_filter, self.run_manager.persistence(), &self.run_manager, self.blob_store.as_ref())`
  - [x] 3.1.3: Note: `get_symbol` REQUIRES `blob_store` (unlike outlines). Follow the same delegation pattern as `search_text()` which also takes `blob_store`.

### Phase 4: MCP Tool

- [x] Task 4.1: Add `get_symbol` MCP tool to `TokenizorServer` in `src/protocol/mcp.rs` (AC: 11, 12, 13)
  - [x] 4.1.1: Add `#[tool(description = "...")]` method in the `#[tool_router] impl TokenizorServer` block (NOT the `#[tool_handler]` block)
  - [x] 4.1.2: Description: `"Retrieve verified source code for a specific symbol from an indexed repository. Returns the exact source text with byte-exact verification against stored content. Verification ensures blob integrity (content hash match), span validity, and raw source fidelity. Parameters: repo_id (string, required), relative_path (string, required — file path relative to repository root), symbol_name (string, required — exact symbol name to retrieve), kind_filter (string, optional: function|method|class|struct|enum|interface|module|constant|variable|type|trait|impl|other)."`
  - [x] 4.1.3: Parse parameters from `rmcp::model::JsonObject` — follow the established pattern from Story 3.4

- [x] Task 4.2: Create `parse_get_symbol_params()` helper (AC: 12, 13)
  - [x] 4.2.1: Define struct: `struct GetSymbolParams { repo_id: String, relative_path: String, symbol_name: String, kind_filter: Option<SymbolKind> }`
  - [x] 4.2.2: Function signature: `fn parse_get_symbol_params(params: &rmcp::model::JsonObject) -> Result<GetSymbolParams, McpError>`
  - [x] 4.2.3: `repo_id` — required via `required_non_empty_string_param(params, "repo_id")`; missing → `McpError::invalid_params("missing required parameter: repo_id", None)`, empty/non-string → `McpError::invalid_params("invalid parameter \`repo_id\`: expected non-empty string", None)`
  - [x] 4.2.4: `relative_path` — required via `required_non_empty_string_param(params, "relative_path")`; missing → `"missing required parameter: relative_path"`, empty/non-string → `"invalid parameter \`relative_path\`: expected non-empty string"`
  - [x] 4.2.5: `symbol_name` — required via `required_non_empty_string_param(params, "symbol_name")`; missing → `"missing required parameter: symbol_name"`, empty/non-string → `"invalid parameter \`symbol_name\`: expected non-empty string"`
  - [x] 4.2.6: `kind_filter` — optional: reuse existing `parse_kind_filter()` from Story 3.4

- [x] Task 4.3: Wire MCP tool method (AC: 11)
  - [x] 4.3.1: Tool method calls `parse_get_symbol_params(&params)?`
  - [x] 4.3.2: Delegates to `self.application.get_symbol(&p.repo_id, &p.relative_path, &p.symbol_name, p.kind_filter).map_err(to_mcp_error)?`
  - [x] 4.3.3: Serializes `ResultEnvelope<VerifiedSourceResponse>` to JSON via `serde_json::to_string_pretty`
  - [x] 4.3.4: Returns `Ok(CallToolResult::success(vec![Content::text(json)]))`

- [x] Task 4.4: Update server instructions in `get_info()` (AC: 11)
  - [x] 4.4.1: Update the `with_instructions()` string to mention `get_symbol` alongside the 4 existing retrieval tools
  - [x] 4.4.2: New instructions text updated to include `get_symbol`

### Phase 5: Unit Tests

- [x] Task 5.1: Unit tests for `get_symbol` in `src/application/search.rs` (AC: 1–10, 14)
  - [x] 5.1.1: `test_get_symbol_returns_verified_source` — happy path: index a file with symbols, retrieve by name → verify `outcome: Success`, `trust: Verified`, `source` matches expected content, `symbol_name`, `symbol_kind`, `line_range`, `byte_range` all correct
  - [x] 5.1.2: `test_get_symbol_preserves_raw_source_fidelity` — verify source text is byte-exact: no trailing newline added, no whitespace normalization, no line-ending conversion
  - [x] 5.1.3: `test_get_symbol_error_for_missing_file` — file not in index → `InvalidArgument("file not found in index: ...")`
  - [x] 5.1.4: `test_get_symbol_error_for_missing_symbol` — symbol name not in file → `InvalidArgument("symbol not found: ...")`
  - [x] 5.1.5: `test_get_symbol_returns_quarantined_for_quarantined_file` — `outcome: Quarantined`, `trust: Quarantined`, `data: None`
  - [x] 5.1.6: `test_get_symbol_blocks_on_blob_integrity_mismatch` — fake blob store returns wrong content → `outcome: Blocked`, `trust: Suspect`, `data: None`
  - [x] 5.1.7: `test_get_symbol_blocks_on_byte_range_out_of_bounds` — symbol byte range exceeds blob size → `outcome: Blocked`, `trust: Suspect`
  - [x] 5.1.8: `test_get_symbol_blocks_on_malformed_byte_range` — start > end → `outcome: Blocked`, `trust: Suspect`
  - [x] 5.1.9: `test_get_symbol_blocks_on_non_utf8_source` — blob content has non-UTF-8 bytes in symbol range → `outcome: Blocked`, `trust: Suspect`
  - [x] 5.1.10: `test_get_symbol_blocks_on_blob_read_failure` — blob store returns error → `outcome: Blocked`, `trust: Suspect`
  - [x] 5.1.11: `test_get_symbol_rejects_invalidated_repo` — request-fatal gate
  - [x] 5.1.12: `test_get_symbol_rejects_failed_repo` — request-fatal gate
  - [x] 5.1.13: `test_get_symbol_rejects_active_mutation` — request-fatal gate
  - [x] 5.1.14: `test_get_symbol_rejects_never_indexed_repo` — NeverIndexed gate
  - [x] 5.1.15: `test_get_symbol_rejects_no_successful_runs` — NoSuccessfulRuns gate
  - [x] 5.1.16: `test_get_symbol_includes_provenance_metadata` — `run_id` + `committed_at_unix_ms` populated
  - [x] 5.1.17: `test_get_symbol_with_kind_filter` — filter narrows to correct symbol when name is ambiguous
  - [x] 5.1.18: `test_get_symbol_returns_first_by_sort_order_when_ambiguous` — multiple matches → first by document order
  - [x] 5.1.19: `test_get_symbol_rejects_degraded_repo` — request-fatal (consistent with existing retrieval ops and Epic 3 Rule 2)
  - [x] 5.1.20: `test_get_symbol_defense_in_depth_not_indexed` — no completed runs after gate passes → NotIndexed
  - [x] 5.1.21: `test_get_symbol_rejects_empty_symbol_name` — `InvalidArgument("symbol_name must not be empty")`
  - [x] 5.1.22: `test_get_symbol_rejects_empty_relative_path` — `InvalidArgument("relative_path must not be empty")`
  - [x] 5.1.23: `test_get_symbol_latency_within_bounds` — sanity check for p50 ≤ 150ms

- [x] Task 5.2: MCP parameter validation unit tests in `src/protocol/mcp.rs` (AC: 12, 13)
  - [x] 5.2.1: `test_get_symbol_tool_rejects_missing_repo_id` — `invalid_params("missing required parameter: repo_id")`
  - [x] 5.2.2: `test_get_symbol_tool_rejects_missing_relative_path` — `invalid_params("missing required parameter: relative_path")`
  - [x] 5.2.3: `test_get_symbol_tool_rejects_missing_symbol_name` — `invalid_params("missing required parameter: symbol_name")`
  - [x] 5.2.4: `test_get_symbol_tool_rejects_empty_repo_id` — empty string → `invalid_params`
  - [x] 5.2.5: `test_get_symbol_tool_rejects_empty_relative_path` — empty string → `invalid_params`
  - [x] 5.2.6: `test_get_symbol_tool_rejects_empty_symbol_name` — empty string → `invalid_params`
  - [x] 5.2.7: `test_get_symbol_tool_rejects_invalid_kind_filter` — reuses existing `parse_kind_filter` error path
  - [x] 5.2.8: `test_get_symbol_tool_accepts_valid_kind_filter` — `kind_filter: "function"` parses correctly
  - [x] 5.2.9: `test_get_symbol_tool_accepts_missing_kind_filter` — `None` (valid)
  - [x] 5.2.10: `test_get_symbol_tool_rejects_non_string_repo_id` — non-string required param → `invalid_params("invalid parameter \`repo_id\`: expected non-empty string")`
  - [x] 5.2.11: `test_get_symbol_tool_rejects_non_string_relative_path` — non-string required param → `invalid_params("invalid parameter \`relative_path\`: expected non-empty string")`
  - [x] 5.2.12: `test_get_symbol_tool_rejects_non_string_symbol_name` — non-string required param → `invalid_params("invalid parameter \`symbol_name\`: expected non-empty string")`

### Phase 6: Integration Tests

- [x] Task 6.1: Integration tests in `tests/retrieval_integration.rs` (AC: 1, 2, 4, 5, 6)
  - [x] 6.1.1: End-to-end: index a fixture repo with Rust file containing functions → call `ApplicationContext::get_symbol()` with valid symbol name → verify `outcome: Success`, `trust: Verified`, `source` matches the expected function source text, provenance populated
  - [x] 6.1.2: End-to-end: `get_symbol` for symbol not in file → verify `InvalidArgument` error with symbol name and file path
  - [x] 6.1.3: End-to-end: `get_symbol` for file not in index → verify `InvalidArgument` error with file path
  - [x] 6.1.4: End-to-end: `get_symbol` against invalidated repo → verify `RequestGated` rejection
  - [x] 6.1.5: End-to-end: `get_symbol` with `kind_filter` → verify correct symbol returned when name is ambiguous
  - [x] 6.1.6: Serialization fidelity: take `ResultEnvelope<VerifiedSourceResponse>` from get_symbol, serialize to JSON, verify JSON contains `outcome`, `trust`, `provenance`, `data.source`, `data.symbol_name`, `data.symbol_kind`, `data.relative_path`, `data.line_range`, `data.byte_range`

### Phase 7: Contract Conformance Tests

- [x] Task 7.1: Extend `tests/retrieval_conformance.rs` (AC: 1, 2)
  - [x] 7.1.1: Conformance test: `VerifiedSourceResponse` is constructable and serializable
  - [x] 7.1.2: Conformance test: `ResultEnvelope<VerifiedSourceResponse>` serializes to JSON with expected field names (`outcome`, `trust`, `provenance`, `data`)
  - [x] 7.1.3: Conformance test: `VerifiedSourceResponse` JSON includes all fields (`relative_path`, `language`, `symbol_name`, `symbol_kind`, `line_range`, `byte_range`, `source`)

## Dev Notes

### Critical Design Decision: Byte-Exact Verified Retrieval

**This is the core trust feature of Tokenizor.** Every byte returned by `get_symbol` MUST be verified against the content-addressable store before being served as trusted output. The verification chain is:

1. **Request gate** (`check_request_gate`) — repo must be healthy, indexed, and not under mutation
2. **Symbol lookup** — exact name match in `FileRecord.symbols`, narrowed by optional `kind_filter`
3. **Blob read** — `blob_store.read_bytes(&record.blob_id)` retrieves raw bytes from CAS
4. **Blob integrity** — `digest_hex(&blob_bytes) == record.blob_id` confirms content hash matches
5. **Span validation** — `symbol.byte_range` must be within blob bounds and `start <= end`
6. **Source extraction** — `blob_bytes[start..end]` produces the exact symbol bytes
7. **UTF-8 conversion** — `String::from_utf8(bytes)` preserves raw fidelity

If ANY step fails, the response uses `outcome: Blocked` + `trust: Suspect` — never silent degradation.

### Verification Pattern (Established in search_text)

Follow the exact blob verification pattern from `search_text_ungated()` in `src/application/search.rs` (lines 118–139):

```rust
use crate::storage::{BlobStore, RegistryPersistence, digest_hex};

// Read blob
let blob_bytes = match blob_store.read_bytes(&record.blob_id) {
    Ok(bytes) => bytes,
    Err(e) => {
        // For search: skip file and continue. For get_symbol: return Blocked.
        return Ok(ResultEnvelope {
            outcome: RetrievalOutcome::Blocked {
                reason: format!("blob read failed for blob_id `{}`: {e}", record.blob_id),
            },
            trust: TrustLevel::Suspect,
            provenance: Some(provenance),
            data: None,
        });
    }
};

// Verify integrity
let computed_hash = digest_hex(&blob_bytes);
if computed_hash != record.blob_id {
    return Ok(ResultEnvelope {
        outcome: RetrievalOutcome::Blocked {
            reason: "blob integrity verification failed: content hash mismatch".to_string(),
        },
        trust: TrustLevel::Suspect,
        provenance: Some(provenance),
        data: None,
    });
}
```

**Key difference from search_text**: In `search_text`, blob failures are item-level (skip file, continue to next). In `get_symbol`, the request targets a single symbol — blob failure is request-fatal and returns a `Blocked` result.

### `digest_hex` Location and Visibility

- Defined in `src/storage/sha256.rs` (line 16): `pub fn digest_hex(bytes: &[u8]) -> String`
- Re-exported in `src/storage/mod.rs`: `pub(crate) use sha256::digest_hex;`
- Imported in `src/application/search.rs` (line 10): `use crate::storage::{BlobStore, RegistryPersistence, digest_hex};`
- Computes SHA-256 hex digest of the input bytes

### Symbol Lookup Semantics

**Exact name matching**: `symbol.name == symbol_name` (not substring). Users discover symbols through `search_symbols` (substring matching) and then use `get_symbol` with the exact name.

**Kind filter disambiguation**: When `kind_filter` is `Some(kind)`, only symbols matching both `name` and `kind` are considered. This resolves common ambiguities like a struct `Foo` and an impl `Foo` in the same file.

**Multiple match resolution**: If multiple symbols match after filtering, select the first by `sort_order` (document order). This is deterministic and repeatable. The response includes `symbol_kind`, `line_range`, and `byte_range` so the caller can verify they got the intended symbol.

### Quarantine Handling: Same as get_file_outline

Quarantined files return `ResultEnvelope { outcome: Quarantined, trust: Quarantined, provenance, data: None }` — consistent with the targeted-request policy established in Story 3.3. This is different from search operations which silently exclude quarantined files.

From Epic 3 execution narrative (Failure Mode Guidance):
> File-level: individual file quarantined → excluded from search results; returned with `trust: quarantined` for targeted outline/retrieval requests.

### VerifiedSourceResponse Schema

```rust
/// Response for get_symbol — verified source text for a single symbol
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiedSourceResponse {
    pub relative_path: String,     // "src/main.rs"
    pub language: LanguageId,      // Rust, Python, etc.
    pub symbol_name: String,       // "main", "HashMap", "impl Default for MyStruct"
    pub symbol_kind: SymbolKind,   // Function, Struct, Impl, etc.
    pub line_range: (u32, u32),    // (start_line, end_line) — 0-based
    pub byte_range: (u32, u32),    // (start_byte, end_byte)
    pub source: String,            // The verified source text (byte-exact, UTF-8)
}
```

### Contract Requirements (Shared Read-Side Contract)

The shared contract types in `src/domain/retrieval.rs` are already implemented. Story 3.5 extends them with `VerifiedSourceResponse` only. No changes to existing contract types.

- **Result envelope**: `ResultEnvelope<VerifiedSourceResponse>` wraps outcome + trust + provenance + data
- **Request-level gating**: Reuse `check_request_gate()` from Story 3.1 — NO changes needed
- **Result-state disambiguation**: Success (verified source) vs NotIndexed vs Quarantined vs Blocked (verification failure)
- **Active-context resolution**: Scoped to `repo_id` parameter

### Request-Fatal vs Item-Local Rules

**Identical to previous stories** — reuse `check_request_gate()` unchanged.

**Request-fatal (entire request fails before any CAS I/O):**
- No active context / unknown `repo_id`
- `RepositoryStatus::Invalidated`
- `RepositoryStatus::Failed`
- Active mutation in progress
- No completed (`Succeeded`) runs

**Request-fatal (same gate as other retrieval operations):**
- `RepositoryStatus::Degraded`

**Item-local (get_symbol specific):**
- File not found in index → `InvalidArgument` error
- File quarantined → `Quarantined` result
- Symbol not found → `InvalidArgument` error
- Blob read failure → `Blocked` result
- Blob integrity mismatch → `Blocked` result
- Byte range invalid → `Blocked` result
- Non-UTF-8 source → `Blocked` result

### Epic 3 Retrieval Architecture Rules (Mandatory)

From project-context.md — non-negotiable for ALL Epic 3 code:

1. **Rule 1 (blob verification)**: **APPLIES** — `get_symbol` returns blob content (source text). Must verify `digest_hex(bytes) == blob_id` before serving. This is the core rule for Story 3.5.
2. **Rule 2 (repo status check)**: Enforced via `check_request_gate()`. Degraded is request-fatal and never returns trusted retrieval.
3. **Rule 3 (provenance)**: Every `VerifiedSourceResponse` wrapped in `ResultEnvelope` with provenance (run_id + committed_at_unix_ms + repo_id).
4. **Rule 4 (disambiguation)**: Success (verified source returned) vs NotIndexed vs Quarantined vs Blocked are distinct outcomes. Symbol/file not found is an explicit error.
5. **Rule 5 (early gating)**: Gate check runs FIRST, before any blob reads or symbol lookups.
6. **Rule 6 (quarantine in search)**: Rule 6 says "must never appear in **search** results." `get_symbol` is a **targeted retrieval** request. Quarantined files are **surfaced with explicit trust state** per the targeted-request policy.

### Scope Boundaries — What Story 3.5 Does NOT Cover

- Blocking/quarantine ACTIONS or repair guidance for verification failures (Story 3.6)
- Batched retrieval / `get_symbols` tool (Story 3.7)
- Line-based or byte-offset code slice retrieval (future enhancement)
- Cross-file or cross-repository symbol retrieval
- Symbol hierarchy/nesting traversal
- Live filesystem change detection
- Write-side operations (re-indexing, repair)
- Freshness policy enforcement (Epic 4/5)

### Existing Read-Side Interfaces from Previous Stories

| Interface | Used in 3.5? | Signature | Notes |
|-----------|-------------|-----------|-------|
| Repo status | YES | `RegistryPersistence::get_repository(&self, repo_id: &str) -> Result<Option<Repository>>` | Gate check |
| Active mutation | YES | `RunManager::has_active_run(&self, repo_id: &str) -> bool` | Gate check |
| Latest completed run | YES | `RegistryPersistence::get_latest_completed_run(&self, repo_id: &str) -> Result<Option<IndexRun>>` | Returns `None` if no Succeeded runs |
| File records by run | YES | `RegistryPersistence::get_file_records(&self, run_id: &str) -> Result<Vec<FileRecord>>` | Find file + symbols |
| Runs by repo | YES (gate) | `RegistryPersistence::get_runs_by_repo(&self, repo_id: &str) -> Result<Vec<IndexRun>>` | NeverIndexed vs NoSuccessfulRuns |
| **Blob lookup** | **YES** | `BlobStore::read_bytes(&self, blob_id: &str) -> Result<Vec<u8>>` | **Core to 3.5** — raw byte retrieval for verification |
| Digest | YES | `crate::storage::digest_hex(bytes: &[u8]) -> String` | SHA-256 hex digest for blob verification |

### MCP Tool Placement Rule

From project-context.md:
> Two separate macro blocks — don't confuse them:
> - `#[tool_router] impl TokenizorServer` — where tools are defined. Add new tools here.
> - `#[tool_handler] impl ServerHandler for TokenizorServer` — connects to the rmcp runtime. Do NOT add tools here.

The `get_symbol` tool goes in the `#[tool_router] impl TokenizorServer` block.

### Parameter Parsing Pattern

Follow the established pattern from Story 3.4. Reuse `parse_kind_filter()` for the optional `kind_filter` parameter:

```rust
struct GetSymbolParams {
    repo_id: String,
    relative_path: String,
    symbol_name: String,
    kind_filter: Option<SymbolKind>,
}

fn required_non_empty_string_param(
    params: &rmcp::model::JsonObject,
    key: &str,
) -> Result<String, McpError> {
    let value = params.get(key).ok_or_else(|| {
        McpError::invalid_params(format!("missing required parameter: {key}"), None)
    })?;
    let value = value.as_str().ok_or_else(|| {
        McpError::invalid_params(
            format!("invalid parameter `{key}`: expected non-empty string"),
            None,
        )
    })?;
    if value.trim().is_empty() {
        return Err(McpError::invalid_params(
            format!("invalid parameter `{key}`: expected non-empty string"),
            None,
        ));
    }
    Ok(value.to_string())
}

fn parse_get_symbol_params(params: &rmcp::model::JsonObject) -> Result<GetSymbolParams, McpError> {
    let repo_id = required_non_empty_string_param(params, "repo_id")?;
    let relative_path = required_non_empty_string_param(params, "relative_path")?;
    let symbol_name = required_non_empty_string_param(params, "symbol_name")?;
    let kind_filter = parse_kind_filter(params)?;

    Ok(GetSymbolParams { repo_id, relative_path, symbol_name, kind_filter })
}
```

**Note:** Empty string validation (`.filter(|s| !s.is_empty())`) was added as a hardening pattern in Story 3.4's review. Apply it here consistently.

### Response Serialization Fidelity

`ResultEnvelope<VerifiedSourceResponse>` serializes to JSON preserving:
- `outcome` — `RetrievalOutcome` variant (snake_case)
- `trust` — `TrustLevel` variant (snake_case)
- `provenance` — `{ run_id, committed_at_unix_ms, repo_id }` (nullable)
- `data` — `Option<VerifiedSourceResponse>` containing `relative_path`, `language`, `symbol_name`, `symbol_kind`, `line_range`, `byte_range`, `source`

### Error Mapping (Already Complete)

`to_mcp_error()` in `src/protocol/mcp.rs` maps all relevant retrieval error variants:

| `TokenizorError` variant | MCP mapping | Trigger in `get_symbol` |
|-------------------------|-------------|---------------------------|
| `RequestGated { gate_error }` | `invalid_params("request gated: {gate_error}")` | Invalidated/failed/active-mutation/not-indexed repo |
| `InvalidArgument(message)` | `invalid_params(message)` | Empty query, file not found, symbol not found |
| `Storage(message)` | `internal_error(message)` | Registry read failure |
| `Serialization(message)` | `internal_error(message)` | JSON serialization failure |

`TokenizorError::RequestGated` and its `to_mcp_error()` mapping are required for explicit gate failures.

### MCP Tool Summary (All tools after 3.5)

After Story 3.5, `TokenizorServer` exposes 12 tools total:

| Tool | Category | Story |
|------|----------|-------|
| `health` | Operational | Epic 1 |
| `index_folder` | Indexing | Epic 2 |
| `get_index_run` | Indexing | 2.5 |
| `list_index_runs` | Indexing | 2.5 |
| `cancel_index_run` | Indexing | 2.7 |
| `checkpoint_now` | Indexing | 2.8 |
| `reindex_repository` | Indexing | 2.9 |
| `invalidate_indexed_state` | Trust | 2.10 |
| `search_text` | Retrieval | 3.4 |
| `search_symbols` | Retrieval | 3.4 |
| `get_file_outline` | Retrieval | 3.4 |
| `get_repo_outline` | Retrieval | 3.4 |
| **`get_symbol`** | **Retrieval** | **3.5** |

Future addition (Story 3.7):
| `get_symbols` | Retrieval | 3.7 |

### Performance Requirements

- **`get_symbol`**: p50 ≤ 150 ms, p95 ≤ 400 ms on representative medium-to-large repositories (warm local index) [Source: epics.md Contract Gaps NFR]
- `get_symbol` involves one blob read + SHA-256 hash + byte range extraction. Should be inherently fast for single-symbol retrieval.
- Basic latency sanity check required before "done".

### Self-Audit Checklist (mandatory before requesting review)

_Run this checklist after all tasks are complete. This is a blocking step — do not request review until every item is verified._

#### Generic Verification
- [x] For every task marked `[x]`, cite the specific test that verifies it
- [x] For every new error variant or branch, confirm a test exercises it
- [x] For every computed value, trace it to where it surfaces (log, return value, persistence)
- [x] For every test, verify the assertion can actually fail (no `assert!(true)`, no conditionals that always pass)

#### Epic 3-Specific Trust Verification
- [x] For every retrieval path, confirm blob_id verification runs BEFORE source is served
- [x] For every verification failure path, confirm `trust: Suspect` and `outcome: Blocked` are returned
- [x] For every "no results" path, confirm the response distinguishes symbol-not-found vs file-not-found vs quarantined vs blocked
- [x] For the request gate, confirm it runs BEFORE any blob reads or symbol lookups

#### Story 3.5-Specific Verification
- [x] Confirm `digest_hex(&blob_bytes) == record.blob_id` check exists before any byte range extraction
- [x] Confirm byte range validation (`start <= end`, `end <= blob_bytes.len()`) exists before slice operation
- [x] Confirm `String::from_utf8()` is used (NOT `from_utf8_lossy`) to preserve fidelity — failure returns Blocked
- [x] Confirm quarantined file handling returns `Quarantined` result (NOT silently excluded, NOT error)
- [x] Confirm `get_symbol` takes `blob_store` parameter (unlike outlines which don't need it)
- [x] Confirm symbol lookup uses exact name match, not substring
- [x] Confirm multiple matches are resolved by `sort_order` (document order)
- [x] Confirm `kind_filter` narrows matches when provided
- [x] Confirm the MCP tool is in `#[tool_router] impl TokenizorServer`, NOT `#[tool_handler]`
- [x] Confirm `to_mcp_error()` includes the `RequestGated` mapping used by retrieval gates
- [x] Confirm the only new `TokenizorError` variant introduced for retrieval is `RequestGated`
- [x] Confirm parameter parsing rejects empty strings for all required parameters
- [x] Confirm server instructions text is updated to mention `get_symbol`
- [x] Confirm latency sanity check test exists

### Testing Requirements

- **Naming**: `test_verb_condition` (e.g., `test_get_symbol_returns_verified_source`)
- **Fakes**: Hand-written fakes inside `#[cfg(test)] mod tests`. Use existing `FakeBlobStore` pattern from search tests. No mock crates.
- **Assertions**: Plain `assert!`, `assert_eq!`. No assertion crates.
- **Test type**: `#[test]` for synchronous tests. `#[tokio::test]` only if async needed.
- **Unit tests**: `#[cfg(test)]` block inside `src/application/search.rs`.
- **MCP tests**: `#[cfg(test)]` block inside `src/protocol/mcp.rs`.
- **Integration tests**: Extend `tests/retrieval_integration.rs`.
- **Conformance tests**: Extend `tests/retrieval_conformance.rs`.
- **Setup**: Follow existing `setup_test_env()` pattern. For `get_symbol` tests, you NEED the `FakeBlobStore` to control blob content.
- **FakeBlobStore pattern** (from `src/application/search.rs` existing tests):
  ```rust
  struct FakeBlobStore {
      blobs: Mutex<HashMap<String, Vec<u8>>>,
  }
  impl BlobStore for FakeBlobStore {
      fn read_bytes(&self, blob_id: &str) -> TResult<Vec<u8>> {
          self.blobs.lock().unwrap().get(blob_id).cloned()
              .ok_or_else(|| TokenizorError::Storage(format!("blob not found: {blob_id}")))
      }
      // ... other methods unreachable!("not used in search tests")
  }
  ```
  **For get_symbol tests:** Store file content in `FakeBlobStore` keyed by `digest_hex(content)`. Create `FileRecord` with matching `blob_id`. Then call `get_symbol` and verify the source extraction.

### Build Order (Mandatory)

1. Domain type `VerifiedSourceResponse` in `src/domain/retrieval.rs` + re-export in `src/domain/mod.rs`
2. `get_symbol()` + `get_symbol_ungated()` in `src/application/search.rs`
3. `ApplicationContext::get_symbol()` wiring in `src/application/mod.rs`
4. `parse_get_symbol_params()` + `get_symbol` MCP tool in `src/protocol/mcp.rs`
5. Server instructions update in `src/protocol/mcp.rs`
6. Unit tests in `src/application/search.rs` (~23 tests)
7. MCP parameter validation tests in `src/protocol/mcp.rs` (~9 tests)
8. Integration tests in `tests/retrieval_integration.rs` (~6 tests)
9. Conformance tests in `tests/retrieval_conformance.rs` (~3 tests)
10. `cargo fmt` + `cargo test` full validation

### Architecture Compliance

- **Layer**: `get_symbol` retrieval logic in `application/`. Domain type in `domain/`. MCP tool in `protocol/`. No layer violations.
- **Persistence model**: All reads via `RegistryPersistence` + `BlobStore`. No SpacetimeDB reads.
- **Error handling**: Use `TokenizorError::RequestGated` for request-fatal repo-state failures and `TokenizorError::InvalidArgument` for item-local request errors. `to_mcp_error()` must map `RequestGated` to MCP `invalid_params`.
- **No Mutex across .await**: `get_symbol` is synchronous (blob read is sync, gate check is sync). No async concerns.
- **No mock crates**: Hand-written `FakeBlobStore` with `Mutex<HashMap>`.
- **No assertion crates**: Plain `assert!`/`assert_eq!`.
- **Import style**: `crate::storage::{BlobStore, RegistryPersistence, digest_hex}` for storage imports. Standard grouping.

### Previous Story Intelligence (Stories 3.1 through 3.4)

**Patterns established that MUST be followed:**
- `check_request_gate()` is the single gate entry point — reuse as-is, NO changes
- Gate returns `TokenizorError::RequestGated` which maps to MCP `invalid_params`
- `Degraded` repos are request-fatal (consistent with project-context.md Rule 2 and the other retrieval operations)
- `ResultEnvelope` pattern with `outcome`, `trust`, `provenance`, `data`
- Defense-in-depth `NotIndexed` check inside ungated function
- Blob verification: `digest_hex(&bytes) == blob_id` before serving content (established in `search_text`)
- Case-sensitive path matching for file lookups
- Quarantine policy: silently exclude from search; surface with trust state for targeted requests
- `serde_json::to_string_pretty()` for MCP JSON responses
- `CallToolResult::success(vec![Content::text(json)])` as MCP return pattern
- `parse_kind_filter()` reuse for optional kind parameter
- Empty-string rejection for required MCP parameters (Story 3.4 hardening)

**Dev agent failure modes to guard against (from retrospectives):**
1. **No-op/sentinel tests**: `assert!(true)` or conditional logic that silently passes. Every test assertion MUST be able to fail.
2. **Adding tools to wrong macro block**: Tools MUST go in `#[tool_router]`, NOT `#[tool_handler]`.
3. **Missing parameter validation**: Every required parameter must have explicit missing-parameter check with clear error message.
4. **Implicit context override**: Tools must NOT accept parameters that redirect queries to arbitrary paths.
5. **Blob verification skip**: NEVER return blob content without verifying blob_id. This is the CORE TRUST CONTRACT.
6. **Weak assertions**: `assert!(result.is_ok())` without checking data contents is insufficient. Verify `source`, `symbol_name`, `trust`, `outcome` etc.
7. **Data computed but dropped**: Ensure all response fields are wired to the response struct, not computed and discarded.

**Previous story completion stats:** Total tests after Story 3.4: 505. Actual Story 3.5 coverage adds 44 tests (23 retrieval unit + 12 MCP param + 6 integration + 3 conformance).

### Project Structure Notes

- New: `src/domain/retrieval.rs` — retrieval contract types including `VerifiedSourceResponse`
- Extended: `src/domain/mod.rs` — re-export retrieval contract types
- New: `src/application/search.rs` — shared retrieval/search implementation including `get_symbol()` and 23 unit tests
- Extended: `src/application/mod.rs` — add `ApplicationContext::get_symbol()` method
- Extended: `src/application/run_manager.rs` — surface active run ids for gated retrieval errors
- Extended: `src/error.rs` — add `TokenizorError::RequestGated`
- Extended: `src/protocol/mcp.rs` — add `get_symbol` MCP tool, `GetSymbolParams`, `parse_get_symbol_params()`, 12 MCP param tests, updated server instructions and MCP error mapping
- New: `tests/retrieval_integration.rs` — add 6 `get_symbol` integration tests
- New: `tests/retrieval_conformance.rs` — add 3 `VerifiedSourceResponse` conformance tests
- NO changes to: `src/storage/` (uses existing `BlobStore::read_bytes` and `digest_hex`)

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story-3.5] — User story, ACs, BDD scenarios
- [Source: _bmad-output/planning-artifacts/epics.md#Epic-3-Execution-Narrative] — Phase 2 (Verified Retrieval), ADRs, failure modes, gating rules
- [Source: _bmad-output/planning-artifacts/epics.md#Failure-Mode-Guidance] — Retrieval verification model, quarantine exclusion rules
- [Source: _bmad-output/planning-artifacts/epics.md#Contract-Gaps] — Raw byte fidelity requirement, get_symbol NFR targets
- [Source: _bmad-output/project-context.md#Epic-3-Retrieval-Architecture] — 6 mandatory retrieval rules (Rule 1 is core to 3.5)
- [Source: _bmad-output/planning-artifacts/architecture.md#Retrieval-Trust-Model] — Trust verification model, byte-exact CAS
- [Source: src/application/search.rs#search_text_ungated] — Blob verification pattern (lines 118–139)
- [Source: src/application/search.rs#check_request_gate] — Request gating function (lines 13–62)
- [Source: src/domain/retrieval.rs] — Contract types (ResultEnvelope, TrustLevel, RetrievalOutcome, Provenance, RequestGateError)
- [Source: src/domain/index.rs] — FileRecord (blob_id, symbols), SymbolRecord (byte_range, line_range), PersistedFileOutcome
- [Source: src/storage/blob.rs] — BlobStore trait (read_bytes, store_bytes)
- [Source: src/storage/sha256.rs#digest_hex] — SHA-256 hex digest function
- [Source: src/storage/mod.rs] — `pub(crate) use sha256::digest_hex`
- [Source: src/protocol/mcp.rs] — MCP tool pattern, parse_kind_filter, to_mcp_error, parameter parsing helpers
- [Source: _bmad-output/implementation-artifacts/3-4-expose-the-full-baseline-retrieval-slice-through-mcp.md] — MCP wiring pattern, parameter validation hardening, parse_kind_filter reuse
- [Source: _bmad-output/implementation-artifacts/3-3-retrieve-file-and-repository-outlines.md] — Quarantine handling for targeted requests, has_symbol_support pattern
- [Source: _bmad-output/implementation-artifacts/3-1-search-indexed-repositories-by-text.md] — Blob verification in search, request gate implementation
- [Source: _bmad-output/implementation-artifacts/3-2-search-indexed-repositories-by-symbol.md] — Symbol search, kind filtering, coverage transparency

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

### Completion Notes List

- All 44 new tests implemented (23 unit + 12 MCP param + 6 integration + 3 conformance) — total suite: 549 tests, 0 failures
- Fixed name collision with Story 3.2 test helpers by using `make_verified_file_record` and `setup_verified_env`
- Fixed missing `checkpoint_cursor_fn` field on `ActiveRun` struct (added in a later story after 3.5 was spec'd)
- Removed unused `VerifiedSourceResponse` import from test module (cleanup)
- Added `TokenizorError::RequestGated` plus the corresponding `to_mcp_error()` mapping; preserved the existing `check_request_gate()` gate behavior.
- Reused `parse_kind_filter()` and `required_non_empty_string_param()` from Story 3.4

### Change Log

| Date | Change |
|------|--------|
| 2026-03-08 | Story 3.5 implemented: verified source retrieval (`get_symbol`) with full verification chain, MCP tool, 44 tests |
| 2026-03-08 | Senior developer review completed: aligned degraded-repo gating documentation, tightened required-parameter validation coverage, corrected implementation metadata, and approved the story |

### File List

- `src/domain/retrieval.rs` — Added `VerifiedSourceResponse` struct
- `src/domain/mod.rs` — Re-exported `VerifiedSourceResponse`
- `src/application/search.rs` — Added `get_symbol()`, `get_symbol_ungated()`, 23 unit tests, and clarified degraded-repo gating via `test_get_symbol_rejects_degraded_repo`
- `src/application/mod.rs` — Added `ApplicationContext::get_symbol()` method, imported `VerifiedSourceResponse`
- `src/application/run_manager.rs` — Added active-run-id support used by gated retrieval errors
- `src/error.rs` — Added `TokenizorError::RequestGated`
- `src/protocol/mcp.rs` — Added `get_symbol` MCP tool, `GetSymbolParams`, `parse_get_symbol_params()`, 12 MCP param tests, updated server instructions, and mapped `RequestGated` at the MCP boundary
- `tests/retrieval_integration.rs` — Added 6 `get_symbol` integration tests including the `ApplicationContext::get_symbol()` end-to-end path
- `tests/retrieval_conformance.rs` — Added 3 `VerifiedSourceResponse` conformance tests

## Senior Developer Review (AI)

### Reviewer

Codex (GPT-5) — 2026-03-08

### Outcome

Approved

### Review Notes

- Initial review found story and implementation drift around degraded-repository gating, MCP required-parameter validation wording, and the recorded implementation/file inventory.
- Fixed the misleading degraded-repo test name so it matches the actual request-gating contract and tightened `get_symbol` MCP validation coverage for empty and non-string required parameters.
- Corrected the story artifact to reflect the real implementation surface, including `TokenizorError::RequestGated`, the supporting `run_manager` changes, and the `ApplicationContext::get_symbol()` end-to-end coverage.
- Re-ran `cargo test get_symbol -- --nocapture` and `cargo test`; all tests passed.
