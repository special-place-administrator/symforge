//! Worktree-awareness acceptance tests (TDD red state).
//!
//! Mirrors §4 of the spec at `wiki/concepts/SymForge Worktree Awareness.md`
//! (Obsidian: [[SymForge Worktree Awareness]]) plus the expanded matrix in
//! its Implementation Notes (added 2026-04-17).
//!
//! Every test here is expected to fail until:
//!   - `src/worktree.rs` ships canonicalization, `git worktree list` cache,
//!     and `resolve_target_path`.
//!   - The 7 edit handlers listed in spec §2.1 accept `working_directory`
//!     and emit `wrote_to` / `indexed_path` / `rerouted` in their response.
//!   - `README.md` documents the parameter with one example.
//!
//! The feature is flagged behind `SYMFORGE_WORKTREE_AWARE=1`. Each test
//! enables the flag before building its fixture. The tests set env vars, so
//! they MUST run single-threaded — project `CLAUDE.md` already mandates
//! `--test-threads=1` for this crate.
//!
//! Harness shells out to the system `git` binary (version 2.5+ for
//! `git worktree add`). On dev machines and CI this is always present.
//!
//! Response-format assertions in this file (`rerouted: true`,
//! `WorkingDirectoryNotARecognizedWorktree`, `TargetFileMissing`, etc.) are
//! a TDD-driven *choice* — the spec shows a JSON shape but does not
//! prescribe the formatter. Implementation task 3 either matches these
//! strings or updates this file. Keep the contract explicit either way.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{Value, json};
use symforge::live_index::LiveIndex;
use symforge::protocol::SymForgeServer;
use symforge::watcher::WatcherInfo;
use tempfile::TempDir;

// ─── Constants ──────────────────────────────────────────────────────────────

/// The 7 edit tools listed in spec §2.1.
const EDIT_TOOLS: &[&str] = &[
    "edit_within_symbol",
    "replace_symbol_body",
    "insert_symbol",
    "delete_symbol",
    "batch_edit",
    "batch_insert",
    "batch_rename",
];

/// Shared sample content for tests that just need a parseable Rust file.
const HELLO_RS: &str =
    "fn hello() {\n    println!(\"hello\");\n}\n\nfn world() {\n    println!(\"world\");\n}\n";

// ─── Feature flag helpers ───────────────────────────────────────────────────

fn enable_feature_flag() {
    // SAFETY: tests are `--test-threads=1` per project `CLAUDE.md`. Rust 2024
    // requires `unsafe` for env-var mutation; see `.planning/STATE.md:66`.
    unsafe { std::env::set_var("SYMFORGE_WORKTREE_AWARE", "1") };
}

// ─── Git helpers (shell out; git is required dev tooling) ───────────────────

fn run_git(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("git spawn failed for {args:?}: {e}"));
    if !out.status.success() {
        panic!(
            "git {args:?} in {cwd:?} failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn git_init_with_initial_commit(root: &Path) {
    run_git(root, &["init", "-b", "main"]);
    run_git(root, &["config", "user.email", "test@example.com"]);
    run_git(root, &["config", "user.name", "wtaware-test"]);
    run_git(root, &["add", "-A"]);
    run_git(root, &["commit", "-m", "initial"]);
}

fn git_worktree_add(indexed_root: &Path, worktree_root: &Path, branch: &str) {
    if let Some(parent) = worktree_root.parent() {
        fs::create_dir_all(parent).expect("worktree parent dir");
    }
    run_git(
        indexed_root,
        &[
            "worktree",
            "add",
            "-b",
            branch,
            worktree_root.to_str().expect("utf-8 worktree path"),
        ],
    );
}

// ─── Fixtures ───────────────────────────────────────────────────────────────

/// Single-tree fixture: indexed repo, no worktree.
///
/// Used for backward-compat and "`working_directory` == indexed root" cases,
/// plus the cache-refresh test (which creates a worktree mid-session).
struct IndexedOnlyFixture {
    _dir: TempDir,
    root: PathBuf,
    server: SymForgeServer,
}

impl IndexedOnlyFixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let container = dir.path().to_path_buf();
        let root = container.join("main");
        fs::create_dir_all(&root).expect("create main dir");
        for (rel, content) in files {
            let path = root.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent dir");
            }
            fs::write(&path, content).expect("write fixture file");
        }
        git_init_with_initial_commit(&root);

        let shared = LiveIndex::load(&root).expect("LiveIndex::load");
        let watcher_info = Arc::new(Mutex::new(WatcherInfo::default()));
        let server = SymForgeServer::new(
            shared,
            "worktree_awareness_indexed_only".to_string(),
            watcher_info,
            Some(root.clone()),
            None,
        );
        Self { _dir: dir, root, server }
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.root.join(rel)).expect("read file")
    }
}

