# Coding Conventions

**Analysis Date:** 2026-03-14

## Naming Patterns

**Files:**
- Rust source files: `snake_case.rs` (e.g., `git_temporal.rs`, `edit_format.rs`)
- Test modules: co-located in files using `#[cfg(test)] mod tests { ... }`
- Module hierarchy: split by domain (e.g., `protocol/`, `live_index/`, `parsing/`)

**Functions:**
- Public functions: `snake_case` (e.g., `get_symbol`, `search_symbols`, `apply_splice`)
- Helper functions: `snake_case` prefixed with verb (e.g., `make_indexed_file`, `make_live_index`, `render_symbol_selector`)
- Async functions: same `snake_case` convention, async context implied by type signature
- Test functions: `test_<function_or_behavior_name>` (e.g., `test_apply_splice_replaces_middle`, `test_churn_bar_zero`)

**Variables:**
- Local variables: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE` for module-level constants
- Struct fields: `snake_case`
- Type parameters: `T`, `U`, `S` (standard Rust conventions)

**Types:**
- Structs: `PascalCase` (e.g., `LiveIndex`, `IndexedFile`, `SymbolRecord`)
- Enums: `PascalCase` (e.g., `FileOutcome`, `SymbolMatchTier`, `CircuitBreakerState`)
- Enum variants: `PascalCase` (e.g., `Exact`, `Prefix`, `Substring`)
- Type aliases: `PascalCase` (e.g., `ParseSourceOutput`, `Result<T>`)

## Code Style

**Formatting:**
- Tool: `rustfmt` (built-in, no custom config file)
- CI enforces: `cargo fmt --all --check`
- Line length: default (typically 100 chars, enforced by CI)
- Braces: Allman style with opening brace on same line per Rust convention

**Linting:**
- Tool: implicit clippy (via `cargo test` / `cargo check`)
- CI: test suite enforces code quality via `cargo test --all-targets -- --test-threads=1`
- No explicit `.clippy.toml` or linting config file

**Code organization:**
- Group imports: stdlib → third-party → local (within use blocks)
- One blank line between logical sections (marked with `// ─── Comment ──`)
- Two blank lines between major sections (e.g., between function definitions)

## Import Organization

**Order:**
1. Stdlib crates (`use std::...`)
2. Third-party crates (in alphabetical order where practical)
3. Internal crate modules (`use crate::...`)

**Path aliases:**
- Not used explicitly; rely on `pub use` re-exports in `mod.rs` files
- Example: `crate::domain` re-exports from `crate::domain::index`

**Example from `src/protocol/tools.rs`:**
```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use axum::http::StatusCode;
use rmcp::handler::server::wrapper::Parameters;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use crate::domain::LanguageId;
use crate::live_index::IndexedFile;
use crate::protocol::edit;
```

## Error Handling

**Pattern:**
- Use `thiserror::Error` with `#[derive]` macro for custom error types
- Define custom error enum at module level (see `src/error.rs`)
- Use type alias: `pub type Result<T> = std::result::Result<T, TokenizorError>;`
- Propagate with `?` operator in most contexts

**Custom error types (from `src/error.rs`):**
```rust
#[derive(Debug, Error)]
pub enum TokenizorError {
    #[error("i/o error at `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("parse error: {0}")]
    Parse(String),
    #[error("discovery error: {0}")]
    Discovery(String),
    #[error("circuit breaker: {0}")]
    CircuitBreaker(String),
    #[error("invalid configuration: {0}")]
    Config(String),
}
```

**Error handling in tests:**
- Use `.unwrap()` with descriptive panic message: `.unwrap_or_else(|| panic!("message"))`
- Use `.expect()` when message is important: `.read().expect("lock poisoned")`
- Return `Result` types from test helpers, unwrap at test boundaries

**Fallible operations:**
- Lock operations: `.read().expect("lock")` or `.write().expect("lock")`
- File I/O: propagate with `?` or handle with explicit match
- Parsing: catch panics with `panic::catch_unwind`, wrap parse errors as `FileOutcome::Failed`

## Logging

**Framework:** `tracing` (init in `src/observability.rs`)

**Patterns:**
- Initialization: `tracing_subscriber` with env-filter (see `observability::init_tracing()`)
- Log levels: implicitly configured via `RUST_LOG` env var
- No explicit log statements in many modules; structured tracing available when enabled
- Hooks (CLI binary): forbidden from writing to stderr/stdout except for final JSON output (see `src/cli/hook.rs` HOOK-10)

## Comments

**Module-level documentation:**
- Use `//!` for crate/module documentation (explains purpose, design principles)
- Example from `src/live_index/git_temporal.rs`:
```rust
//! Git temporal intelligence — enriches the index with git history metadata.
//!
//! Computes per-file churn scores (exponential-decay weighted), ownership
//! distribution, co-change coupling (Jaccard coefficient), and repo-wide
//! hotspot summaries using libgit2 via [`crate::git::GitRepo`].
//!
//! Design principles:
//! - In-process git access: uses libgit2 (via git2 crate) — no child
//!   processes, no console windows, faster execution.
```

