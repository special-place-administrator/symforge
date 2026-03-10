/// Integration tests for the LiveIndex startup pipeline.
///
/// These tests prove that discovery → parsing → LiveIndex work together end-to-end,
/// and that the binary produces zero stdout bytes (RELY-04 CI gate).
use std::fs;
use std::path::Path;
use tempfile::tempdir;
use tokenizor_agentic_mcp::live_index::{IndexState, LiveIndex, ParseStatus};

// --------------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------------

fn write_file(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

// --------------------------------------------------------------------------
// Test: Full startup from tempdir with 5 valid source files
//
// Proves: LIDX-01 (files discovered), LIDX-02 (symbols queryable from RAM),
//         LiveIndex reports Ready state after clean load.
// --------------------------------------------------------------------------

#[test]
fn test_startup_loads_all_files() {
    let dir = tempdir().unwrap();

    write_file(dir.path(), "main.rs", "fn main() {}\nfn helper() {}");
    write_file(dir.path(), "app.py", "def run(): pass\ndef stop(): pass");
    write_file(dir.path(), "index.js", "function start() {}\nfunction end() {}");
    write_file(dir.path(), "lib.ts", "function util(): void {}\nfunction core(): void {}");
    write_file(dir.path(), "main.go", "package main\nfunc main() {}\nfunc run() {}");

    let shared = LiveIndex::load(dir.path()).unwrap();
    let index = shared.read().unwrap();

    assert_eq!(
        index.index_state(),
        IndexState::Ready,
        "LiveIndex should be Ready after loading 5 valid files"
    );
    assert_eq!(index.file_count(), 5, "should have 5 indexed files");
    assert!(
        index.symbol_count() > 0,
        "should have extracted symbols from valid source files"
    );

    // Verify each file is accessible by relative path
    assert!(
        index.get_file("main.rs").is_some(),
        "main.rs should be queryable"
    );
    assert!(
        index.get_file("app.py").is_some(),
        "app.py should be queryable"
    );
    assert!(
        index.get_file("index.js").is_some(),
        "index.js should be queryable"
    );
    assert!(
        index.get_file("lib.ts").is_some(),
        "lib.ts should be queryable"
    );
    assert!(
        index.get_file("main.go").is_some(),
        "main.go should be queryable"
    );
}

// --------------------------------------------------------------------------
// Test: Circuit breaker trips when >20% of files are garbage
//
// Proves: RELY-01 (circuit breaker fires on mass failure).
//
// Strategy: .rb files are discovered (Ruby is a known extension) but parsing
// returns FileOutcome::Failed because the language is not onboarded in
// parse_source. 3 valid Rust + 3 Ruby = 50% failure rate > 20% threshold.
// --------------------------------------------------------------------------

#[test]
fn test_circuit_breaker_trips_on_mass_failure() {
    let dir = tempdir().unwrap();

    // 3 valid Rust files → Parsed
    write_file(dir.path(), "a.rs", "fn alpha() {}");
    write_file(dir.path(), "b.rs", "fn beta() {}");
    write_file(dir.path(), "c.rs", "fn gamma() {}");

    // 3 Ruby files → Failed (language not onboarded in parse_source)
    // 3/6 = 50% > 20% threshold — circuit breaker must trip
    write_file(dir.path(), "x.rb", "def foo; end");
    write_file(dir.path(), "y.rb", "def bar; end");
    write_file(dir.path(), "z.rb", "def baz; end");

    let shared = LiveIndex::load(dir.path()).unwrap();
    let index = shared.read().unwrap();

    assert!(
        matches!(index.index_state(), IndexState::CircuitBreakerTripped { .. }),
        "CircuitBreakerTripped expected with 50% failure rate, got: {:?}",
        index.index_state()
    );
}

// --------------------------------------------------------------------------
// Test: Syntax error files produce PartialParse status but remain queryable
//
// Proves: RELY-02 (symbols retained on partial parse).
// --------------------------------------------------------------------------

#[test]
fn test_partial_parse_keeps_symbols() {
    let dir = tempdir().unwrap();

    // One file with a valid function AND a broken function signature.
    // tree-sitter error-recovers: valid() should still be extracted.
    write_file(dir.path(), "mixed.rs", "fn valid() {}\nfn broken(");

    let shared = LiveIndex::load(dir.path()).unwrap();
    let index = shared.read().unwrap();

    let file = index
        .get_file("mixed.rs")
        .expect("mixed.rs should be indexed");

    // The file must be PartialParse (not Failed) — tree-sitter recovers
    assert!(
        matches!(file.parse_status, ParseStatus::PartialParse { .. }),
        "syntax errors should produce PartialParse, got: {:?}",
        file.parse_status
    );

    // At least the valid() function should be in the symbols list
    assert!(
        !file.symbols.is_empty(),
        "symbols should be retained even when syntax errors are present"
    );

    let symbol_names: Vec<&str> = file.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(
        symbol_names.contains(&"valid"),
        "valid() function should be extracted despite later syntax error; symbols: {symbol_names:?}"
    );
}

// --------------------------------------------------------------------------
// Test: Content bytes stored for all files including failed ones
//
// Proves: LIDX-03 (zero disk I/O on read path — content is in memory).
// --------------------------------------------------------------------------

#[test]
fn test_content_bytes_stored_for_all_files() {
    let dir = tempdir().unwrap();

    let content_a = "fn hello() { println!(\"hello\"); }";
    let content_b = "def greet(): pass";
    write_file(dir.path(), "a.rs", content_a);
    write_file(dir.path(), "b.py", content_b);

    let shared = LiveIndex::load(dir.path()).unwrap();
    let index = shared.read().unwrap();

    let file_a = index.get_file("a.rs").expect("a.rs should be indexed");
    assert_eq!(
        file_a.content.len(),
        content_a.len(),
        "content bytes length should match file size"
    );
    assert_eq!(
        file_a.content,
        content_a.as_bytes(),
        "content bytes should match what was written to disk"
    );

    let file_b = index.get_file("b.py").expect("b.py should be indexed");
    assert_eq!(
        file_b.content.len(),
        content_b.len(),
        "content bytes length should match file size for Python file"
    );
    assert_eq!(
        file_b.content,
        content_b.as_bytes(),
        "content bytes should match what was written to disk"
    );
}

// --------------------------------------------------------------------------
// Test: Symbols queryable by file path after load
//
// Proves: LIDX-02 (symbols queryable from RAM).
// --------------------------------------------------------------------------

#[test]
fn test_symbols_queryable_by_file_path() {
    let dir = tempdir().unwrap();

    write_file(
        dir.path(),
        "funcs.rs",
        "fn alpha() {}\nfn beta() {}\nfn gamma() {}",
    );

    let shared = LiveIndex::load(dir.path()).unwrap();
    let index = shared.read().unwrap();

    let symbols = index.symbols_for_file("funcs.rs");
    assert!(
        symbols.len() >= 3,
        "should extract at least 3 functions; got: {:?}",
        symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"alpha"), "alpha() should be indexed");
    assert!(names.contains(&"beta"), "beta() should be indexed");
    assert!(names.contains(&"gamma"), "gamma() should be indexed");
}