/// Two-tree fixture: an indexed repo plus one parallel worktree at
/// `<container>/wt_one` on branch `tentacle/test`.
struct WorktreeFixture {
    _dir: TempDir,
    root: PathBuf,
    worktree_root: PathBuf,
    server: SymForgeServer,
}

impl WorktreeFixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let container = dir.path().to_path_buf();
        let root = container.join("main");
        fs::create_dir_all(&root).expect("create main dir");
        for (rel, content) in files {
            let path = root.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent dir");
            }
            fs::write(&path, content).expect("write fixture file");
        }
        git_init_with_initial_commit(&root);

        let worktree_root = container.join("wt_one");
        git_worktree_add(&root, &worktree_root, "tentacle/test");

        let shared = LiveIndex::load(&root).expect("LiveIndex::load");
        let watcher_info = Arc::new(Mutex::new(WatcherInfo::default()));
        let server = SymForgeServer::new(
            shared,
            "worktree_awareness_test".to_string(),
            watcher_info,
            Some(root.clone()),
            None,
        );
        Self {
            _dir: dir,
            root,
            worktree_root,
            server,
        }
    }

    fn container(&self) -> PathBuf {
        self.root.parent().expect("container").to_path_buf()
    }

    fn read_indexed(&self, rel: &str) -> String {
        fs::read_to_string(self.root.join(rel)).expect("read indexed file")
    }

    fn read_worktree(&self, rel: &str) -> String {
        fs::read_to_string(self.worktree_root.join(rel)).expect("read worktree file")
    }
}

// ─── Call helper ────────────────────────────────────────────────────────────

async fn call(server: &SymForgeServer, tool: &str, params: Value) -> String {
    server.dispatch_tool_for_tests(tool, params).await
}

fn assert_contains(result: &str, needle: &str) {
    assert!(
        result.contains(needle),
        "expected result to contain `{needle}`; result was:\n{result}"
    );
}

fn assert_not_contains(result: &str, needle: &str) {
    assert!(
        !result.contains(needle),
        "expected result NOT to contain `{needle}`; result was:\n{result}"
    );
}

// ─── §4 Acceptance criteria (7 tests, 1:1 mapping) ──────────────────────────

