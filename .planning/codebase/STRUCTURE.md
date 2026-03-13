# Codebase Structure

**Analysis Date:** 2026-03-14

## Directory Layout

```
tokenizor_agentic_mcp/
├── src/                           # Rust source code (36,099 LOC)
│   ├── main.rs                    # Entry point: CLI dispatch, MCP/daemon/hook routing
│   ├── lib.rs                     # Public module exports
│   ├── cli/                       # Command-line interface
│   │   ├── mod.rs                 # Clap CLI definition
│   │   ├── init.rs                # Tool initialization command
│   │   └── hook.rs                # Hook subcommand handlers
│   ├── discovery/                 # File discovery and project root detection
│   │   └── mod.rs                 # discover_files(), find_project_root()
│   ├── domain/                    # Core data types
│   │   ├── mod.rs                 # Re-exports
│   │   └── index.rs               # LanguageId, SymbolKind, SymbolRecord, ReferenceRecord, FileClassification
│   ├── parsing/                   # Tree-sitter based parsing
│   │   ├── mod.rs                 # process_file(), entry point for parsing pipeline
│   │   ├── xref.rs                # Cross-reference extraction
│   │   └── languages/             # Language-specific parsers (15 languages)
│   │       ├── mod.rs             # Language dispatcher
│   │       ├── rust.rs            # Rust grammar plugin
│   │       ├── python.rs          # Python grammar plugin
│   │       ├── javascript.rs      # JavaScript grammar plugin
│   │       ├── typescript.rs      # TypeScript grammar plugin
│   │       ├── go.rs              # Go grammar plugin
│   │       ├── java.rs            # Java grammar plugin
│   │       ├── c.rs, cpp.rs       # C/C++ grammar plugins
│   │       ├── csharp.rs          # C# grammar plugin
│   │       ├── ruby.rs, php.rs, swift.rs, perl.rs, kotlin.rs, dart.rs, elixir.rs
│   ├── live_index/                # In-memory symbol index
│   │   ├── mod.rs                 # Public re-exports
│   │   ├── store.rs               # LiveIndex, IndexedFile, ParseStatus, CircuitBreakerState, SharedIndex
│   │   ├── query.rs               # Query functions and view types (RepoOutlineView, SymbolDetailView, etc.)
│   │   ├── search.rs              # Trigram-based search (SearchIndex)
│   │   ├── persist.rs             # Serialization (postcard binary) and snapshot loading
│   │   ├── git_temporal.rs        # Git change frequency computation (Phase 5)
│   │   └── trigram.rs             # Trigram index implementation
│   ├── protocol/                  # MCP protocol implementation
│   │   ├── mod.rs                 # TokenizorServer, MCP server initialization
│   │   ├── tools.rs               # 24 tool handlers: get_symbol, search_symbols, analyze_file_impact, etc.
│   │   ├── format.rs              # Plain-text formatters for query results
│   │   ├── edit.rs                # Symbolic edit operations (InsertSymbol, DeleteSymbol, etc.)
│   │   ├── edit_format.rs         # Edit result formatting
│   │   ├── explore.rs             # Explore tool helpers
│   │   ├── resources.rs           # MCP resource handlers (tokenizor:// URIs)
│   │   └── prompts.rs             # Prompt definitions
│   ├── sidecar/                   # HTTP companion server for hook handlers
│   │   ├── mod.rs                 # SidecarHandle, TokenStats, SidecarState
│   │   ├── server.rs              # Axum server startup (spawn_sidecar)
│   │   ├── handlers.rs            # HTTP handlers: read, edit, write, grep, stats
│   │   ├── router.rs              # Axum route definitions
│   │   └── port_file.rs           # Port/PID file management
│   ├── daemon.rs                  # Shared daemon for project-level indexing
│   │   └── DaemonState, DaemonSessionClient, HTTP endpoints (/api/projects/open, /api/tool/*)
│   ├── watcher/                   # File system monitoring
│   │   └── mod.rs                 # run_watcher(), file change detection and re-parsing
│   ├── git.rs                     # Git utilities (placeholder for git2 operations)
│   ├── hash.rs                    # Content hashing (digest_hex for .tokenizor snapshots)
│   ├── error.rs                   # Error types
│   └── observability.rs           # Logging initialization (tracing)
├── npm/                           # NPM package wrapper for Rust binary
│   ├── package.json               # Node.js package definition
│   ├── index.js                   # Binary location wrapper
│   └── preinstall.js              # Build hook (cargo build)
├── Cargo.toml                     # Rust project manifest, dependencies, features
├── Cargo.lock                     # Locked dependency versions
├── .tokenizor/                    # Runtime index directory (generated)
│   ├── index.bin                  # Persisted snapshot (postcard format)
│   ├── sidecar.port               # Ephemeral sidecar port
│   ├── daemon.port                # Daemon HTTP port
│   ├── daemon.pid                 # Daemon process ID
│   └── daemon.starting            # Lock file during daemon startup
├── .github/                       # GitHub workflows
│   └── workflows/                 # CI/CD pipeline (rustfmt, test, build, npm publish)
├── docs/                          # Documentation and analysis
├── .claude/                       # Claude Code metadata
├── scripts/                       # Utility scripts
├── target/                        # Cargo build artifacts (generated)
└── [project-root-files]
    ├── Cargo.toml
    ├── Cargo.lock
    ├── CLAUDE.md                  # Project directives for Claude
    ├── CHANGELOG.md               # Release history
    ├── README.md                  # Project overview
    ├── LICENSE                    # MIT license
    └── .gitignore                 # Git exclusions
```

