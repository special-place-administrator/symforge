# Story 2.2: Execute Indexing for the Initial Quality-Focus Language Set

Status: review

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a power user,
I want Tokenizor to execute indexing for the initial quality-focus language set (`Rust`, `Python`, `JavaScript / TypeScript`, and `Go`),
so that the first trusted retrieval slice is implementable at high quality.

## Acceptance Criteria

1. Given a repository contains eligible `Rust`, `Python`, `JavaScript / TypeScript`, or `Go` files
   When an indexing run executes
   Then Tokenizor discovers and processes eligible files under bounded concurrency
   And the run records explicit per-file processing progress within the correct run and repository context

2. Given a repository also contains files outside the initial quality-focus language set
   When language eligibility is evaluated for the run
   Then Tokenizor marks only `Rust`, `Python`, `JavaScript / TypeScript`, and `Go` files as in scope for this story
   And it does not claim indexing support for other languages during this execution slice

3. Given some files fail parsing or extraction during the run
   When processing continues
   Then the affected files are isolated safely
   And the full run is not treated as globally poisoned by a single file failure

## Tasks / Subtasks

- [x] Task 1: Define `LanguageId` enum and file-extension-based detection (AC: 2)
  - [x] Create `LanguageId` enum in `src/domain/index.rs` with variants: `Rust`, `Python`, `JavaScript`, `TypeScript`, `Go`
  - [x] Implement `from_extension(&str) -> Option<LanguageId>` — single source of truth for extension mapping
  - [x] Implement `extensions(&self) -> &[&str]` — returns all recognized extensions per language
  - [x] Implement `support_tier(&self) -> SupportTier` — all five variants return `QualityFocus` for this story
  - [x] Add `SupportTier` enum (`QualityFocus`, `Broader`, `Unsupported`) to future-proof the onboarding pattern (Story 2.4)
  - [x] Unit tests: extension mapping covers `.rs`, `.py`, `.js`, `.jsx`, `.ts`, `.tsx`, `.go`; unknown extensions return `None`

- [x] Task 1.5: Verify tree-sitter grammar crates load on the build platform (AC: 1 — platform gate)
  - [x] Add `tree-sitter`, `tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-javascript`, `tree-sitter-typescript`, `tree-sitter-go`, and `ignore` to `Cargo.toml`
  - [x] Write one integration test per language in `tests/` that creates a `Parser`, calls `set_language(&LANGUAGE.into())`, parses a trivial 1-line source string, and asserts the root node is not null
  - [x] Run `cargo test`. If ANY grammar fails to load (missing .dll/.so, ABI mismatch), STOP and fix the platform issue before proceeding to Task 2
  - [x] This is a hard gate: no parsing code, no discovery code, no pipeline code until all 5 grammars load successfully on Windows/MSYS2

- [x] Task 2: Define `FileProcessingResult`, `FileOutcome`, and `SymbolRecord` domain types (AC: 1, 3)
  - [x] Create `FileProcessingResult` struct: `relative_path: String`, `language: LanguageId`, `outcome: FileOutcome`, `symbols: Vec<SymbolRecord>`, `byte_len: u64`, `content_hash: String`
  - [x] Create `FileOutcome` enum: `Processed`, `PartialParse { warning: String }`, `Failed { error: String }`
  - [x] Create `SymbolRecord` struct: `name: String`, `kind: SymbolKind`, `depth: u32`, `sort_order: u32`, `byte_range: (u32, u32)`, `line_range: (u32, u32)`
  - [x] Create `SymbolKind` enum: `Function`, `Method`, `Class`, `Struct`, `Enum`, `Interface`, `Module`, `Constant`, `Variable`, `Type`, `Trait`, `Impl`, `Other`
  - [x] All types derive `Clone, Debug, Serialize, Deserialize, PartialEq, Eq`
  - [x] Symbols are flat `Vec<SymbolRecord>` with `depth` + `sort_order` for hierarchical reconstruction
  - [x] Unit tests: constructors, serde round-trip