// --------------------------------------------------------------------------
// Test: Stdout purity — binary stdout is empty (RELY-04 CI gate)
//
// Spawns the compiled binary, captures stdout, asserts it is empty.
// All tracing output goes to stderr. This is the Phase 1 completeness gate.
// --------------------------------------------------------------------------

#[test]
fn test_stdout_purity() {
    // Create a tempdir with a few valid source files and a .git directory
    // so find_git_root() anchors to the tempdir instead of walking up.
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    write_file(dir.path(), "main.rs", "fn main() {}");
    write_file(dir.path(), "lib.rs", "fn helper() {}");

    // Locate the compiled binary
    let exe = std::env::current_exe()
        .expect("should be able to find test executable path")
        .parent()
        .expect("test executable has a parent dir")
        .to_path_buf();

    // The binary is in the same profile directory (debug or release)
    let binary = exe.join("tokenizor_agentic_mcp.exe");
    if !binary.exists() {
        // On non-Windows or different naming, try without .exe
        let binary_unix = exe.join("tokenizor_agentic_mcp");
        if !binary_unix.exists() {
            // Binary not built yet (CI); skip gracefully but warn
            eprintln!(
                "SKIP test_stdout_purity: binary not found at {:?} or {:?}",
                binary, binary_unix
            );
            return;
        }
    }

    let binary_path = if binary.exists() {
        binary
    } else {
        exe.join("tokenizor_agentic_mcp")
    };

    let output = std::process::Command::new(&binary_path)
        .current_dir(dir.path())
        .env("RUST_LOG", "error") // suppress stderr noise in test output
        .output()
        .unwrap_or_else(|e| panic!("failed to run binary at {:?}: {e}", binary_path));

    assert!(
        output.stdout.is_empty(),
        "binary stdout must be empty (RELY-04): got {} bytes: {:?}",
        output.stdout.len(),
        String::from_utf8_lossy(&output.stdout)
    );
}

// --------------------------------------------------------------------------
// Test: Custom threshold via CircuitBreakerState::new() changes behavior
//
// Tests threshold configurability end-to-end using the constructor directly
// (more reliable than env var approach in parallel test runs).
//
// Proves: Circuit breaker threshold is configurable (AD-5).
// --------------------------------------------------------------------------

#[test]
fn test_custom_threshold_prevents_trip_at_high_threshold() {
    use tokenizor_agentic_mcp::live_index::store::CircuitBreakerState;

    // 10 files, 3 failures = 30% failure rate
    // With threshold=0.50 (50%), should NOT trip
    let cb = CircuitBreakerState::new(0.50);
    for _ in 0..7 {
        cb.record_success();
    }
    for i in 0..3 {
        cb.record_failure(&format!("file{i}.rb"), "not onboarded");
    }
    assert!(
        !cb.should_abort(),
        "30% failure rate should NOT trip a 50% threshold circuit breaker"
    );
}

#[test]
fn test_custom_threshold_trips_at_low_threshold() {
    use tokenizor_agentic_mcp::live_index::store::CircuitBreakerState;

    // 10 files, 2 failures = 20% failure rate
    // With threshold=0.10 (10%), 20% > 10% should trip
    let cb = CircuitBreakerState::new(0.10);
    for _ in 0..8 {
        cb.record_success();
    }
    for i in 0..2 {
        cb.record_failure(&format!("file{i}.rb"), "not onboarded");
    }
    assert!(
        cb.should_abort(),
        "20% failure rate should trip a 10% threshold circuit breaker"
    );
}