/// AC1: Every write tool listed in spec §2.1 accepts a `working_directory`
/// parameter.
///
/// Drives each tool through `dispatch_tool_for_tests` with a payload that
/// includes `working_directory`, and asserts the dispatcher does not
/// reject the param at the input-struct layer (`"invalid tool parameters"`
/// would indicate missing `#[serde(default)] pub working_directory` on
/// the tool's input struct).
#[tokio::test]
async fn ac1_all_seven_edit_tools_accept_working_directory_param() {
    enable_feature_flag();
    // Lock the tool set in one place; future edit tools must be added here
    // before the file is re-landed.
    assert_eq!(EDIT_TOOLS.len(), 7, "spec §2.1 lists exactly 7 write tools");

    let fx = WorktreeFixture::new(&[("src/lib.rs", HELLO_RS)]);
    let wt_arg = fx.worktree_root.to_str().unwrap().to_string();

    // Minimum payload per tool that (a) parses, (b) includes
    // `working_directory`. Most tools will still return a downstream
    // error because the dummy payloads skip real arguments, but that is
    // fine — AC1 only asserts input-struct acceptance, not happy-path
    // success.
    let payloads: Vec<(&str, Value)> = vec![
        (
            "edit_within_symbol",
            json!({
                "path": "src/lib.rs",
                "name": "hello",
                "old_text": "hello",
                "new_text": "HELLO",
                "working_directory": wt_arg.clone(),
            }),
        ),
        (
            "replace_symbol_body",
            json!({
                "path": "src/lib.rs",
                "name": "hello",
                "new_body": "fn hello() {}",
                "working_directory": wt_arg.clone(),
            }),
        ),
        (
            "insert_symbol",
            json!({
                "path": "src/lib.rs",
                "name": "hello",
                "content": "fn added() {}",
                "working_directory": wt_arg.clone(),
            }),
        ),
        (
            "delete_symbol",
            json!({
                "path": "src/lib.rs",
                "name": "hello",
                "working_directory": wt_arg.clone(),
            }),
        ),
        (
            "batch_edit",
            json!({
                "edits": [],
                "working_directory": wt_arg.clone(),
            }),
        ),
        (
            "batch_insert",
            json!({
                "content": "fn x() {}",
                "position": "after",
                "targets": [],
                "working_directory": wt_arg.clone(),
            }),
        ),
        (
            "batch_rename",
            json!({
                "path": "src/lib.rs",
                "name": "hello",
                "new_name": "greet",
                "working_directory": wt_arg.clone(),
            }),
        ),
    ];

    assert_eq!(
        payloads.len(),
        EDIT_TOOLS.len(),
        "AC1 payload table must cover every tool in EDIT_TOOLS"
    );

    for (tool, params) in payloads {
        let result = call(&fx.server, tool, params).await;
        assert!(
            !result.contains("invalid tool parameters"),
            "tool `{tool}` rejected the `working_directory` param; \
             response was:\n{result}"
        );
        assert!(
            !result.starts_with("dispatch_tool_for_tests: unknown tool"),
            "tool `{tool}` is not wired into dispatch_tool_for_tests: {result}"
        );
    }
}

/// AC2: When `working_directory` is omitted, behavior is byte-identical to
/// today (indexed-path write, pre-existing response format).
#[tokio::test]
async fn ac2_omitted_working_directory_is_byte_identical() {
    enable_feature_flag();
    let fx = IndexedOnlyFixture::new(&[("src/lib.rs", HELLO_RS)]);

    let result = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() {\n    println!(\"HELLO\");\n}",
            // intentionally NO `working_directory`
        }),
    )
    .await;

    // Pre-existing contract lines — must remain verbatim so omitted-param
    // callers see byte-identical output:
    assert_contains(&result, "Edit safety: structural-edit-safe");
    assert_contains(&result, "Path authority: repository-bound");
    assert_contains(&result, "Write semantics: atomic write + reindex");
    // When the param is omitted, the reroute marker must NOT appear — that
    // is how callers recognise "no reroute happened, default path".
    assert_not_contains(&result, "rerouted: true");

    let on_disk = fx.read("src/lib.rs");
    assert!(
        on_disk.contains("HELLO"),
        "indexed copy must be written when working_directory is omitted: {on_disk}",
    );
}

/// AC3: `working_directory` at a known worktree routes the write to the
/// worktree's copy and leaves the indexed copy untouched.
#[tokio::test]
async fn ac3_working_directory_at_known_worktree_writes_to_worktree() {
    enable_feature_flag();
    let fx = WorktreeFixture::new(&[("src/lib.rs", HELLO_RS)]);
    let wt_arg = fx.worktree_root.to_str().unwrap().to_string();

    let result = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() {\n    println!(\"HELLO_WT\");\n}",
            "working_directory": wt_arg,
        }),
    )
    .await;

    // §2.3 response shape — every rerouted write must self-describe.
    assert_contains(&result, "rerouted: true");
    assert_contains(&result, "wrote_to");
    assert_contains(&result, "indexed_path");

    let wt_after = fx.read_worktree("src/lib.rs");
    assert!(
        wt_after.contains("HELLO_WT"),
        "worktree copy must receive the write: {wt_after}",
    );
    let indexed_after = fx.read_indexed("src/lib.rs");
    assert!(
        !indexed_after.contains("HELLO_WT"),
        "indexed copy must NOT be touched when rerouting: {indexed_after}",
    );
}