- [x] Task 3: Implement `process_file` pure function (AC: 1, 3)
  - [x] Create `fn process_file(relative_path: &str, bytes: &[u8], language: LanguageId) -> Result<FileProcessingResult>` in `src/parsing/mod.rs`
  - [x] This function is pure: no I/O, no state, no concurrency
  - [x] Convert bytes to UTF-8 (lossy for non-UTF-8 source files)
  - [x] Create tree-sitter `Parser`, set language grammar based on `LanguageId`
  - [x] Parse source, handle `None` (parse failure) as `FileOutcome::Failed`
  - [x] Walk tree nodes to extract `SymbolRecord` entries (functions, classes, structs, etc.)
  - [x] Wrap tree-sitter parsing in `std::panic::catch_unwind` for isolation
  - [x] Handle partial parses (`tree.root_node().has_error()`) as `FileOutcome::PartialParse`
  - [x] Compute `content_hash` via `digest_hex` on the raw bytes
  - [x] Unit tests: parse a small sample file for each of the 5 `LanguageId` variants; test all three `FileOutcome` variants; test panic isolation

- [x] Task 4: Implement tree-sitter symbol extraction per language (AC: 1)
  - [x] Create `src/parsing/languages/` directory with `mod.rs`, `rust.rs`, `python.rs`, `javascript.rs`, `typescript.rs`, `go.rs`
  - [x] Each language module implements `fn extract_symbols(node: &tree_sitter::Node, source: &str) -> Vec<SymbolRecord>`
  - [x] Rust: extract `fn`, `struct`, `enum`, `trait`, `impl`, `const`, `static`, `mod`, `type`
  - [x] Python: extract `def`, `class`, `async def`
  - [x] JavaScript: extract `function`, `class`, `const/let/var` (top-level), `export`
  - [x] TypeScript: extract same as JavaScript plus `interface`, `type`, `enum`
  - [x] Go: extract `func`, `type`, `struct`, `interface`, `const`, `var`
  - [x] Assign `depth` and `sort_order` during tree walk for hierarchical reconstruction
  - [x] Integration tests: parse a real sample file per language, verify symbol names and kinds

- [x] Task 5: Implement file discovery using `ignore` crate (AC: 1, 2)
  - [x] Create `src/indexing/discovery.rs` with `fn discover_files(root: &Path) -> Result<Vec<DiscoveredFile>>`
  - [x] `DiscoveredFile` struct: `relative_path: String`, `absolute_path: PathBuf`, `language: LanguageId`
  - [x] Use `ignore::WalkBuilder` for `.gitignore`-respecting traversal
  - [x] Filter discovered files by `LanguageId::from_extension` — only include files with recognized extensions
  - [x] Sort results by normalized relative path (forward slashes, lowercase on Windows) for deterministic order
  - [x] Unit tests: discover files in a temp directory with mixed languages; verify gitignore exclusions

- [x] Task 6: Implement indexing pipeline orchestrator with bounded concurrency (AC: 1, 3)
  - [x] Create `src/indexing/pipeline.rs` with the main `IndexingPipeline` struct
  - [x] Accept `run_id`, `repo_root: PathBuf`, concurrency cap (default ~8 or `num_cpus`)
  - [x] Use `tokio::sync::Semaphore` for bounded `tokio::spawn` concurrency
  - [x] Pipeline flow: discover files → sort → for each file: acquire permit → read bytes → `process_file` → record result → release permit
  - [x] Each task is self-contained: a single file failing records degraded status, releases permit, continues
  - [x] Implement consecutive-failure circuit breaker: if N consecutive tasks fail (default N=5), abort with `Aborted` status; counter resets on success
  - [x] Track in-memory progress: `Arc<AtomicU64>` for files_processed, files_failed, total_files
  - [x] Transition run status: `Queued` → `Running` (at pipeline start) → `Succeeded`/`Failed`/`Aborted` (at pipeline end)
  - [x] Never hold `std::sync::Mutex` across `.await`
  - [x] Unit tests: pipeline processes files, circuit breaker triggers after N consecutive failures, progress tracking

- [x] Task 7: Wire pipeline into `RunManager` background task spawning (AC: 1)
  - [x] Extend `RunManager::start_run()` or add `RunManager::launch_run()` to spawn the indexing pipeline as a background `tokio::spawn` task
  - [x] Create `ActiveRun` with `JoinHandle`, `CancellationToken`, and progress `Arc`
  - [x] Register via existing `register_active_run()` method
  - [x] Background task: run pipeline → update run status via `RegistryPersistence` → deregister active run on completion
  - [x] `index_folder` MCP tool: call `launch_run()` instead of just `start_run()`, return `run_id` immediately
  - [x] Ensure tool handler never `.await`s the full pipeline