## Directory Purposes

**`src/`:**
- Purpose: All Rust source code for the MCP server
- Contains: Main entry point, library modules, protocol implementation, indexing logic
- Key files: `main.rs`, `lib.rs`

**`src/cli/`:**
- Purpose: Command-line interface and subcommands
- Contains: Clap CLI definition, init logic, hook handlers
- Key files: `mod.rs` (Cli struct)

**`src/discovery/`:**
- Purpose: File system discovery and project root detection
- Contains: Gitignore-respecting file walker, safe project root finder
- Key files: `mod.rs` (discover_files, find_project_root functions)

**`src/domain/`:**
- Purpose: Core data types shared across all layers
- Contains: LanguageId, SymbolKind, SymbolRecord, ReferenceRecord, FileClassification
- Key files: `index.rs` (domain type definitions)

**`src/parsing/`:**
- Purpose: Tree-sitter based parsing for symbol extraction
- Contains: Language-specific parser modules (15+ languages), cross-reference extraction
- Key files: `mod.rs` (process_file entry point), `languages/mod.rs` (dispatcher), `xref.rs` (reference extraction)

**`src/parsing/languages/`:**
- Purpose: Language-specific tree-sitter implementations
- Contains: One module per supported language (rust.rs, python.rs, typescript.rs, etc.)
- Pattern: Each module exposes `extract_symbols(tree, text)` and handles language-specific syntax

**`src/live_index/`:**
- Purpose: In-memory symbol database and query APIs
- Contains: LiveIndex store, search indices, persistence, query formatters
- Key files: `store.rs` (LiveIndex, IndexedFile), `query.rs` (query functions), `search.rs` (trigram search)

**`src/protocol/`:**
- Purpose: MCP protocol implementation and tool definitions
- Contains: 24 tool handlers, prompt definitions, resource handlers, response formatters
- Key files: `tools.rs` (tool handlers), `format.rs` (text output formatting), `edit.rs` (symbolic edits)

**`src/sidecar/`:**
- Purpose: HTTP companion server for hook handlers and token statistics
- Contains: Axum HTTP endpoints, token stats tracking, port file management
- Key files: `server.rs` (Axum setup), `handlers.rs` (HTTP handler logic)

**`.tokenizor/`:**
- Purpose: Runtime state directory (created on first run)
- Contains: Serialized index snapshots, port files for daemon/sidecar discovery
- Generated: Yes, auto-created at startup
- Committed: No (in .gitignore)

## Key File Locations

**Entry Points:**
- `src/main.rs` — Main process entry, CLI dispatch, MCP/daemon startup
- `src/sidecar/server.rs:spawn_sidecar()` — HTTP server entry
- `src/watcher/mod.rs:run_watcher()` — File watcher loop
- `src/live_index/git_temporal.rs:spawn_git_temporal_computation()` — Background git analysis

**Configuration:**
- `Cargo.toml` — Rust dependencies, features (v1 feature gate), package metadata
- `.env.example` — Environment variable template (TOKENIZOR_AUTO_INDEX, TOKENIZOR_CB_THRESHOLD, etc.)
- `.github/workflows/` — CI/CD pipeline definitions

**Core Logic:**
- `src/live_index/store.rs` — Main symbol storage and mutation logic
- `src/live_index/query.rs` — All query functions (get_file, get_symbol, find_references, etc.)
- `src/protocol/tools.rs` — MCP tool handler implementations (24 tools)
- `src/sidecar/handlers.rs` — Hook response builders (read, edit, write, grep)

**Testing:**
- Inline `#[cfg(test)] mod tests` blocks in most modules
- No separate `tests/` directory; tests live alongside code
- Key test locations:
  - `src/live_index/store.rs` tests circuit breaker
  - `src/discovery/mod.rs` tests file discovery, path normalization
  - `src/main.rs` tests startup index logging
  - `src/parsing/mod.rs` tests for each language