/// AC4: `working_directory` at a path that is NOT a recognized worktree
/// returns an error and writes zero bytes.
#[tokio::test]
async fn ac4_working_directory_at_unknown_path_errors_and_writes_nothing() {
    enable_feature_flag();
    let fx = WorktreeFixture::new(&[("src/lib.rs", HELLO_RS)]);
    // A real, existing directory that isn't registered with the indexed
    // repo's `git worktree list`.
    let bogus = tempfile::tempdir().expect("bogus tempdir");

    let indexed_before = fx.read_indexed("src/lib.rs");
    let wt_before = fx.read_worktree("src/lib.rs");

    let result = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() {\n    println!(\"SHOULD_NOT_LAND\");\n}",
            "working_directory": bogus.path().to_str().unwrap(),
        }),
    )
    .await;

    // Error surface — implementation returns
    // `WorkingDirectoryNotARecognizedWorktree` per spec §2.2.
    assert_contains(&result, "WorkingDirectoryNotARecognizedWorktree");
    // Actionable hint requirement from spec §2.2.
    assert_contains(&result, "git worktree list");

    // Zero bytes written anywhere.
    assert_eq!(
        fx.read_indexed("src/lib.rs"),
        indexed_before,
        "indexed copy modified despite unknown working_directory",
    );
    assert_eq!(
        fx.read_worktree("src/lib.rs"),
        wt_before,
        "worktree copy modified despite unknown working_directory",
    );
}

/// AC5: Response includes `wrote_to`, `indexed_path`, and `rerouted` fields
/// so callers can verify the actual write target.
#[tokio::test]
async fn ac5_response_surfaces_wrote_to_and_indexed_path_and_rerouted() {
    enable_feature_flag();
    let fx = WorktreeFixture::new(&[("src/lib.rs", HELLO_RS)]);
    let wt_arg = fx.worktree_root.to_str().unwrap().to_string();

    let result = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() { }",
            "working_directory": wt_arg,
        }),
    )
    .await;

    assert_contains(&result, "wrote_to");
    assert_contains(&result, "indexed_path");
    assert_contains(&result, "rerouted: true");
    // The rerouted write target must appear verbatim in the response so the
    // caller can log / verify it.
    assert_contains(&result, fx.worktree_root.to_str().unwrap());
}

/// AC6: Test matrix covers the two cases not covered by AC2-AC4:
///  (a) `working_directory` == indexed root → `rerouted: false`.
///  (b) `working_directory` is a worktree where the file doesn't exist at
///      HEAD → `TargetFileMissing` with an actionable hint.
#[tokio::test]
async fn ac6_matrix_covers_indexed_root_and_missing_file_cases() {
    enable_feature_flag();
    let fx = WorktreeFixture::new(&[("src/lib.rs", HELLO_RS)]);

    // Case A: working_directory = indexed root → same behaviour as omitted.
    let result_a = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() {}",
            "working_directory": fx.root.to_str().unwrap(),
        }),
    )
    .await;
    assert_contains(&result_a, "rerouted: false");

    // Case B: working_directory = worktree, but file is missing at HEAD in
    // that worktree. Simulate by deleting it before the call.
    fs::remove_file(fx.worktree_root.join("src/lib.rs")).expect("remove worktree file");
    let result_b = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() {}",
            "working_directory": fx.worktree_root.to_str().unwrap(),
        }),
    )
    .await;
    assert_contains(&result_b, "TargetFileMissing");
    // Actionable hint from spec §2.2.
    assert_contains(&result_b, "git ls-tree");
}

