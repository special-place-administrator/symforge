# External Integrations

**Analysis Date:** 2026-03-14

## APIs & External Services

**Model Context Protocol (MCP):**
- **Service:** Anthropic Claude Code integration via MCP protocol
- **What it's used for:** Expose 24 MCP tools for symbol-aware code navigation and editing to Claude
- **Implementation:** `src/protocol/` implements MCP tool handlers (24 tools exposed via `tools/list`)
- **Transport:** Stdio transport for local communication with Claude; HTTP daemon proxy for remote sessions

**HTTP Sidecar:**
- **Service:** Local HTTP API for hook integration
- **What it's used for:** Allows Claude editor hooks to query index without MCP overhead (token savings)
- **SDK/Client:** Axum 0.8 HTTP server on ephemeral port
- **Endpoints:**
  - `GET /health` - Index health and file/symbol counts
  - `GET /outline` - File outline with token budget
  - `GET /impact` - File impact analysis for modified files
  - `GET /symbol-context` - Symbol context and consumers
  - `GET /prompt-context` - Smart context assembly from natural language
  - `POST /stats` - Token savings metrics from hook fires

**HTTP Daemon:**
- **Service:** Long-lived background daemon for session pooling
- **What it's used for:** Shared index across multiple MCP clients (multi-client support)
- **SDK/Client:** reqwest 0.12 HTTP client (DaemonSessionClient)
- **Endpoints:**
  - `POST /projects/open` - Open/create project session
  - `GET /sessions/{id}/health` - Session health
  - `POST /sessions/{id}/tools/{tool}` - Route tool calls to session's index
  - `POST /sessions/{id}/close` - Close session and cleanup

## Data Storage

**Databases:**
- **Not used** - No traditional database; in-memory index only

**File Storage:**
- **Local filesystem only** - Files read from disk; no cloud storage
- **Index snapshots:** `.tokenizor/index.bin` (postcard binary format) for snapshot persistence on shutdown
- **Working directory:** Must have read access to project root and write access to `.tokenizor/` subdirectory

**Caching:**
- **In-process index:** Shared `Arc<RwLock<LiveIndex>>` across all tools and connections
- **File watching:** `notify` crate watches filesystem for changes; updates live index in real-time
- **Symbol cache:** `Arc<RwLock<HashMap<String, Vec<SymbolSnapshot>>>>` in daemon for symbol deduplication across sessions
- **Snapshot persistence:** Binary index saved to `.tokenizor/index.bin` on clean shutdown; reloaded on next startup

## Authentication & Identity

**Auth Provider:**
- **None** - No external authentication
- **Implementation:** Implicit trust; tokenizor runs in user's local environment
- **Session IDs:** UUID-v4 generated per session for daemon session tracking
- **Project IDs:** Generated from project root path hash for multi-project grouping

## Monitoring & Observability

**Error Tracking:**
- **None** - No external error tracking
- **Implementation:** Errors logged to stderr via `tracing` crate with structured fields

**Logs:**
- **Approach:** Structured logging to stderr using `tracing` and `tracing-subscriber`
- **Configuration:** `RUST_LOG` environment variable controls verbosity (default: `info`)
- **Output format:** Plain text (ANSI disabled for compatibility with pipes/logs)
- **Captured fields:** File paths, symbol counts, parse errors, watch events, git operations

## Git Integration

**Git Operations:**
- **Library:** libgit2 via `git2` crate (vendored)
- **What it's used for:**
  - Uncommitted changes detection (`git status` replacement)
  - Diff between refs (3-dot semantics for branch comparisons)
  - Git log extraction for temporal analysis (Phase 5: doc comment range tracking)
  - Merge-base calculation for diff semantics
- **No shell execution** - All operations in-process via libgit2; zero child processes
- **Features used:** `vendored-libgit2` to bundle libgit2 (no external git binary needed for API)

## Webhooks & Callbacks

**Incoming:**
- **None** - Tokenizor is driven by tool calls from Claude (MCP) or hook calls from Claude editor

**Outgoing:**
- **None** - Does not call external webhooks or fire events

## File Watching & Hot Reload

**File Watching:**
- **Library:** `notify 8` + `notify-debouncer-full 0.7`
- **What it's used for:** Detect file changes in project directory and update live index in real-time
- **Behavior:**
  - Watches project root recursively
  - Respects `.gitignore` via `ignore` crate
  - Debounces rapid changes (bundled by `notify-debouncer-full`)
  - Updates `LiveIndex` on create/modify/delete events
  - Purges symbols and references when files are deleted
  - Works on Windows, macOS, Linux via OS-specific backends (inotify, FSEvents, ReadDirectoryChangesW)

## No External APIs

The codebase **does not integrate with:**
- No cloud platforms (AWS, GCP, Azure)
- No API services (Stripe, Auth0, SendGrid, etc.)
- No LLM APIs (OpenAI, Anthropic, etc.) - only used by Claude Code client
- No databases or ORMs
- No package registries (npm, crates.io)
- No CI/CD systems
- No monitoring services (Datadog, New Relic, Sentry)
- No message queues or event buses

## Environment Configuration

**Required env vars:**
- None - All env vars are optional with sensible defaults

**Optional env vars (for runtime tuning):**
- `TOKENIZOR_AUTO_INDEX=false` - Disable auto-indexing on startup (start with empty index)
- `TOKENIZOR_DAEMON_BIND=0.0.0.0` - Change daemon HTTP bind address
- `TOKENIZOR_SIDECAR_BIND=0.0.0.0` - Change sidecar HTTP bind address
- `TOKENIZOR_HOME=/custom/path` - Override daemon home directory (default: `~/.tokenizor/`)
- `RUST_LOG=debug` - Change logging level (default: `info`)

**Secrets location:**
- **No secrets needed** - Tokenizor is a local service with no external authentication
- No `.env` files required
- No API keys, tokens, or credentials used

## Session Management

**Daemon Sessions:**
- **Type:** HTTP-based session pooling for multi-client scenarios
- **Lifecycle:**
  1. Client calls `POST /projects/open` with project root and client name
  2. Daemon creates `ProjectInstance` if new, or reuses existing
  3. Daemon allocates new `SessionRecord` with UUID session ID
  4. Client receives `DaemonSessionClient` with base URL, project ID, session ID
  5. Subsequent tool calls route through daemon's HTTP proxy
  6. Client calls `POST /sessions/{id}/close` to end session
  7. Daemon cleans up session record (project persists for other sessions)

**MCP Server Modes:**
1. **Local mode** (default): In-process index, stdio MCP transport
2. **Daemon-backed mode:** Stdio MCP transport proxied through daemon HTTP client
3. **Sidecar mode:** Embedded Axum HTTP server for hook integration (runs alongside MCP server)

---

*Integration audit: 2026-03-14*
