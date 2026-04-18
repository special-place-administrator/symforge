//! TDD acceptance matrix — frecency-weighted file ranking.
//!
//! These tests pin the contract *before* the implementation exists. Every
//! test either compiles against the current empty `src/live_index/frecency.rs`
//! placeholder and fails at runtime with `todo!()`, or stubs out entirely
//! with `panic!("pending frecency implementation: …")`. Once the
//! implementation lands, pending assertions are replaced with real checks.
//!
//! Test matrix (from Implementation Notes §"Test matrix" on the
//! `[[SymForge Frecency-Weighted File Ranking]]` spec):
//!
//!   - Bump on every write tool (7 tests)
//!   - Bump on read tools: `get_symbol`, `get_symbol_context`,
//!     `get_file_context`, `get_file_content` (4 tests)
//!   - No-bump on discovery tools: `search_files`, `search_text`,
//!     `search_symbols` (3 tests — positive-feedback-loop guard)
//!   - Decay: `last_access_ts = now - 7d` → score ≈ 50% of peak
//!   - Fresh file with no row → baseline ranking, no error
//!   - Fusion: file_A bumped 10× 6 months ago vs file_B bumped 1× 5 min ago
//!     → file_B ranks higher with `rank_by="frecency"`
//!   - HEAD change: 100 commits → halve; 1000 commits → reset
//!   - Concurrency: 10 parallel bumps on same path → `hit_count == 10`
//!
//! Scope notes:
//!   - Six of these tools are not yet wired into
//!     `SymForgeServer::dispatch_tool_for_tests`: `get_file_context`,
//!     `get_file_content`, `get_symbol`, `get_symbol_context`, `search_text`,
//!     `search_symbols`. Those tests use `panic!("pending …")` until the
//!     implementer of todo #3 extends the harness.
//!   - `SYMFORGE_FRECENCY=1` gating, rusqlite setup, and time/HEAD
//!     injection APIs are all implementation details the pending todos own.
//!     Tests deliberately avoid touching unsafe `std::env::set_var` since the
//!     `todo!()` panic fires before any flag check matters.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{Value, json};
use symforge::live_index::LiveIndex;
use symforge::protocol::SymForgeServer;
use symforge::watcher::WatcherInfo;
use tempfile::TempDir;

// ─── Fixture ─────────────────────────────────────────────────────────────────

struct Fixture {
    _dir: TempDir,
    #[allow(dead_code)]
    root: PathBuf,
    server: SymForgeServer,
}

impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().to_path_buf();
        for (rel, content) in files {
            let path = root.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent dir");
            }
            fs::write(&path, content).expect("write fixture file");
        }
        let shared = LiveIndex::load(&root).expect("LiveIndex::load");
        let watcher_info = Arc::new(Mutex::new(WatcherInfo::default()));
        let server = SymForgeServer::new(
            shared,
            "frecency_ranking_test".to_string(),
            watcher_info,
            Some(root.clone()),
            None,
        );
        Self {
            _dir: dir,
            root,
            server,
        }
    }
}

async fn call(server: &SymForgeServer, tool: &str, params: Value) -> String {
    server.dispatch_tool_for_tests(tool, params).await
}

// ─── Bump on write tools (7 tests) ──────────────────────────────────────────
//
// Spec §"Bump hooks" — every edit tool must record a frecency bump for each
// path it modifies. Batch tools dedup per-invocation so a rename that touches
// N files yields N distinct bumps, not N × (bumps per file).

#[tokio::test]
async fn replace_symbol_body_bumps_frecency() {
    let fx = Fixture::new(&[("src/lib.rs", "fn hello() {}\n")]);

    let _ = call(
        &fx.server,
        "replace_symbol_body",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "new_body": "fn hello() { 1 }",
        }),
    )
    .await;

    todo!(
        "pending frecency implementation: expected src/lib.rs hit_count == 1 \
         after replace_symbol_body; verify via FrecencyStore::last_10_bumps"
    );
}

#[tokio::test]
async fn insert_symbol_bumps_frecency() {
    let fx = Fixture::new(&[("src/lib.rs", "fn hello() {}\n")]);

    let _ = call(
        &fx.server,
        "insert_symbol",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "content": "fn world() {}",
            "position": "after",
        }),
    )
    .await;

    todo!(
        "pending frecency implementation: expected src/lib.rs hit_count == 1 \
         after insert_symbol"
    );
}

#[tokio::test]
async fn delete_symbol_bumps_frecency() {
    let fx = Fixture::new(&[("src/lib.rs", "fn hello() {}\nfn world() {}\n")]);

    let _ = call(
        &fx.server,
        "delete_symbol",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
        }),
    )
    .await;

    todo!(
        "pending frecency implementation: expected src/lib.rs hit_count == 1 \
         after delete_symbol"
    );
}

#[tokio::test]
async fn edit_within_symbol_bumps_frecency() {
    let fx = Fixture::new(&[("src/lib.rs", "fn hello() {\n    old();\n}\n")]);

    let _ = call(
        &fx.server,
        "edit_within_symbol",
        json!({
            "path": "src/lib.rs",
            "name": "hello",
            "old_text": "old()",
            "new_text": "new()",
            "replace_all": false,
        }),
    )
    .await;

    todo!(
        "pending frecency implementation: expected src/lib.rs hit_count == 1 \
         after edit_within_symbol"
    );
}