/// AC7: `README.md` documents the `working_directory` parameter with an
/// example.
#[test]
fn ac7_readme_documents_working_directory_parameter() {
    let readme_path = format!("{}/README.md", env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(&readme_path)
        .unwrap_or_else(|e| panic!("README.md unreadable at {readme_path}: {e}"));
    assert!(
        readme.contains("working_directory"),
        "README.md must document the `working_directory` parameter",
    );
    assert!(
        readme.to_lowercase().contains("worktree"),
        "README.md must reference git worktrees",
    );
}

// ─── Implementation Notes expanded matrix (5 tests) ─────────────────────────

/// Matrix #1 — Windows path separator normalization: `C:\repo` vs `C:/repo`.
/// Gated to Windows because `\` is a path separator only there; on POSIX it
/// is a legal filename character and the assertion would be nonsense.
#[cfg(target_os = "windows")]
#[tokio::test]
async fn matrix_windows_forward_and_back_slash_canonicalize_equal() {
    enable_feature_flag();
    let fx = WorktreeFixture::new(&[("src/lib.rs", HELLO_RS)]);
    let back = fx.worktree_root.to_str().unwrap().to_string();
    let forward = back.replace('\\', "/");

    let result = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() {}",
            "working_directory": forward,
        }),
    )
    .await;

    assert_contains(&result, "rerouted: true");
    assert_not_contains(&result, "WorkingDirectoryNotARecognizedWorktree");
}

/// Matrix #2 — Mixed-case drive letter / path on Windows' case-insensitive
/// filesystem. Canonicalization must treat `C:\Repo` and `c:\repo` as the
/// same worktree.
#[cfg(target_os = "windows")]
#[tokio::test]
async fn matrix_mixed_case_windows_canonicalize_equal() {
    enable_feature_flag();
    let fx = WorktreeFixture::new(&[("src/lib.rs", HELLO_RS)]);
    let original = fx.worktree_root.to_str().unwrap().to_string();
    // Flip-case selected characters (drive letter, a couple of segments) so
    // the string differs from any cached entry.
    let mixed: String = original
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i % 3 == 0 {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();

    let result = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() {}",
            "working_directory": mixed,
        }),
    )
    .await;

    assert_contains(&result, "rerouted: true");
}

/// Matrix #3 — Trailing-slash asymmetry: the same directory passed with and
/// without a trailing separator must canonicalize to the same worktree.
#[tokio::test]
async fn matrix_trailing_slash_canonicalize_equal() {
    enable_feature_flag();
    let fx = WorktreeFixture::new(&[("src/lib.rs", HELLO_RS)]);
    let base = fx.worktree_root.to_str().unwrap().to_string();
    let sep = std::path::MAIN_SEPARATOR;
    let with_trailing = if base.ends_with(sep) {
        base.clone()
    } else {
        format!("{base}{sep}")
    };

    let result = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() {}",
            "working_directory": with_trailing,
        }),
    )
    .await;

    assert_contains(&result, "rerouted: true");
    assert_not_contains(&result, "WorkingDirectoryNotARecognizedWorktree");
}

/// Matrix #4 — Two worktrees, two writes: no cross-talk.
///
/// The `#[tokio::test]` default runtime serialises task execution, so this
/// does not exercise a true data race — the spec already out-of-scopes
/// file-level locking (§5). What we verify is that *routing* is
/// per-call-correct: wt1's write lands in wt1, wt2's in wt2, indexed root
/// is untouched by either.
#[tokio::test]
async fn matrix_two_worktrees_concurrent_calls_have_no_crosstalk() {
    enable_feature_flag();
    let fx = WorktreeFixture::new(&[("src/lib.rs", HELLO_RS)]);

    let wt2 = fx.container().join("wt_two");
    git_worktree_add(&fx.root, &wt2, "tentacle/test-two");

    let wt1_arg = fx.worktree_root.to_str().unwrap().to_string();
    let wt2_arg = wt2.to_str().unwrap().to_string();

    let r1 = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() { /* wt1 */ }",
            "working_directory": wt1_arg,
        }),
    )
    .await;
    let r2 = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() { /* wt2 */ }",
            "working_directory": wt2_arg,
        }),
    )
    .await;

    assert_contains(&r1, "rerouted: true");
    assert_contains(&r2, "rerouted: true");

    let c1 = fs::read_to_string(fx.worktree_root.join("src/lib.rs")).unwrap();
    let c2 = fs::read_to_string(wt2.join("src/lib.rs")).unwrap();
    assert!(
        c1.contains("wt1") && !c1.contains("wt2"),
        "wt1 cross-written:\n{c1}",
    );
    assert!(
        c2.contains("wt2") && !c2.contains("wt1"),
        "wt2 cross-written:\n{c2}",
    );
    let indexed = fx.read_indexed("src/lib.rs");
    assert!(
        !indexed.contains("wt1") && !indexed.contains("wt2"),
        "indexed root polluted:\n{indexed}",
    );
}

