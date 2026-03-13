# Testing Patterns

**Analysis Date:** 2026-03-14

## Test Framework

**Runner:**
- Framework: `cargo test` (built-in Rust test harness)
- Config: `.github/workflows/ci.yml` enforces `cargo test --all-targets -- --test-threads=1`
- Single-threaded execution prevents race conditions in shared state tests

**Assertion Library:**
- Standard library: `assert_eq!`, `assert!`, `assert_ne!`
- No external assertion library (no `pretty_assertions`, no custom macros)

**Run Commands:**
```bash
cargo test --all-targets -- --test-threads=1   # Run all tests (matches CI config)
cargo test --lib                                # Run library tests only
cargo test --test-threads=1                     # Run with single thread (default in repo)
cargo test <test_name>                          # Run specific test by name
cargo test -- --nocapture                       # Show println! output during tests
```

## Test File Organization

**Location:**
- Co-located with source code: tests live in the same `.rs` file as implementation
- Pattern: `#[cfg(test)] mod tests { ... }` at end of each module
- No separate `tests/` directory (all unit/integration tests inline)

**Naming:**
- Module: `mod tests`
- Test functions: `test_<function_or_component>_<behavior>` (e.g., `test_apply_splice_replaces_middle`)
- Test helpers: `make_<type>`, `setup_<condition>` (e.g., `make_live_index`, `make_indexed_file`)

**Structure (from `src/protocol/edit.rs`):**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::index::SymbolKind;

    // -- apply_splice --

    #[test]
    fn test_apply_splice_replaces_middle() { ... }

    #[test]
    fn test_apply_splice_replaces_at_start() { ... }

    // -- atomic_write_file --

    #[test]
    fn test_atomic_write_file_creates_file() { ... }

    // --- Test helpers ---

    fn make_result(...) -> FileProcessingResult { ... }
}
```

## Test Structure

**Suite Organization:**
- Group related tests with section comments: `// -- apply_splice --`
- Separate test helpers with: `// --- Test helpers ---`
- Each test function is independent; no setup/teardown shared state

**Pattern (unit test):**
```rust
#[test]
fn test_apply_splice_replaces_middle() {
    let content = b"fn foo() { old_body }";
    let result = apply_splice(content, (11, 19), b"new_body");
    assert_eq!(result, b"fn foo() { new_body }");
}
```

**Pattern (with temporary files):**
```rust
#[test]
fn test_atomic_write_file_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.rs");
    atomic_write_file(&path, b"fn main() {}").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"fn main() {}");
}
```

**Pattern (with shared state):**
```rust
#[test]
fn test_reindex_after_write_updates_index() {
    let handle = crate::live_index::LiveIndex::empty();
    let content = b"fn hello() {}\nfn world() {}\n".to_vec();
    reindex_after_write(&handle, "src/lib.rs", content, LanguageId::Rust);

    let guard = handle.read().expect("lock");
    let file = guard.get_file("src/lib.rs");
    assert!(file.is_some());
    let symbols = &file.unwrap().symbols;
    assert!(symbols.iter().any(|s| s.name == "hello"));
}
```

## Mocking

**Framework:**
- No external mocking library (no `mockito`, `wiremock`, etc.)
- Manual test doubles: helper functions create test fixtures inline

**Patterns:**

### Builder-style helpers:
```rust
fn make_live_index_ready(files: Vec<(String, IndexedFile)>) -> LiveIndex {
    use crate::live_index::trigram::TrigramIndex;
    let files_map = files
        .into_iter()
        .map(|(path, file)| (path, std::sync::Arc::new(file)))
        .collect::<HashMap<_, _>>();
    let trigram_index = TrigramIndex::build_from_files(&files_map);
    let mut index = LiveIndex {
        files: files_map,
        loaded_at: Instant::now(),
        // ... other fields ...
    };
    index.rebuild_reverse_index();
    index
}

fn make_live_index_empty() -> LiveIndex {
    LiveIndex {
        files: HashMap::new(),
        // ... populated with defaults ...
    }
}

fn make_live_index_tripped() -> LiveIndex {
    use crate::live_index::trigram::TrigramIndex;
    let cb = CircuitBreakerState::new(0.10);
    for _ in 0..8 {
        cb.record_success();
    }
    for i in 0..2 {
        cb.record_failure(&format!("f{i}.rs"), "err");
    }
    cb.should_abort(); // trips at 20% > 10%
    LiveIndex { /* ... */ }
}
```

### Field constructor helpers:
```rust
fn make_indexed_file_for_mutation(path: &str) -> IndexedFile {
    IndexedFile {
        relative_path: path.to_string(),
        language: LanguageId::Rust,
        classification: crate::domain::FileClassification::for_code_path(path),
        content: b"fn test() {}".to_vec(),
        symbols: vec![dummy_symbol()],
        parse_status: ParseStatus::Parsed,
        // ... other required fields ...
    }
}
```

**What to Mock:**
- Expensive resources: file systems (use `tempfile` crate), git operations
- External state: index state, circuit breaker states
- Never mock internal pure functions; test them directly

**What NOT to Mock:**
- Pure computation (math, string manipulation)
- Internal data structures that are cheap to construct
- Lock-based synchronization (test real locking behavior)

## Fixtures and Factories

**Test Data:**