**Function documentation:**
- Use `///` for public functions (describes what it does, parameters, return value)
- Include constraints, side effects, panics where relevant
- Example from `src/protocol/tools.rs`:
```rust
/// Deserialize a `u32` from either a JSON number or a stringified number like `"5"`.
pub(crate) fn lenient_u32<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u32>, D::Error>
```

**Inline comments:**
- Use `//` for implementation details, non-obvious logic, or Anti-pattern notes
- Section separators: `// ─── Section Name ───────────────────`
- Example from `src/protocol/tools.rs`:
```rust
/// Each handler follows the pattern:
/// 1. Acquire read lock (or write lock for `index_folder`)
/// 2. Check loading guard (except `health` which always responds)
/// 3. Extract needed data into owned values
/// 4. Drop lock
/// 5. Call `format::` function
/// 6. Return `String`
///
/// Anti-patterns avoided (per RESEARCH.md):
/// - Never return JSON — always plain text String (AD-6)
/// - Never use MCP error codes for not-found — return helpful text via format functions
/// - Never hold RwLockReadGuard across await points — extract into owned values first
```

## Struct Design

**Visibility:**
- Public structs for types that cross module boundaries (e.g., `IndexedFile`, `SymbolRecord`)
- Private structs (`pub(crate)`) for internal implementation details
- Input structs for handlers use `#[derive(Deserialize, Serialize, JsonSchema)]` for serde support

**Deserialization helpers:**
- Custom deserializers using `#[serde(deserialize_with = "...")]` for lenient parsing
- Functions like `lenient_u32`, `lenient_bool` convert JSON numbers/strings to typed values
- Serde `#[serde(default)]` for optional fields that default to `None` or empty

**Example from `src/protocol/tools.rs`:**
```rust
#[derive(Deserialize, Serialize, JsonSchema)]
pub struct SearchSymbolsInput {
    /// Search query (case-insensitive substring match).
    pub query: String,
    /// Optional kind filter using display names such as `fn`, `class`, or `interface`.
    pub kind: Option<String>,
    /// Optional relative path prefix scope, for example `src/` or `src/protocol`.
    pub path_prefix: Option<String>,
    /// Optional maximum number of matches to return (default 50, capped at 100).
    #[serde(default, deserialize_with = "lenient_u32")]
    pub limit: Option<u32>,
}
```

## Function Design

**Size:**
- Prefer small, single-responsibility functions (typical: 10–50 lines)
- Large functions (>100 lines) split into helpers if logic is complex
- Test-helpers marked with comments: `// ─── Test helpers ─────────────────`

**Parameters:**
- Use owned types where cheap (e.g., `String`, `Vec`) for clarity
- Borrow for expensive types or when mutation needed (e.g., `&mut [u8]`)
- Path parameters: use `&str` for lookups, `PathBuf` when storing
- Result types: use custom `Result<T>` alias, not `Result<T, anyhow::Error>`

**Return values:**
- Use `Option<T>` for fallible lookups (`get_file`, `get_symbol`)
- Use `Result<T>` for operations that might fail (I/O, parsing)
- Return owned values unless performance-critical
- Return `String` from formatting functions (never `JSON`, always plain text)

## Module Design

**Exports:**
- Re-export commonly-used types in `mod.rs` via `pub use`
- Example from `src/domain/mod.rs`:
```rust
pub use index::{
    FileClass, FileClassification, FileOutcome, FileProcessingResult, LanguageId,
    ReferenceKind, ReferenceRecord, SupportTier, SymbolKind, SymbolRecord,
};
```

**Barrel files:**
- `mod.rs` files list submodules and re-export types
- Flatten internal hierarchy for external API (e.g., `live_index::SearchFilesHit` not `live_index::search::SearchFilesHit`)

**Test modules:**
- Co-located in source files via `#[cfg(test)] mod tests { ... }`
- Never in separate `tests/` directory (all tests in `src/`)
- Access to `super::*` for testing private functions

## Special Patterns

**Async/await:**
- Use `tokio::spawn` for background tasks
- Guard spawns: check `tokio::runtime::Handle::try_current().is_ok()` to ensure runtime exists
- Type signature declares `async`: `async fn run_mcp_server_async() -> anyhow::Result<()>`

**Locking:**
- Use `Arc<RwLock<T>>` for shared state (shared mutable references)
- Always extract data into owned values before dropping lock (anti-pattern: holding guard across await)
- Test pattern: `.read().expect("lock")` or `.write().unwrap()`

**String conversions:**
- Use `.to_string()` for `impl ToString` types
- Use `String::from()` for `&str` literals when clarity needed
- Use `.to_owned()` for references when explicit

---

*Convention analysis: 2026-03-14*
