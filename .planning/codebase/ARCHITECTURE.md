# Architecture

**Analysis Date:** 2026-03-14

## Pattern Overview

**Overall:** Multi-layer MCP (Model Context Protocol) server with local and daemon-backed execution modes.

**Key Characteristics:**
- **Stateful indexing**: In-memory symbol index with disk persistence (`.tokenizor/index.bin`)
- **Dual modes**: Local stdio MCP server or daemon-proxied remote server with fallback
- **Real-time watching**: Background file watcher with trigram-based search index
- **HTTP sidecar**: Companion web server for hook handlers and token statistics
- **Symbol-aware operations**: Tree-sitter based parsing (15+ languages) with cross-reference tracking
- **Daemon coordination**: Shared project-level indexing via Axum HTTP service in the same process

## Layers

**Parsing Layer:**
- Purpose: Extract symbols, references, and metadata from source files using language-specific tree-sitter grammars
- Location: `src/parsing/`
- Contains: Language plugins for Rust, Python, JavaScript, TypeScript, Go, Java, C/C++, C#, Ruby, PHP, Swift, Perl, Kotlin, Dart, Elixir
- Depends on: tree-sitter parsers (external crates), domain types (`LanguageId`, `SymbolRecord`)
- Used by: LiveIndex loading pipeline
- Key files: `src/parsing/languages/mod.rs` dispatches to language-specific modules, `src/parsing/xref.rs` extracts cross-references

**Storage/Query Layer:**
- Purpose: Maintain in-memory index of files, symbols, references, and search indices; expose query APIs
- Location: `src/live_index/`
- Contains:
  - `store.rs`: `LiveIndex` (main in-memory store), `IndexedFile`, `ParseStatus`, `CircuitBreakerState`
  - `query.rs`: Public query functions returning view types (RepoOutlineView, SymbolDetailView, etc.)
  - `search.rs`: Trigram-based full-text search and symbol search
  - `persist.rs`: Snapshot serialization/deserialization (postcard binary format)
  - `git_temporal.rs`: Background computation of git change frequency per symbol
- Depends on: Parsing layer output, git2 for repository metadata
- Used by: Protocol layer (tools), sidecar handlers, watcher for incremental updates
- Key abstraction: `SharedIndex` = `Arc<RwLock<LiveIndex>>` allows concurrent reads and exclusive writes