Named constructors for common test objects:
```rust
fn dummy_symbol() -> SymbolRecord {
    SymbolRecord {
        name: "test".to_string(),
        kind: SymbolKind::Function,
        // ... defaults for other fields ...
    }
}

fn make_ref(
    name: &str,
    qualified_name: Option<&str>,
    kind: ReferenceKind,
    line: u32,
    enclosing_symbol_index: Option<u32>,
) -> ReferenceRecord {
    ReferenceRecord {
        name: name.to_string(),
        qualified_name: qualified_name.map(str::to_string),
        kind,
        byte_range: (line * 10, line * 10 + 6),
        line_range: (line, line),
        enclosing_symbol_index,
    }
}
```

**Fixture Location:**
- Inline in test module (at bottom of `#[cfg(test)] mod tests { ... }`)
- Organized with comment header: `// --- Fixtures ---` or `// --- Test helpers ---`
- One helper per concern (e.g., `make_indexed_file`, `make_live_index_ready`, `make_server_with_root`)

**Temporary Files:**
- Use `tempfile::tempdir().unwrap()` for isolated test directories
- Cleanup automatic when `TempDir` goes out of scope
- Example from `src/protocol/edit.rs`:
```rust
#[test]
fn test_atomic_write_file_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.rs");
    atomic_write_file(&path, b"fn main() {}").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"fn main() {}");
}
```

## Coverage

**Requirements:** None enforced (no coverage threshold in CI)

**View Coverage:**
- No built-in command (would require `cargo tarpaulin` or `llvm-cov` plugin)
- Coverage not tracked in CI pipeline

## Test Types

**Unit Tests:**
- Scope: single function or small component (e.g., `apply_splice`, `churn_bar`)
- Approach: direct function call with known inputs, assert on output
- Location: same file as implementation, `#[cfg(test)]` module
- Example: `test_apply_splice_replaces_middle` in `src/protocol/edit.rs`

**Integration Tests:**
- Scope: multi-component behavior (e.g., `reindex_after_write` combining write + parse + index update)
- Approach: create fixtures (`LiveIndex`, `IndexedFile`), call public API, verify side effects
- Location: same file as main API, `#[cfg(test)]` module
- Example: `test_reindex_after_write_updates_index` in `src/protocol/edit.rs`

**E2E Tests:**
- Framework: None (all tests are unit/integration; no separate E2E suite)
- Rationale: MCP server invocation tested via manual testing or client-side tools

## Common Patterns

**Async Testing:**
- No special async test macro (no `#[tokio::test]`)
- Async code tested via guard: `if tokio::runtime::Handle::try_current().is_err() { return; }`
- Example from `src/live_index/git_temporal.rs`:
```rust
pub fn spawn_git_temporal_computation(index: SharedIndex, repo_root: PathBuf) {
    // Guard: only spawn if a tokio runtime is available (not the case in some sync tests).
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }

    tokio::spawn(async move { /* ... */ });
}
```

**Lock Testing:**
- Always expect locks to succeed: `.read().expect("lock")`, `.write().unwrap()`
- Test poisoning behavior explicitly if needed
- Example from `src/protocol/edit.rs`:
```rust
#[test]
fn test_reindex_after_write_updates_index() {
    let handle = crate::live_index::LiveIndex::empty();
    let content = b"fn hello() {}\n".to_vec();
    reindex_after_write(&handle, "src/lib.rs", content, LanguageId::Rust);

    let guard = handle.read().expect("lock");
    let file = guard.get_file("src/lib.rs");
    assert!(file.is_some());
}
```

**Error Testing:**
- Test both success and error paths
- Verify error message content if important
- Example from `src/protocol/edit.rs`:
```rust
#[test]
fn test_resolve_or_error_returns_not_found() {
    // ... setup indexed_file ...
    let result = resolve_or_error(&indexed_file, "nonexistent", None, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}

#[test]
fn test_resolve_or_error_handles_ambiguous() {
    // ... setup with multiple definitions ...
    let result = resolve_or_error(&indexed_file, "foo", None, None);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("Ambiguous"), "error was: {err}");
    assert!(err.contains("symbol_line"), "error was: {err}");
}
```

**Boundary Testing:**
- Test edge cases: empty inputs, zero values, large values, boundary conditions
- Example from `src/live_index/git_temporal.rs`:
```rust
#[test]
fn test_churn_bar_zero() {
    assert_eq!(churn_bar(0.0), "░░░░░░░░░░");
}

#[test]
fn test_churn_bar_clamps_above_one() {
    assert_eq!(churn_bar(1.5), "██████████");
}

#[test]
fn test_churn_bar_clamps_negative() {
    assert_eq!(churn_bar(-0.2), "░░░░░░░░░░");
}

#[test]
fn test_relative_time_negative() {
    assert_eq!(relative_time(-1.0), "just now");
}
```

## CI Configuration

**Test step (from `.github/workflows/ci.yml`):**
```yaml
- name: Run Rust tests
  run: cargo test --all-targets -- --test-threads=1
```

**Conditions:**
- Runs on every commit and PR to `main`
- Single-threaded enforced (prevents test isolation bugs)
- All targets (unit, integration, doc tests)
- Fails if any test panics or assertion fails

**Version sync tests (Python):**
```yaml
- name: Run version sync tests
  run: python -m unittest discover -s execution -p 'test_*.py'
```

---

*Testing analysis: 2026-03-14*