#[tokio::test]
async fn batch_edit_bumps_each_touched_file_once() {
    let fx = Fixture::new(&[
        ("src/a.rs", "fn alpha() {\n    a_old();\n}\n"),
        ("src/b.rs", "fn beta() {\n    b_old();\n}\n"),
    ]);

    let _ = call(
        &fx.server,
        "batch_edit",
        json!({
            "edits": [
                {
                    "path": "src/a.rs",
                    "name": "alpha",
                    "operation": {
                        "type": "edit_within",
                        "old_text": "a_old()",
                        "new_text": "a_new()",
                    },
                },
                {
                    "path": "src/b.rs",
                    "name": "beta",
                    "operation": {
                        "type": "edit_within",
                        "old_text": "b_old()",
                        "new_text": "b_new()",
                    },
                },
            ]
        }),
    )
    .await;

    todo!(
        "pending frecency implementation: expected hit_count == 1 for BOTH \
         src/a.rs and src/b.rs after batch_edit (per-invocation dedup — one \
         bump per unique path, not per edit)"
    );
}

#[tokio::test]
async fn batch_rename_bumps_definition_and_call_site() {
    let fx = Fixture::new(&[
        ("src/lib.rs", "pub fn old_name() {}\n"),
        (
            "src/caller.rs",
            "use crate::old_name;\n\nfn caller() {\n    old_name();\n}\n",
        ),
    ]);

    let _ = call(
        &fx.server,
        "batch_rename",
        json!({
            "path": "src/lib.rs",
            "name": "old_name",
            "new_name": "new_name",
        }),
    )
    .await;

    todo!(
        "pending frecency implementation: expected hit_count == 1 for BOTH \
         src/lib.rs (definition) and src/caller.rs (call site) after \
         batch_rename"
    );
}

#[tokio::test]
async fn batch_insert_bumps_each_target_once() {
    let fx = Fixture::new(&[
        ("src/a.rs", "fn alpha() {}\n"),
        ("src/b.rs", "fn beta() {}\n"),
    ]);

    let _ = call(
        &fx.server,
        "batch_insert",
        json!({
            "content": "fn shared() {}\n",
            "position": "after",
            "targets": [
                { "path": "src/a.rs", "name": "alpha" },
                { "path": "src/b.rs", "name": "beta" },
            ],
        }),
    )
    .await;

    todo!(
        "pending frecency implementation: expected hit_count == 1 for BOTH \
         src/a.rs and src/b.rs after batch_insert"
    );
}

// ─── Bump on read tools (4 tests) ───────────────────────────────────────────
//
// Spec §"Bump — invisible, no API change". These tools are read-only but are
// commitment signals: the agent chose to load this specific file/symbol.
// They are NOT yet wired into `dispatch_tool_for_tests`; todo #3 owns that
// wire-up. Tests stand as the contract.

#[tokio::test]
async fn get_file_context_bumps_frecency() {
    panic!(
        "pending frecency implementation: expected src/foo.rs hit_count == 1 \
         after get_file_context(path=\"src/foo.rs\"); requires \
         dispatch_tool_for_tests extension to wire get_file_context + \
         FrecencyStore::last_10_bumps check"
    );
}

#[tokio::test]
async fn get_file_content_bumps_frecency() {
    panic!(
        "pending frecency implementation: expected src/foo.rs hit_count == 1 \
         after get_file_content(path=\"src/foo.rs\"); requires \
         dispatch_tool_for_tests extension to wire get_file_content + \
         FrecencyStore::last_10_bumps check"
    );
}

#[tokio::test]
async fn get_symbol_bumps_frecency() {
    panic!(
        "pending frecency implementation: expected src/foo.rs hit_count == 1 \
         after get_symbol(symbol_spec=\"src/foo.rs::thing\"); requires \
         dispatch_tool_for_tests extension to wire get_symbol + \
         FrecencyStore::last_10_bumps check"
    );
}

#[tokio::test]
async fn get_symbol_context_bumps_frecency() {
    panic!(
        "pending frecency implementation: expected src/foo.rs hit_count == 1 \
         after get_symbol_context(symbol_spec=\"src/foo.rs::thing\"); \
         requires dispatch_tool_for_tests extension to wire \
         get_symbol_context + FrecencyStore::last_10_bumps check"
    );
}

// ─── No-bump on discovery tools (3 tests) ───────────────────────────────────
//
// Spec §"Search tools deliberately do NOT bump" — positive-feedback-loop
// prevention. If search_files bumped, the top-ranked file would compound
// toward permanent dominance on every subsequent search. This is the
// single most important invariant of the whole feature.