- [x] Task 8: Add comprehensive tests (AC: 1, 2, 3)
  - [x] `LanguageId`: extension mapping, support tier, exhaustive coverage
  - [x] `process_file`: sample file per language, all `FileOutcome` variants, panic isolation
  - [x] Tree-sitter integration: parse small sample files per language, verify grammar loads correctly (catches .dll/.so load failures on Windows)
  - [x] Discovery: gitignore exclusion, language filtering, deterministic sort order
  - [x] Pipeline: end-to-end with temp repo, bounded concurrency, circuit breaker, progress tracking
  - [x] Run lifecycle: `Queued` → `Running` → `Succeeded`/`Failed` transitions via background task
  - [x] Error isolation: single file failure doesn't poison the run

## Dev Notes

> **CRITICAL: Before implementing, load `_bmad-output/project-context.md` in full.** It contains 87 agent rules scoped to Epic 2 covering persistence architecture, type design, concurrency, error handling, testing, and anti-patterns. Every rule applies to this story. Do not start coding without reading it.

### Story Requirements

- This story builds the core indexing execution pipeline on top of the run identity infrastructure from Story 2.1.
- Scope is **execution and in-memory progress** only. Durable persistence of `FileRecord` and `SymbolRecord` to the registry and CAS blob storage is Story 2.3. Do NOT implement file/symbol persistence here.
- The `process_file` function is a **pure function** — no I/O, no state, no concurrency. Build and test this FIRST before any orchestration. If the agent builds the orchestrator first, they'll be debugging parsing logic inside concurrent async code.
- The pipeline produces `Vec<FileProcessingResult>` in memory. Story 2.3 will consume these results to produce durable `FileRecord` entries with `blob_id` references.
- `LanguageId` is the **single source of truth** for language detection. The compiler enforces exhaustive handling. Never hardcode extensions in multiple places.
- Story 2.2 does NOT implement checkpointing (Story 2.8), cancellation response (Story 2.7), or live progress exposure (Story 2.6). The pipeline runs to completion or aborts.

### Build Order (Critical)

1. **Domain types first:** `LanguageId`, `SupportTier`, `FileProcessingResult`, `FileOutcome`, `SymbolRecord`, `SymbolKind` (Task 1)
2. **Platform gate second:** Add grammar crate deps to `Cargo.toml`, write one "grammar loads" integration test per language, run `cargo test`. If ANY grammar fails to load on Windows/MSYS2, STOP and fix before proceeding. (Task 1.5)
3. **More domain types:** `FileProcessingResult`, `FileOutcome`, `SymbolRecord`, `SymbolKind` (Task 2)
4. **Pure parsing:** `process_file` + per-language extractors (Tasks 3-4)
5. **Discovery:** File walking with `ignore` crate (Task 5)
6. **Pipeline orchestration:** Bounded concurrency, circuit breaker, progress (Task 6)
7. **Wiring last:** Connect pipeline to `RunManager` background task (Task 7)

Do NOT skip ahead. Each layer depends on the previous one being tested and working. Task 1.5 is a **hard gate** — if tree-sitter grammars don't produce working native libraries on the build platform, everything after it is wasted work.

### Current Implementation Baseline

- `src/indexing/mod.rs` has empty `IndexingScaffold` placeholder — replace with real pipeline code
- `src/parsing/mod.rs` has empty `ParsingScaffold` placeholder — replace with real parsing code
- `src/domain/index.rs` has `IndexRun`, `IndexRunStatus`, `IndexRunMode`, `Checkpoint` — extend with new types
- `src/application/run_manager.rs` has `RunManager` with `start_run()`, `register_active_run()`, `ActiveRun` struct — extend with `launch_run()`
- `src/storage/sha256.rs` has `digest_hex` for content hashing
- `src/domain/health.rs` has `unix_timestamp_ms()` for timestamps
- `src/protocol/mcp.rs` has `index_folder` MCP tool — update to call `launch_run()` instead of `start_run()`
- Current test baseline: **99 tests** (96 library + 3 binary). Do not drop below this count.

### Developer Context

