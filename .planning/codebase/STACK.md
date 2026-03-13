# Technology Stack

**Analysis Date:** 2026-03-14

## Languages

**Primary:**
- **Rust** 2024 Edition - Core implementation; server, indexing, parsing, daemon
- **Tree-sitter Query Language** - Symbol extraction and cross-reference queries via tree-sitter

**Supported Code Analysis Targets:**
- Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, PHP, Swift, Perl, Kotlin, Dart, Elixir

## Runtime

**Environment:**
- **Tokio 1.48** - Async runtime for I/O operations, file watching, HTTP server
- **Rust async/await** - Multi-threaded async model with `tokio::runtime::Builder::new_multi_thread()`

**Package Manager:**
- **Cargo** - Rust package manager
- **Lockfile:** `Cargo.lock` present

## Frameworks

**Core:**
- **rmcp 1.1.0** - Model Context Protocol server framework with stdio and transport support
- **Axum 0.8** - HTTP server framework for sidecar HTTP API (async web framework)
- **Tokio 1.48** - Async runtime with multi-thread executor, signal handlers, timers, sync primitives

**Parsing & Analysis:**
- **Tree-sitter 0.26** - Core parser with language-specific bindings (tree-sitter-rust, tree-sitter-python, tree-sitter-javascript, tree-sitter-typescript, tree-sitter-go, tree-sitter-java, tree-sitter-c, tree-sitter-cpp, tree-sitter-c-sharp, tree-sitter-ruby, tree-sitter-php, tree-sitter-swift, tree-sitter-perl, tree-sitter-kotlin-sg, tree-sitter-dart, tree-sitter-elixir)
- **tree-sitter** version matrix (0.23-0.26 across language bindings)

**Testing:**
- No external testing framework; tests are inline in modules using `#[cfg(test)]` and `#[test]` attributes

**Build/Dev:**
- **Clap 4** - CLI argument parsing with derive macros
- **Serde 1.0** - Serialization framework with derive support
- **Schemars 1.0** - JSON Schema generation for MCP tool parameters

## Key Dependencies

**Critical:**
- **rmcp 1.1.0** - MCP server protocol implementation; enables Claude Code integration
- **Tokio 1.48** - Async I/O runtime; required for file watching, HTTP server, concurrent indexing
- **Tree-sitter 0.26** - Core parsing engine; extracts symbols and cross-references from source code
- **git2 0.20** - libgit2 bindings for in-process git operations (replaces shell commands)

**Infrastructure:**
- **reqwest 0.12** - HTTP client; used by daemon for inter-session communication
- **Axum 0.8** - Async web server for sidecar HTTP endpoints
- **Tracing 0.1 + tracing-subscriber 0.3** - Structured logging to stderr with env filter
- **Postcard 1.1** - Binary serialization for index persistence
- **toml_edit 0.23** - TOML file parsing for Cargo.toml inspection
- **serde_json 1.0** - JSON handling for MCP protocol and HTTP sidecar
- **git2 0.20** - libgit2 bindings (vendored); zero-process git operations for status, diff, log
- **anyhow 1.0** - Error handling with context
- **thiserror 2.0** - Derive error types
- **notify 8 + notify-debouncer-full 0.7** - File system watcher for live index updates
- **globset 0.4** - `.gitignore`-style pattern matching for file filtering
- **ignore 0.4** - `.gitignore` parsing and respecting
- **regex 1.11** - Pattern matching for text search
- **rayon 1.10** - Data parallelism for batch indexing
- **streaming-iterator 0.1** - Memory-efficient iteration over large datasets
- **dirs 6** - Cross-platform home directory detection
- **serde_json 1.0** - JSON serialization for MCP responses

## Configuration

**Environment Variables:**
- `TOKENIZOR_AUTO_INDEX` - Control startup auto-indexing (default: `true`); set to `false` to start with empty index
- `TOKENIZOR_DAEMON_BIND` - Daemon HTTP bind address (default: `127.0.0.1`)
- `TOKENIZOR_SIDECAR_BIND` - Sidecar HTTP bind address (default: `127.0.0.1`)
- `TOKENIZOR_HOME` - Override daemon home directory (defaults to `~/.tokenizor/`)
- `RUST_LOG` - Tracing/logging level (processed by `tracing-subscriber::EnvFilter`, default: `info`)

**Build Configuration:**
- `Cargo.toml` - Package manifest with features `[v1]` (legacy feature gate)
- No `Cargo.lock` modifications required; vendored libgit2 via `git2` default features

**Project Directories:**
- `.tokenizor/` - Local runtime state (index snapshots, sidecar port files, daemon ports)
- `.tokenizor/index.bin` - Serialized live index snapshot (postcard format)
- `.tokenizor/sidecar.port` - Ephemeral HTTP port for sidecar (written at startup)
- `.tokenizor/sidecar.session` - Session ID for sidecar (written at startup)

## Platform Requirements

**Development:**
- Rust 1.81+ (2024 Edition)
- Cargo (ships with Rust)
- `git` binary in PATH (for libgit2 vendoring during build)

**Runtime:**
- Linux, macOS, Windows (x86_64 and ARM64)
- libgit2 (vendored in `git2` crate; no external git binary required for operations)
- POSIX file system with watching support (via `notify` crate)

**Production:**
- Deployment as MCP server (stdio transport) or daemon mode (HTTP proxy)
- Can run as standalone process or embedded in Claude Code environments
- Sidecar mode: Axum HTTP server on ephemeral port (127.0.0.1 by default)
- Daemon mode: Shared service pool at `~/.tokenizor/daemon/` for multi-project sessions

---

*Stack analysis: 2026-03-14*