#[tokio::test]
async fn search_files_does_not_bump() {
    let fx = Fixture::new(&[
        ("src/alpha.rs", "pub fn alpha() {}\n"),
        ("src/beta.rs", "pub fn beta() {}\n"),
    ]);

    // search_files IS wired into dispatch_tool_for_tests — exercise the real
    // handler so the pending assertion can eventually verify the bump log is
    // empty for every path touched by the search.
    let _ = call(
        &fx.server,
        "search_files",
        json!({ "query": "alpha", "limit": 10 }),
    )
    .await;

    todo!(
        "pending frecency implementation: expected FrecencyStore::last_10_bumps \
         to be EMPTY after search_files — discovery tools must not bump"
    );
}

#[tokio::test]
async fn search_text_does_not_bump() {
    panic!(
        "pending frecency implementation: expected FrecencyStore::last_10_bumps \
         to be EMPTY after search_text — discovery tools must not bump \
         (positive-feedback-loop prevention); requires dispatch_tool_for_tests \
         extension to wire search_text"
    );
}

#[tokio::test]
async fn search_symbols_does_not_bump() {
    panic!(
        "pending frecency implementation: expected FrecencyStore::last_10_bumps \
         to be EMPTY after search_symbols — discovery tools must not bump \
         (positive-feedback-loop prevention); requires dispatch_tool_for_tests \
         extension to wire search_symbols"
    );
}

// ─── Decay (1 test) ─────────────────────────────────────────────────────────
//
// Implementation Notes §"Decay + fusion starting parameters":
// `score = hit_count * exp(-ln(2) * (now - last_access_ts) / 604800)`.
// A row with `last_access_ts = now - 7d` and `hit_count = 1` decays to
// exactly 0.5 (7-day half-life).

#[tokio::test]
async fn score_decays_to_half_after_seven_days() {
    panic!(
        "pending frecency implementation: expected \
         FrecencyStore::score(path, now) ≈ 0.5 * hit_count when \
         last_access_ts = now - 604800; requires FrecencyStore::bump_at_time \
         or similar time-injection API"
    );
}

// ─── Fresh file baseline (1 test) ───────────────────────────────────────────
//
// A file with no frecency row must receive score == 0 (not an error, not a
// negative number) so the fusion math treats it as baseline.

#[tokio::test]
async fn fresh_file_with_no_row_returns_baseline_score() {
    let fx = Fixture::new(&[("src/fresh.rs", "pub fn fresh() {}\n")]);

    // Exercise a search against a freshly-created file that has never been
    // bumped. Must not error when the ranker consults frecency.
    let result = call(
        &fx.server,
        "search_files",
        json!({ "query": "fresh", "limit": 10 }),
    )
    .await;

    assert!(
        !result.to_lowercase().contains("error"),
        "search_files must not error on a file with no frecency row; got:\n{result}"
    );

    todo!(
        "pending frecency implementation: expected \
         FrecencyStore::score(src/fresh.rs, now) == 0.0 for a fresh file \
         (no row in the `frecency` table)"
    );
}

// ─── Fusion (1 test) ────────────────────────────────────────────────────────
//
// Implementation Notes §"Test matrix": "File touched 5 min ago outranks file
// touched 6 months ago with 10× hits" — single test, quoted verbatim. This
// pins the weight (exp(-ln(2) * 182.5d / 7d) ≈ 1.6e-8 vs 10 × ~1.0).

#[tokio::test]
async fn recent_single_bump_outranks_old_ten_bumps() {
    panic!(
        "pending frecency implementation: expected file_B (1 bump 5 min ago) \
         to outrank file_A (10 bumps 6 months ago) in search_files with \
         rank_by=\"frecency\"; requires FrecencyStore::bump_at_time + \
         SearchFilesInput::rank_by field"
    );
}

// ─── HEAD-change reset (2 tests) ────────────────────────────────────────────
//
// Implementation Notes §"Reset-on-HEAD-change: graduated, not binary":
// `<50` commits no-op, `50-500` halve scores, `>500` or branch change zero.

#[tokio::test]
async fn head_change_halves_scores_at_100_commits() {
    panic!(
        "pending frecency implementation: expected all frecency scores halved \
         after reset_or_halve_on_head_change(prev, curr, commit_distance=100); \
         requires FrecencyStore::reset_or_halve_on_head_change + git test \
         repo helper"
    );
}

#[tokio::test]
async fn head_change_resets_scores_at_1000_commits() {
    panic!(
        "pending frecency implementation: expected all frecency scores zeroed \
         after reset_or_halve_on_head_change(prev, curr, commit_distance=1000); \
         requires FrecencyStore::reset_or_halve_on_head_change + git test \
         repo helper"
    );
}

// ─── Concurrency (1 test) ───────────────────────────────────────────────────
//
// Implementation Notes §"Storage: skip JSON, use SQLite from day 1": SQLite
// with WAL mode handles parallel writes. 10 parallel bumps on the same path
// must land 10 distinct increments (no lost updates from race conditions).

#[tokio::test]
async fn ten_parallel_bumps_yield_hit_count_ten() {
    panic!(
        "pending frecency implementation: expected hit_count == 10 after 10 \
         parallel threads each call FrecencyStore::bump([\"src/x.rs\"]); \
         requires FrecencyStore public API + std::thread::scope harness"
    );
}