## Naming Conventions

**Files:**
- Module files: `lowercase_with_underscores.rs` (e.g., `parse_file.rs`, `get_symbol.rs`)
- Submodule directories: `lowercase_with_underscores/` containing `mod.rs`
- Special: `mod.rs` is the module entry point (pub use, re-exports)

**Functions:**
- Public: `snake_case` (e.g., `discover_files()`, `get_file_outline()`)
- Internal: `snake_case` prefixed with `_` or in `impl` blocks (e.g., `_handle_write()`)
- Async: no special prefix, convention is `async fn name()` (e.g., `run_watcher()`)

**Variables:**
- Local: `snake_case` (e.g., `file_count`, `symbol_index`)
- Statics/constants: `UPPER_SNAKE_CASE` (e.g., `DEFAULT_THRESHOLD`, `DAEMON_DIR_NAME`)
- Boolean prefixes: `is_`, `should_`, `has_` (e.g., `is_test`, `should_abort`)

**Types:**
- Structs: `PascalCase` (e.g., `LiveIndex`, `IndexedFile`, `SymbolRecord`)
- Enums: `PascalCase` variants (e.g., `ParseStatus::Parsed`, `SymbolKind::Function`)
- Type aliases: `PascalCase` (e.g., `SharedIndex`, `SharedDaemonState`)
- Generics: Single uppercase letter or descriptive (e.g., `T`, `S` for Serialize)

**Modules:**
- Submodules: `lowercase_with_underscores` (e.g., `live_index`, `sidecar`, `parsing`)
- Re-exports: `pub use` statements in `mod.rs`

## Where to Add New Code

**New Tool:**
1. Define input struct in `src/protocol/tools.rs` (with `#[derive(Deserialize, JsonSchema)]`)
2. Add `#[tool]` handler method in `TokenizorServer` or submodule
3. Add to tool router in `src/protocol/mod.rs` if macro-generated
4. Add output formatter to `src/protocol/format.rs`
5. Register in `src/cli/init.rs` in `TOKENIZOR_TOOL_NAMES`
6. Add tests in `src/protocol/tools.rs` alongside handler

**New Language Parser:**
1. Create `src/parsing/languages/newlang.rs`
2. Implement `extract_symbols(tree: &Tree, text: &str) -> Vec<SymbolRecord>`
3. Add `LanguageId::NewLang` variant to `src/domain/index.rs`
4. Add file extension mapping in `LanguageId::from_extension()`
5. Add dispatcher entry in `src/parsing/languages/mod.rs:parse_source()`
6. Add tree-sitter crate dependency to `Cargo.toml`
7. Test with sample files

**New Query Function:**
1. Implement function in `src/live_index/query.rs`
2. Return a view type (or define new struct ending in `View`)
3. Use `index.read_lock()` to acquire read access
4. Format result via `format::` functions
5. Add corresponding tool in `protocol/tools.rs`

**New HTTP Hook Handler:**
1. Add handler function to `src/sidecar/handlers.rs` (signature: `async fn handle_hook(State<SidecarState>, Json<Input>) -> String`)
2. Add route to router in `src/sidecar/router.rs` (e.g., `Router::new().route("/myhook", post(handler))`)
3. Record token savings via `state.token_stats.record_*()`
4. Return plain text response via formatting functions

**Bug Fix or Refactor:**
1. Locate relevant module (e.g., `src/live_index/store.rs` for index logic)
2. Add test in the module's `#[cfg(test)]` block
3. Fix the implementation
4. Run `cargo test --all-targets` to verify
5. Run `cargo fmt` and `cargo clippy` before committing

## Special Directories

**`.tokenizor/`:**
- Purpose: Runtime state directory for index snapshots, port files, lock files
- Generated: Yes, created at `src/live_index/persist.rs` on first startup
- Committed: No (added to .gitignore)
- Cleanup: Safe to delete; index will be recomputed on next startup

**`target/`:**
- Purpose: Cargo build artifacts (debug/release binaries, deps)
- Generated: Yes, by `cargo build`
- Committed: No (in .gitignore)
- Cleanup: `cargo clean` removes

**`.github/workflows/`:**
- Purpose: CI/CD pipeline definitions
- Committed: Yes, part of repository
- Key workflows: `rust.yml` (test + build + publish)

**`npm/`:**
- Purpose: Node.js wrapper package for binary distribution
- Committed: Yes, checked into repo
- Key files: `package.json` (package definition), `index.js` (binary locator), `preinstall.js` (build hook)

**`docs/`:**
- Purpose: User documentation, design specs, implementation guides
- Committed: Yes
- Key patterns: Markdown documents in chronological structure

---

*Structure analysis: 2026-03-14*