- **`FileProcessingResult` contains extracted symbols and file metadata, NOT blob storage outcomes.** The orchestrator stores bytes in CAS separately (Story 2.3), then combines the `blob_id` with the processing result to produce the final `FileRecord`. For Story 2.2, just produce the `FileProcessingResult`.
- **`FileProcessingResult` carries an explicit `FileOutcome` enum.** Variants: `Processed`, `PartialParse { warning }`, `Failed { error }`. Symbols are empty for `Failed`, possibly incomplete for `PartialParse`. Every consumer must handle all three variants.
- **Two failure domains require opposite responses.** File-local errors (parse, encoding): isolate and continue. Systemic errors (disk, CAS root, registry): abort immediately. Use `TokenizorError::is_systemic()` (implemented in Story 2.1) to classify.
- **Consecutive-failure circuit breaker.** If N consecutive file tasks fail (default N=5), abort with explicit `Aborted` status. Counter resets on any successful file.
- **Content-addressed storage is self-healing for mid-edit races.** No file-locking or snapshot-at-discovery needed. Do NOT add complexity here.
- **Active run progress lives in-memory.** Use shared atomic counters (`Arc<AtomicU64>`). Durable checkpoints live on disk (Story 2.8). Two separate read paths.

### Project Structure Notes

- `src/parsing/mod.rs` — replace scaffold with `process_file` function and language dispatch
- `src/parsing/languages/` — new directory with `mod.rs` + per-language extraction modules (`rust.rs`, `python.rs`, `javascript.rs`, `typescript.rs`, `go.rs`)
- `src/indexing/mod.rs` — replace scaffold with pipeline module organization
- `src/indexing/discovery.rs` — new file for gitignore-aware file discovery
- `src/indexing/pipeline.rs` — new file for the bounded-concurrency indexing orchestrator
- `src/domain/index.rs` — extend with `LanguageId`, `SupportTier`, `FileProcessingResult`, `FileOutcome`, `SymbolRecord`, `SymbolKind`
- This codebase uses `mod.rs` style exclusively. Do NOT introduce `module_name.rs` + `module_name/` directory style.
- Each `mod.rs` re-exports public types. `lib.rs` re-exports the top-level public API.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 2.2: Execute Indexing for the Initial Quality-Focus Language Set]
- [Source: _bmad-output/planning-artifacts/epics.md#Epic 2: Durable Indexing and Run Control]
- [Source: _bmad-output/planning-artifacts/prd.md#FR9]
- [Source: _bmad-output/planning-artifacts/architecture.md#Indexing Module Responsibility (line 501)]
- [Source: _bmad-output/planning-artifacts/architecture.md#Parsing Architecture (line 502)]
- [Source: _bmad-output/planning-artifacts/architecture.md#Concurrency Rules (line 554)]
- [Source: _bmad-output/planning-artifacts/architecture.md#Language Support Structure (line 720-735)]
- [Source: _bmad-output/project-context.md#Indexing Pipeline Architecture]
- [Source: _bmad-output/project-context.md#Tree-sitter Rules]
- [Source: _bmad-output/project-context.md#Epic 2 Type Design]
- [Source: _bmad-output/project-context.md#MCP Server & Run Management]
- [Source: _bmad-output/project-context.md#Critical Don't-Miss Rules]
- [Source: _bmad-output/implementation-artifacts/2-1-start-an-indexed-run-with-durable-run-identity.md]
- [Source: src/domain/index.rs — IndexRun, IndexRunStatus, IndexRunMode, Checkpoint]
- [Source: src/application/run_manager.rs — RunManager, ActiveRun, start_run(), register_active_run()]
- [Source: src/indexing/mod.rs — IndexingScaffold (placeholder)]
- [Source: src/parsing/mod.rs — ParsingScaffold (placeholder)]
- [Source: src/storage/sha256.rs — digest_hex]
- [Source: src/domain/health.rs — unix_timestamp_ms]
- [Source: Cargo.toml — current dependency set]

## Technical Requirements

- **Rust Edition 2024.** `gen` is reserved. `unsafe_op_in_unsafe_fn` enforced. Do not assume 2021-era closure capture behavior.
- **Error boundary:** `TokenizorError` (thiserror 2.0) inside all library code. `anyhow::Result` only in `main.rs`. Add new `TokenizorError` variants as needed (e.g., `Parse`, `Discovery`).
- **All lifecycle states are exhaustive enums.** `LanguageId`, `SupportTier`, `FileOutcome`, `SymbolKind` — never use raw strings for state.
- **All timestamps use `u64` millis via `unix_timestamp_ms()`.** No chrono, no f64, no direct SystemTime.
- **`ControlPlane` trait methods are synchronous.** Async boundary lives in the application layer.
- **MCP tools are non-blocking launchers.** `index_folder` spawns background task, returns `run_id` immediately. Tool handlers never `.await` the full pipeline.
- **`RunManager` is `Arc`-wrapped.** It holds `JoinHandle` (not `Clone`). It is the deliberate exception to the short-lived service pattern.
- **Never hold `std::sync::Mutex` across `.await`.** Acquire, extract, drop guard, then await.
- **Bounded `tokio::spawn` with `tokio::sync::Semaphore`.** Default cap ~8 or `num_cpus`. No `rayon` (blocks async runtime).
- **Expand `to_mcp_error()` for each new `TokenizorError` variant.**

## Architecture Compliance

- Preserve the layered flow: `main.rs` → `application` → `domain/storage/indexing/parsing`.
- `domain` defines core entities (`LanguageId`, `FileProcessingResult`, `SymbolRecord`), value types, invariants.
- `parsing` owns tree-sitter bindings, extraction, and parse-specific translation. `process_file` lives here.
- `indexing` owns discovery, pipeline coordination. Does NOT own parsing logic.
- `application` orchestrates use cases — `RunManager` spawns the pipeline.
- `storage` implements persistence boundaries — NOT modified in this story.
- `protocol` adapts external surfaces (MCP/CLI) — minimal changes (update `index_folder` to call `launch_run`).

## Library / Framework Requirements

- **Add `tree-sitter = "0.24"`** — core parsing library
- **Add `tree-sitter-rust = "0.23"`** — Rust grammar
- **Add `tree-sitter-python`** — Python grammar (use latest compatible with tree-sitter 0.24)
- **Add `tree-sitter-javascript`** — JavaScript grammar (use latest compatible with tree-sitter 0.24)
- **Add `tree-sitter-typescript`** — TypeScript/TSX grammar (use latest compatible with tree-sitter 0.24)
- **Add `tree-sitter-go`** — Go grammar (use latest compatible with tree-sitter 0.24)
- **Add `ignore = "0.4"`** — gitignore-aware file walking (from ripgrep project)
- **Add `num_cpus = "1.16"`** — for default concurrency cap
- **Pin core tree-sitter + grammar crate versions together.** Version matrix mismatches cause build errors or runtime ABI crashes. Verify all grammars compile against tree-sitter 0.24 before committing.
- **Tree-sitter API (0.24):** Use `parser.set_language(&tree_sitter_rust::LANGUAGE.into())` — NOT the older `tree_sitter_rust::language()` style.
- **Tree-sitter nodes are borrowed, not owned.** Extract into owned domain types (`SymbolRecord`) immediately during the parse walk. Never store `Node` or `Tree` beyond the parse function scope.
- Do not add `uuid` — use `digest_hex` for deterministic ID generation.
- Stay on current dependency set for everything else.

## File Structure Requirements

**Files to create:**
- `src/parsing/languages/mod.rs` — language module organization and dispatch
- `src/parsing/languages/rust.rs` — Rust symbol extraction
- `src/parsing/languages/python.rs` — Python symbol extraction
- `src/parsing/languages/javascript.rs` — JavaScript symbol extraction
- `src/parsing/languages/typescript.rs` — TypeScript symbol extraction
- `src/parsing/languages/go.rs` — Go symbol extraction
- `src/indexing/discovery.rs` — gitignore-aware file discovery
- `src/indexing/pipeline.rs` — bounded-concurrency indexing orchestrator

**Files to modify:**
- `src/domain/index.rs` — add `LanguageId`, `SupportTier`, `FileProcessingResult`, `FileOutcome`, `SymbolRecord`, `SymbolKind`
- `src/parsing/mod.rs` — replace `ParsingScaffold` with `process_file` function and `pub mod languages`
- `src/indexing/mod.rs` — replace `IndexingScaffold` with `pub mod discovery`, `pub mod pipeline`
- `src/application/run_manager.rs` — add `launch_run()` method, wire pipeline spawning
- `src/application/mod.rs` — update `start_indexing` to use `launch_run`
- `src/protocol/mcp.rs` — update `index_folder` tool to call `launch_run()`
- `src/error.rs` — add new `TokenizorError` variants if needed (e.g., `Parse`, `Discovery`)
- `src/lib.rs` — ensure new modules/types are re-exported as needed
- `Cargo.toml` — add tree-sitter crates, `ignore`, `num_cpus`

**Files NOT to modify:**
- `src/storage/control_plane.rs` — not in scope (no persistence changes)
- `src/storage/registry_persistence.rs` — only minimal changes for run status updates (already has `update_run_status`)
- `src/main.rs` — should not need changes (wiring goes through `ApplicationContext`)

## Testing Requirements

- **Unit tests (co-located in modules):**
  - `LanguageId`: extension mapping, support tier, all variants
  - `FileProcessingResult` / `FileOutcome` / `SymbolRecord`: serde round-trip, construction
  - `process_file`: parse sample source for each language, all three `FileOutcome` variants, panic isolation via `catch_unwind`
  - Discovery: language filtering, deterministic sort, gitignore respect
  - Pipeline: circuit breaker triggers after N consecutive failures, counter resets on success

- **Integration tests (`tests/` at crate root):**
  - Tree-sitter grammar verification: parse a small sample file (5-line) per language to catch .dll/.so load failures on Windows
  - End-to-end pipeline: create temp repo with mixed-language files, run pipeline, verify correct files discovered and processed
  - Run lifecycle: `Queued` → `Running` → `Succeeded` transition via background task

- **Test naming:** `test_verb_condition` (e.g., `test_process_file_extracts_rust_symbols`)
- **Fakes:** Hand-written fakes with `AtomicUsize` call counters. No mock crates.
- **Assertions:** Plain `assert!`, `assert_eq!`. No assertion crates.
- **`#[test]` by default.** `#[tokio::test]` only for `async fn` tests.
- **Tests use temp directories** for discovery and pipeline tests.
- **Baseline: 99 tests.** Must not drop below. Target adding ~25-35 new tests.

## Previous Story Intelligence

- **Story 2.1** implemented `RegistryPersistence`, `RunManager`, `ActiveRun`, idempotency checking, startup sweep. Key learnings:
  - Agent model: Claude Opus 4.6. Code review caught H1 (silent success on nonexistent `run_id`) and M2 (missing test).
  - `ActiveRun` struct and `register_active_run()` are scaffolding from 2.1 — this story exercises them.
  - `start_run()` creates Queued run but does NOT spawn background task — `launch_run()` is the gap to fill.
  - rmcp tool parameter syntax: `#[tool(param)]` attribute is NOT supported on method parameters. Use `rmcp::model::JsonObject` with manual parsing.
  - `RegistrySnapshot` in `init.rs` is private — `RegistryPersistence` uses its own `RegistryData` superset type.
  - Test count went from 59 → 99 (40 new tests, 0 regressions).

- **Story 1.7** (last Epic 1 story): Production `expect()` calls flagged — use proper error propagation. Large functions (400+ lines) decomposed during review. `unsafe` Windows FFI needs `// SAFETY:` comments.

- **Epic 1 retrospective** recommended interim registry persistence (no SpacetimeDB coupling).

## Git Intelligence Summary

Recent commits:
- `0c54f13` feat: implement Story 2.1 — durable run identity
- `759734c` docs: create Story 2.1 — durable run identity
- `ff7d8f6` chore: add missing validate-workflow.xml BMAD task
- `b47ba86` chore: regenerate sprint-status.yaml with all 5 epics and 39 stories
- `9c70010` docs: add project-context.md and interim persistence ADR
- `b539b76` feat: complete Epic 1 — Reliable Local Setup and Workspace Identity

Key insights:
- Story 2.1 is the immediate predecessor — all infrastructure (`RunManager`, `RegistryPersistence`, `ActiveRun`) is fresh.
- Tree-sitter and `ignore` crate are NOT in `Cargo.toml` yet — this story adds them as new dependencies.
- The indexing and parsing modules are empty scaffolds — greenfield implementation.

## Latest Technical Information

- **`tree-sitter` crate v0.24:** Latest stable Rust binding. API change: use `parser.set_language(&LANGUAGE.into())` not the older `language()` function. `Parser::new()`, `.parse(source, None)`, `.root_node()`.
- **`tree-sitter-rust` v0.23:** Rust grammar. Exposes `LANGUAGE` constant for tree-sitter 0.24 API.
- **`tree-sitter-python`, `tree-sitter-javascript`, `tree-sitter-typescript`, `tree-sitter-go`:** Use latest versions compatible with tree-sitter 0.24. Verify build on Windows — native C dependencies require both compile AND runtime verification.
- **`ignore` crate v0.4:** Provides `Walk`, `WalkBuilder`, `WalkParallel`. Respects `.gitignore`, `.git/info/exclude`, global gitignore. Use `WalkBuilder::new(root)` for single-threaded walk.
- **Rust Edition 2024:** `gen` keyword is reserved (cannot use as identifier). `unsafe_op_in_unsafe_fn` lint enforced by default.
- **Windows/MSYS2:** Tree-sitter grammar .dll loading must be verified at runtime, not just compile time. Include integration tests that parse sample files.
- **`std::panic::catch_unwind`:** Preferred for tree-sitter parse isolation. CPU-bound sync work. Communicates isolation intent explicitly. Record panicked files as `Failed`, continue the run.

## Project Context Reference

- Full project context for AI agents at `_bmad-output/project-context.md` (87 rules, Epic 2 scoped).
- Key architectural decisions: ADR-1 through ADR-7 documented in project-context.md.
- **Build order guidance:** Build `process_file` first (pure function), then layer orchestration on top. This is the #1 rule for this story.
- **Anti-pattern #9 (most likely agent violation for this story):** Adding language file extensions in multiple places instead of the central `LanguageId` enum. All extension handling goes through `LanguageId::from_extension`.

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- tree-sitter 0.24 API: `Node::is_null()` removed — use `node.kind().is_empty()` for validity checks
- `SymbolKind` needed `Copy` derive since it's used after pattern matching in tree walks
- `ignore` crate requires a `.git/` directory to respect `.gitignore` in tests
- tree-sitter-typescript exposes `LANGUAGE_TYPESCRIPT` (not `LANGUAGE`) for TypeScript grammar

### Completion Notes List

- Implemented full indexing pipeline from domain types through background task spawning
- All 5 quality-focus languages (Rust, Python, JavaScript, TypeScript, Go) parse and extract symbols
- Pipeline uses bounded concurrency via `tokio::sync::Semaphore`, consecutive-failure circuit breaker
- `process_file` is a pure function with `catch_unwind` isolation — safe against tree-sitter panics
- File discovery respects `.gitignore`, normalizes paths with forward slashes, sorts deterministically
- `RunManager::launch_run()` spawns background task, transitions `Queued→Running→Succeeded/Failed/Aborted`
- MCP `index_folder` tool updated to accept `repo_root` parameter and call `launch_indexing()`
- Added `Aborted` variant to `IndexRunStatus` for circuit breaker scenarios
- Test count: 99 → 142 (43 new tests, 0 regressions)

### Change Log

- 2026-03-07: Implemented Story 2.2 — full indexing pipeline for quality-focus language set

### File List

**New files:**
- `src/parsing/languages/mod.rs` — language dispatch for symbol extraction
- `src/parsing/languages/rust.rs` — Rust symbol extraction
- `src/parsing/languages/python.rs` — Python symbol extraction
- `src/parsing/languages/javascript.rs` — JavaScript symbol extraction
- `src/parsing/languages/typescript.rs` — TypeScript symbol extraction
- `src/parsing/languages/go.rs` — Go symbol extraction
- `src/indexing/discovery.rs` — gitignore-aware file discovery
- `src/indexing/pipeline.rs` — bounded-concurrency indexing pipeline
- `tests/tree_sitter_grammars.rs` — grammar load verification (integration)
- `tests/indexing_integration.rs` — run lifecycle and error isolation (integration)

**Modified files:**
- `Cargo.toml` — added tree-sitter, tree-sitter-{rust,python,javascript,typescript,go}, ignore, num_cpus
- `src/domain/index.rs` — added LanguageId, SupportTier, FileProcessingResult, FileOutcome, SymbolRecord, SymbolKind, Aborted status
- `src/domain/mod.rs` — re-export new types
- `src/parsing/mod.rs` — replaced ParsingScaffold with process_file and languages module
- `src/indexing/mod.rs` — replaced IndexingScaffold with discovery and pipeline modules
- `src/application/run_manager.rs` — added launch_run(), deregister_active_run()
- `src/application/mod.rs` — added launch_indexing() method
- `src/protocol/mcp.rs` — updated index_folder to accept repo_root and use launch_indexing
- `src/storage/registry_persistence.rs` — added update_run_status_with_finish()