/// Matrix #5 — Cache refresh: a worktree created mid-session must be
/// accepted on the next tool call, not rejected as unknown.
#[tokio::test]
async fn matrix_cache_refresh_newly_created_worktree_accepted() {
    enable_feature_flag();
    let fx = IndexedOnlyFixture::new(&[("src/lib.rs", HELLO_RS)]);
    let container = fx.root.parent().expect("container").to_path_buf();
    let lateborn = container.join("wt_lateborn");

    // First call: the worktree doesn't exist yet → must be rejected.
    let pre = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() {}",
            "working_directory": lateborn.to_str().unwrap(),
        }),
    )
    .await;
    assert_contains(&pre, "WorkingDirectoryNotARecognizedWorktree");

    // Create the worktree mid-session.
    git_worktree_add(&fx.root, &lateborn, "tentacle/lateborn");

    // Second call: the same path must now be accepted via cache refresh.
    let post = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() {}",
            "working_directory": lateborn.to_str().unwrap(),
        }),
    )
    .await;
    assert_contains(&post, "rerouted: true");
}

// ─── Item 4: health misuse counter + conventions answer ─────────────────────

/// `health` surfaces a rolling "last hour" misuse counter whose value bumps
/// each time an edit tool is called without `working_directory` while
/// `SYMFORGE_WORKTREE_AWARE=1` is set.
#[tokio::test]
async fn health_surfaces_worktree_misuse_counter() {
    enable_feature_flag();
    let fx = IndexedOnlyFixture::new(&[("src/lib.rs", HELLO_RS)]);

    // Baseline: before any edit calls, counter reads 0.
    let baseline = call(&fx.server, "health", json!({})).await;
    assert_contains(&baseline, "Worktree-awareness misuse");
    assert_contains(
        &baseline,
        "edit tool calls without working_directory (last hour): 0",
    );

    // One edit call without `working_directory` should bump the counter.
    let _ = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() {\n    println!(\"bump\");\n}",
        }),
    )
    .await;
    let after_one = call(&fx.server, "health", json!({})).await;
    assert_contains(
        &after_one,
        "edit tool calls without working_directory (last hour): 1",
    );

    // A second call using a DIFFERENT edit tool must also increment.
    let _ = call(
        &fx.server,
        "edit_within_symbol",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "old_text": "bump",
            "new_text": "bumped",
        }),
    )
    .await;
    let after_two = call(&fx.server, "health", json!({})).await;
    assert_contains(
        &after_two,
        "edit tool calls without working_directory (last hour): 2",
    );
}

/// When `working_directory` is supplied, the misuse counter must NOT
/// increment — the caller did the right thing.
#[tokio::test]
async fn health_misuse_counter_does_not_increment_when_working_directory_supplied() {
    enable_feature_flag();
    let fx = WorktreeFixture::new(&[("src/lib.rs", HELLO_RS)]);
    let wt_arg = fx.worktree_root.to_str().unwrap().to_string();

    // Call an edit tool WITH `working_directory`.
    let _ = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() { }",
            "working_directory": wt_arg,
        }),
    )
    .await;

    let after = call(&fx.server, "health", json!({})).await;
    assert_contains(
        &after,
        "edit tool calls without working_directory (last hour): 0",
    );
}

/// `conventions` output documents the `working_directory` parameter,
/// `SYMFORGE_WORKTREE_AWARE` flag, and points at the README — so agents
/// that query project conventions learn how to call edit tools from
/// inside a worktree.
#[tokio::test]
async fn conventions_surfaces_worktree_awareness_guidance() {
    let fx = IndexedOnlyFixture::new(&[("src/lib.rs", HELLO_RS)]);

    let output = call(&fx.server, "conventions", json!({})).await;

    assert_contains(&output, "Worktree awareness");
    assert_contains(&output, "working_directory");
    assert_contains(&output, "SYMFORGE_WORKTREE_AWARE");
    assert_contains(&output, "README");
}