**Protocol/Tool Layer:**
- Purpose: Expose MCP tools (24 tools) and prompts as the primary client interface
- Location: `src/protocol/`
- Contains:
  - `tools.rs`: Tool handler methods and input parameter structs (GetSymbol, SearchSymbols, etc.)
  - `format.rs`: Output formatting functions that transform query results to plain-text responses
  - `edit.rs`: Symbolic edit operations (InsertSymbol, DeleteSymbol, ReplaceSymbolBody, etc.)
  - `explore.rs`: Exploration helpers for the Explore tool
  - `resources.rs`: MCP resource handlers for file content (tokenizor:// URIs)
  - `prompts.rs`: Prompt definitions
- Depends on: Storage/Query layer, domain types, sidecar handlers
- Used by: MCP framework (rmcp crate) via `#[tool]` and `#[prompt]` macros
- Key class: `TokenizorServer` holds the shared index and coordinates tool dispatch

**Sidecar/HTTP Layer:**
- Purpose: Provide HTTP endpoints for hook handlers (read, edit, write, grep) and token statistics
- Location: `src/sidecar/`
- Contains:
  - `server.rs`: Spawns Axum HTTP server, listens on ephemeral port
  - `handlers.rs`: HTTP handlers for `/read`, `/edit`, `/write`, `/grep` routes
  - `router.rs`: Axum route definitions
  - `port_file.rs`: Writes port/PID files to `.tokenizor/` for hook discovery
- Depends on: Storage/Query layer, sidecar handlers (optimized HTTP response formatting)
- Used by: External MCP hooks (via environment variable port discovery)
- Key struct: `TokenStats` tracks atomic counters for hook fires and estimated token savings

**Daemon/Session Layer:**
- Purpose: Maintain project-level indexing state across multiple client sessions
- Location: `src/daemon.rs` (primarily), coordinated by `main.rs` startup logic
- Contains:
  - `DaemonSessionClient`: HTTP client for connecting to daemon from MCP server
  - `DaemonState`: Per-daemon state machine tracking projects, sessions, indices
  - Daemon HTTP routes for project lifecycle (open, heartbeat, tool forwarding)
- Depends on: Storage/Query layer, protocol layer (forwards tool calls)
- Used by: MCP server startup when auto-index finds an existing project root
- Key pattern: Fallback strategy — if daemon connection fails, local stdio mode activates

**File Watching/Discovery Layer:**
- Purpose: Discover source files on startup and track changes during runtime
- Location: `src/discovery/` and `src/watcher/`
- Contains:
  - `discovery/mod.rs`: `discover_files()` walks directory tree respecting `.gitignore`, maps extensions to `LanguageId`
  - `watcher/mod.rs`: Spawns file system watcher (notify-debouncer-full), parses changes, updates index
- Depends on: Parsing layer, domain types
- Used by: Main startup (`run_local_mcp_server_async`), file system notification loop

**CLI Layer:**
- Purpose: Provide subcommands for initialization, daemon management, and hooks
- Location: `src/cli/`
- Contains:
  - `mod.rs`: `Cli` struct using clap for command parsing
  - `init.rs`: Generates tool list for client initialization
  - `hook.rs`: Hook subcommand handlers (placeholder for post-tool-use, stop hooks)
- Depends on: Protocol layer, daemon layer
- Used by: `main.rs` dispatch logic before MCP server starts

## Data Flow

**Startup Flow (Local Mode):**

1. `main.rs` calls `run_mcp_server_async()`
2. `discovery::find_project_root()` walks up from cwd, finds `.git`, returns project root
3. Fast path: `persist::load_snapshot(&root)` deserializes pre-computed index from `.tokenizor/index.bin`
   - If hit: spawn background verification task (`persist::background_verify`) to reconcile against disk mtimes
   - If miss: fall through to full re-index
4. Full re-index: `LiveIndex::load(&root)` discovers files, parses them in parallel (rayon), builds symbol/reference tables
5. Startup logging checks `PublishedIndexStatus::Ready` vs `Degraded` (circuit breaker triggered)
6. `watcher::run_watcher()` spawned asynchronously; registers file system listeners
7. `live_index::git_temporal::spawn_git_temporal_computation()` started for background git analysis
8. `sidecar::spawn_sidecar()` starts HTTP server, writes port to `.tokenizor/sidecar.port`
9. `protocol::TokenizorServer::new()` created with index + project name
10. `serve_server(server, transport::stdio())` starts MCP stdio transport

**File Change Flow:**

1. `notify-debouncer-full` detects file modification
2. `watcher::on_file_changed()` reads new file bytes, detects language from extension
3. `parsing::process_file()` tree-sitter parses, extracts symbols + references
4. `LiveIndex::update_file()` acquires write lock, replaces file in store, updates reverse_index
5. Index generation incremented, `PublishedIndexState` updated atomically
6. File content cached in `IndexedFile::content` for zero-disk-I/O reads

**Tool Call Flow (Daemon-Backed Mode):**

1. Client calls MCP tool via stdio
2. `TokenizorServer::proxy_tool_call()` checks if `daemon_client` is present
3. If present: serialize tool params to JSON, POST to daemon `/tool/{tool_name}`
4. Daemon (in `daemon.rs` HTTP handler) looks up project session, forwards to that session's local `TokenizorServer`
5. Local server executes tool, returns plain-text result
6. HTTP response flows back through proxy, returned to client
7. If daemon connection fails: set `daemon_degraded` flag, fall through to local execution

**Token Savings Flow:**

1. Hook (read/edit/grep) calls sidecar HTTP endpoint with file content + byte range
2. Handler (in `sidecar/handlers.rs`) calls `query::*` functions to extract symbol context
3. Handler formats response (targeted subset vs full file)
4. `TokenStats::record_read()` called with (file_bytes, output_bytes)
5. Savings estimate: `(file_bytes - output_bytes) / 4` tokens
6. MCP health tool reads `token_stats` Arc and reports cumulative savings

**State Management:**

- **Index mutations**: Acquired via `index.write_lock()` (exclusive)
- **Index reads**: Acquired via `index.read_lock()` (shared, many concurrent)
- **Snapshot verification**: Mtime tracking prevents re-parsing unchanged files
- **Symbol cache**: `SidecarState::symbol_cache` (pre-edit snapshots for impact diff)
- **Project lifecycle**: Daemon `SessionRecord` tracks open_at, last_seen_at for cleanup

## Key Abstractions

**SharedIndex (Arc<RwLock<LiveIndex>>):**
- Purpose: Thread-safe shared mutable index
- Files: `src/live_index/store.rs:SharedIndexHandle`
- Pattern: RwLock allows many concurrent readers, exclusive writers
- Used by: All query handlers, watcher, daemon session

**LiveIndex:**
- Purpose: In-memory symbol database with reverse references
- Fields:
  - `files: HashMap<String, IndexedFile>` — relative path → file with symbols
  - `reverse_index: HashMap<String, Vec<ReferenceLocation>>` — symbol name → places referencing it
  - `search: SearchIndex` — trigram-based full-text search on symbol names/file paths
  - `published_state: Arc<AtomicCell<PublishedIndexState>>` — status published to clients
  - `circuit_breaker: CircuitBreakerState` — failure rate tracker
- Key methods: `update_file()`, `get_file()`, `get_symbol()`, `find_references()`, `search_symbols()`

**IndexedFile:**
- Purpose: Single file with all parsed data
- Fields:
  - `content: Vec<u8>` — raw file bytes (zero-disk-I/O guarantee)
  - `symbols: Vec<SymbolRecord>` — parsed symbols (name, kind, byte range, line range)
  - `references: Vec<ReferenceRecord>` — cross-references extracted by xref module
  - `parse_status: ParseStatus` — Parsed | PartialParse | Failed
- Invariant: If parse_status is Failed, symbols list is empty but content is still stored

**SymbolRecord (domain type):**
- Purpose: Single parsed symbol (function, struct, class, etc.)
- Fields:
  - `name: String` — symbol name
  - `kind: SymbolKind` — Function, Struct, Enum, Impl, etc.
  - `byte_range: (u32, u32)` — character offsets in file
  - `line_range: (u32, u32)` — line number range (0-indexed)
  - `doc: Option<String>` — JSDoc/Rustdoc comments
- Files: `src/domain/index.rs`

**TokenizorServer:**
- Purpose: MCP server entrypoint, coordinates tool dispatch
- Fields:
  - `index: SharedIndex` — main symbol database
  - `tool_router: ToolRouter<Self>` — rmcp macro-generated tool dispatcher
  - `daemon_client: Option<Arc<tokio::sync::RwLock<DaemonSessionClient>>>` — fallback to daemon
  - `daemon_degraded: Arc<AtomicBool>` — flag to stop reconnection attempts
  - `token_stats: Option<Arc<TokenStats>>` — shared sidecar statistics
- Key methods:
  - `proxy_tool_call()` — forwards to daemon if available
  - `record_read_savings()` — updates token stats
  - `set_repo_root()` / `capture_repo_root()` — tracks latest project root for `index_folder`

## Entry Points

**Main process entry (stdio MCP):**
- Location: `src/main.rs:main()`
- Triggers: When invoked as MCP server (default, no CLI args)
- Responsibilities:
  1. Parse CLI flags (--daemon, --init, --hook subcommand)
  2. Dispatch to appropriate handler (daemon, init, hook, or MCP)
  3. For MCP: initialize logging, discover project root, load/build index, spawn watchers, start stdio server

**Daemon process entry:**
- Location: `src/main.rs:run_daemon()`
- Triggers: When invoked with `--daemon` flag
- Responsibilities:
  1. Start HTTP server on 127.0.0.1 (OS-assigned port)
  2. Register project instances on demand (`/api/projects/open`)
  3. Forward tool calls to project-specific sessions
  4. Track session heartbeats, perform cleanup

**HTTP Sidecar:**
- Location: `src/sidecar/server.rs:spawn_sidecar()`
- Triggers: During MCP startup (if auto-index enabled)
- Responsibilities:
  1. Bind to ephemeral port
  2. Register routes: `/read`, `/edit`, `/write`, `/grep`, `/stats`
  3. Read `SidecarState` (index + stats + symbol cache)
  4. Write port file to `.tokenizor/sidecar.port`

**File watcher:**
- Location: `src/watcher/mod.rs:run_watcher()`
- Triggers: During MCP startup (if project root discovered)
- Responsibilities:
  1. Debounce file system events
  2. Detect file language from extension
  3. Re-parse modified files
  4. Update index atomically
  5. Track watcher info (events seen, performance)

## Error Handling

**Strategy:** Return informative text, avoid MCP error codes for user-facing problems.

**Patterns:**
- **Parsing failures**: Captured in `ParseStatus::Failed { error }`, index still loads, error reported in `get_file_outline`
- **Circuit breaker**: Failure rate > 20% (configurable) aborts index load, status becomes `PublishedIndexStatus::Degraded`
- **File not found**: Query functions return empty results with explanatory text (e.g., "Symbol 'foo' not found in any file")
- **Daemon connection**: Single reconnect attempt, then fallback to local execution with flag set
- **Parse panics**: Caught with `panic::catch_unwind()`, result wrapped as `ParseStatus::Failed`

## Cross-Cutting Concerns

**Logging:** `src/observability.rs` initializes tracing with env filter (RUST_LOG)
- Tools: tracing crate with info, warn, error levels
- Entry: `observability::init_tracing()`
- Usage: Structured logs at startup, watcher events, parse failures, daemon lifecycle

**Validation:** Tool input structs validated via:
- Serde deserialization (lenient deserializers for u32/bool from strings)
- Loading guard: Tools check `index.published_state().status` before proceeding (except health tool)
- Path validation: Relative paths checked against known files in index

**Authentication:** None (single-user local service or trusted daemon)

**Concurrency:**
- Index access: `Arc<RwLock<LiveIndex>>` for thread-safe reads/writes
- File parsing: Rayon parallel iterator for bulk parse on startup
- Async runtime: Tokio multi-threaded, multiple tasks (watcher, git_temporal, sidecar, MCP server)

---

*Architecture analysis: 2026-03-14*
