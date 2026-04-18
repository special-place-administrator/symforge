//! Per-workspace frecency scoring for file ranking.
//!
//! SQLite-backed, WAL mode, bumped on commitment tools (reads/edits of known
//! files) and never on discovery tools (search). Decays on a 7-day half-life.
//!
//! This module owns only the storage + scoring layer. The `RankSignal` and
//! `EditHook` impls that plug it into search/edit pipelines are wired by the
//! consumer sites (see todo #3 in the frecency-ranking tentacle).
//!
//! Spec: `wiki/concepts/SymForge Frecency-Weighted File Ranking.md`.

use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Half-life for frecency decay, in seconds. 7 days.
pub const HALF_LIFE_SECS: i64 = 7 * 24 * 60 * 60;

/// Commit-distance thresholds for the graduated HEAD-change reset policy.
pub const RESET_NOOP_THRESHOLD: u32 = 50;
pub const RESET_HALVE_THRESHOLD: u32 = 500;

const META_LAST_HEAD: &str = "last_head_sha";

/// Outcome of applying the HEAD-change reset policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetOutcome {
    /// No action taken (first session, same HEAD, or distance below threshold).
    NoOp,
    /// All `hit_count` values were halved.
    Halved,
    /// All `hit_count` values were zeroed.
    Zeroed,
}

/// A single frecency row as surfaced by `last_10_bumps` in health output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BumpEntry {
    pub path: PathBuf,
    pub last_access_ts: i64,
    pub hit_count: i64,
}

/// SQLite-backed per-workspace frecency store.
///
/// Persists to `.symforge/frecency.db` (see `paths::SYMFORGE_FRECENCY_DB_PATH`)
/// or an in-memory DB for tests. All access routes through an internal `Mutex`
/// so the store is `Sync` and safe to share across concurrent bump callers.
pub struct FrecencyStore {
    conn: Mutex<Connection>,
}

impl FrecencyStore {
    /// Open a file-backed store, creating the DB and parent directory if missing.
    pub fn open(db_path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = db_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|_| rusqlite::Error::InvalidPath(parent.to_path_buf()))?;
        }
        let conn = Connection::open(db_path)?;
        // Best-effort WAL; silently falls back on in-memory/read-only FS.
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    /// Open an in-memory store. For tests and ephemeral use.
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("frecency mutex poisoned");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS frecency (
                path TEXT PRIMARY KEY,
                last_access_ts INTEGER NOT NULL,
                hit_count INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
             );",
        )?;
        Ok(())
    }

    /// Bump the given paths at `now_ts`. Each path increments `hit_count` by 1
    /// and sets `last_access_ts = now_ts`. The caller is responsible for
    /// deduplicating within a single invocation (per the Implementation Notes
    /// §"Bump dedup per tool invocation").
    pub fn bump(&self, paths: &[PathBuf], now_ts: i64) -> rusqlite::Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().expect("frecency mutex poisoned");
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO frecency(path, last_access_ts, hit_count)
                 VALUES (?1, ?2, 1)
                 ON CONFLICT(path) DO UPDATE SET
                    last_access_ts = excluded.last_access_ts,
                    hit_count = frecency.hit_count + 1",
            )?;
            for p in paths {
                stmt.execute(params![normalize_path(p), now_ts])?;
            }
        }
        tx.commit()
    }

    /// Decayed frecency score for a single path. Missing paths return `0.0`.
    pub fn score(&self, path: &Path, now_ts: i64) -> rusqlite::Result<f64> {
        let conn = self.conn.lock().expect("frecency mutex poisoned");
        let row: Option<(i64, i64)> = conn
            .query_row(
                "SELECT last_access_ts, hit_count FROM frecency WHERE path = ?1",
                params![normalize_path(path)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(row
            .map(|(ts, hits)| decay_score(hits, now_ts, ts))
            .unwrap_or(0.0))
    }

    /// Batch-score many paths at once. Missing paths are omitted from the map.
    pub fn bulk_scores(
        &self,
        paths: &[&Path],
        now_ts: i64,
    ) -> rusqlite::Result<HashMap<PathBuf, f64>> {
        let mut out = HashMap::with_capacity(paths.len());
        if paths.is_empty() {
            return Ok(out);
        }
        let conn = self.conn.lock().expect("frecency mutex poisoned");
        let mut stmt = conn.prepare_cached(
            "SELECT last_access_ts, hit_count FROM frecency WHERE path = ?1",
        )?;
        for p in paths {
            let key = normalize_path(p);
            let row: Option<(i64, i64)> = stmt
                .query_row(params![key], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?;
            if let Some((ts, hits)) = row {
                out.insert(PathBuf::from(&key), decay_score(hits, now_ts, ts));
            }
        }
        Ok(out)
    }

    /// Most-recently-bumped rows, newest first, capped at 10. For health output.
    pub fn last_10_bumps(&self) -> rusqlite::Result<Vec<BumpEntry>> {
        let conn = self.conn.lock().expect("frecency mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT path, last_access_ts, hit_count FROM frecency
             ORDER BY last_access_ts DESC LIMIT 10",
        )?;
        stmt.query_map([], |r| {
            Ok(BumpEntry {
                path: PathBuf::from(r.get::<_, String>(0)?),
                last_access_ts: r.get(1)?,
                hit_count: r.get(2)?,
            })
        })?
        .collect()
    }

    /// Top-N paths ordered by decayed score at `now_ts`. For health output.
    pub fn top_frecent(&self, n: usize, now_ts: i64) -> rusqlite::Result<Vec<(PathBuf, f64)>> {
        let conn = self.conn.lock().expect("frecency mutex poisoned");
        let mut stmt = conn.prepare("SELECT path, last_access_ts, hit_count FROM frecency")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    PathBuf::from(r.get::<_, String>(0)?),
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut scored: Vec<_> = rows
            .into_iter()
            .map(|(p, ts, hits)| (p, decay_score(hits, now_ts, ts)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(n);
        Ok(scored)
    }

    /// Apply the graduated HEAD-change reset policy and persist `current_head`.
    ///
    /// Policy (Implementation Notes §"Reset-on-HEAD-change: graduated, not binary"):
    /// - `last_head` is `None` (first session) → no-op.
    /// - `current_head == last_head` → no-op.
    /// - `commit_distance == None` (unrelated history / branch change) → zero.
    /// - `commit_distance < 50` → no-op.
    /// - `50 <= commit_distance <= 500` → halve all `hit_count`.
    /// - `commit_distance > 500` → zero all `hit_count`.
    ///
    /// The stored `last_head` is updated to `current_head` in every outcome so
    /// subsequent sessions compare against the most recent reset point.
    ///
    /// Note: the `commit_distance` parameter is `Option<u32>` (not the `u32`
    /// the todo text specified). The spec requires "branch change → zero",
    /// which the `git::commit_distance` helper already signals by returning
    /// `Ok(None)` for unrelated histories. Flowing that through preserves the
    /// distinction without an out-of-band sentinel value.
    pub fn reset_or_halve_on_head_change(
        &self,
        last_head: Option<&str>,
        current_head: &str,
        commit_distance: Option<u32>,
    ) -> rusqlite::Result<ResetOutcome> {
        let mut conn = self.conn.lock().expect("frecency mutex poisoned");
        let tx = conn.transaction()?;
        let outcome = match (last_head, commit_distance) {
            (None, _) => ResetOutcome::NoOp,
            (Some(last), _) if last == current_head => ResetOutcome::NoOp,
            (Some(_), None) => {
                tx.execute("UPDATE frecency SET hit_count = 0", [])?;
                ResetOutcome::Zeroed
            }
            (Some(_), Some(d)) if d < RESET_NOOP_THRESHOLD => ResetOutcome::NoOp,
            (Some(_), Some(d)) if d <= RESET_HALVE_THRESHOLD => {
                tx.execute("UPDATE frecency SET hit_count = hit_count / 2", [])?;
                ResetOutcome::Halved
            }
            (Some(_), Some(_)) => {
                tx.execute("UPDATE frecency SET hit_count = 0", [])?;
                ResetOutcome::Zeroed
            }
        };
        tx.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![META_LAST_HEAD, current_head],
        )?;
        tx.commit()?;
        Ok(outcome)
    }

    /// Read the last HEAD SHA this store recorded.
    pub fn last_head(&self) -> rusqlite::Result<Option<String>> {
        let conn = self.conn.lock().expect("frecency mutex poisoned");
        conn.query_row(
            "SELECT value FROM meta WHERE key = ?1",
            params![META_LAST_HEAD],
            |r| r.get::<_, String>(0),
        )
        .optional()
    }
}

/// Decay formula: `hit_count * exp(-ln(2) * (now - last) / HALF_LIFE_SECS)`.
/// Clamps a future `last_ts` (clock skew) to "no decay" rather than amplifying.
#[inline]
fn decay_score(hit_count: i64, now_ts: i64, last_ts: i64) -> f64 {
    let dt = (now_ts - last_ts).max(0) as f64;
    (hit_count as f64) * (-std::f64::consts::LN_2 * dt / HALF_LIFE_SECS as f64).exp()
}

/// Normalize paths to forward-slash form so Windows and Unix key the same row.
/// Mirrors the pattern in `src/git.rs::collect_diff_paths`.
fn normalize_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> FrecencyStore {
        FrecencyStore::open_in_memory().expect("open in-memory frecency store")
    }

    fn norm(p: &Path) -> PathBuf {
        PathBuf::from(normalize_path(p))
    }

    #[test]
    fn bump_inserts_new_path_with_hit_count_one() {
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        store.bump(&[p.clone()], 1_000).unwrap();
        assert_eq!(store.score(&p, 1_000).unwrap(), 1.0);
    }

    #[test]
    fn bump_increments_existing_path() {
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        store.bump(&[p.clone()], 1_000).unwrap();
        store.bump(&[p.clone()], 2_000).unwrap();
        store.bump(&[p.clone()], 3_000).unwrap();
        assert_eq!(store.score(&p, 3_000).unwrap(), 3.0);
    }

    #[test]
    fn bump_updates_last_access_ts() {
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        store.bump(&[p.clone()], 1_000).unwrap();
        store.bump(&[p.clone()], 5_000).unwrap();
        let entries = store.last_10_bumps().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].last_access_ts, 5_000);
        assert_eq!(entries[0].hit_count, 2);
    }

    #[test]
    fn bump_multiple_paths_in_single_call() {
        let store = make_store();
        let paths = vec![
            PathBuf::from("src/a.rs"),
            PathBuf::from("src/b.rs"),
            PathBuf::from("src/c.rs"),
        ];
        store.bump(&paths, 1_000).unwrap();
        for p in &paths {
            assert_eq!(store.score(p, 1_000).unwrap(), 1.0);
        }
    }

    #[test]
    fn empty_bump_is_noop() {
        let store = make_store();
        store.bump(&[], 0).unwrap();
        assert!(store.top_frecent(10, 0).unwrap().is_empty());
    }

    #[test]
    fn score_returns_zero_for_missing_path() {
        let store = make_store();
        assert_eq!(store.score(Path::new("nope.rs"), 1_000).unwrap(), 0.0);
    }

    #[test]
    fn score_decays_by_half_at_one_half_life() {
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        store.bump(&[p.clone()], 0).unwrap();
        let score = store.score(&p, HALF_LIFE_SECS).unwrap();
        assert!(
            (score - 0.5).abs() < 1e-9,
            "expected ~0.5 at 1 half-life, got {score}"
        );
    }

    #[test]
    fn score_decays_to_quarter_at_two_half_lives() {
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        store.bump(&[p.clone()], 0).unwrap();
        let score = store.score(&p, HALF_LIFE_SECS * 2).unwrap();
        assert!(
            (score - 0.25).abs() < 1e-9,
            "expected ~0.25 at 2 half-lives, got {score}"
        );
    }

    #[test]
    fn score_is_stable_when_now_equals_last_access() {
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        store.bump(&[p.clone()], 12_345).unwrap();
        assert_eq!(store.score(&p, 12_345).unwrap(), 1.0);
    }

    #[test]
    fn score_does_not_amplify_on_clock_skew() {
        // If now_ts < last_access_ts (clock skew), score should not exceed hit_count.
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        store.bump(&[p.clone()], 10_000).unwrap();
        assert_eq!(store.score(&p, 9_000).unwrap(), 1.0);
    }

    #[test]
    fn fusion_property_recent_outranks_ancient_with_many_hits() {
        // "File touched 5 min ago outranks file touched 6 months ago with 10× hits."
        let store = make_store();
        let ancient = PathBuf::from("src/ancient.rs");
        let recent = PathBuf::from("src/recent.rs");
        let six_months: i64 = 60 * 60 * 24 * 30 * 6;
        for _ in 0..10 {
            store.bump(&[ancient.clone()], 0).unwrap();
        }
        store.bump(&[recent.clone()], six_months - 300).unwrap();
        let now = six_months;
        assert!(store.score(&recent, now).unwrap() > store.score(&ancient, now).unwrap());
    }

    #[test]
    fn bulk_scores_matches_per_path_score() {
        let store = make_store();
        let a = PathBuf::from("src/a.rs");
        let b = PathBuf::from("src/b.rs");
        let missing = PathBuf::from("src/missing.rs");
        store.bump(&[a.clone()], 0).unwrap();
        store.bump(&[b.clone()], 0).unwrap();
        store.bump(&[b.clone()], 0).unwrap();
        let now = HALF_LIFE_SECS;
        let paths: Vec<&Path> = vec![a.as_path(), b.as_path(), missing.as_path()];
        let bulk = store.bulk_scores(&paths, now).unwrap();
        assert_eq!(bulk.len(), 2, "missing path must be omitted from bulk map");
        assert!((bulk[&norm(&a)] - store.score(&a, now).unwrap()).abs() < 1e-9);
        assert!((bulk[&norm(&b)] - store.score(&b, now).unwrap()).abs() < 1e-9);
    }

    #[test]
    fn bulk_scores_empty_input_is_empty_map() {
        let store = make_store();
        let paths: Vec<&Path> = vec![];
        assert!(store.bulk_scores(&paths, 0).unwrap().is_empty());
    }

    #[test]
    fn last_10_bumps_returns_most_recent_first() {
        let store = make_store();
        for i in 0..15 {
            store
                .bump(&[PathBuf::from(format!("src/f_{i}.rs"))], i as i64 * 1_000)
                .unwrap();
        }
        let entries = store.last_10_bumps().unwrap();
        assert_eq!(entries.len(), 10);
        assert_eq!(entries[0].last_access_ts, 14_000);
        assert_eq!(entries[9].last_access_ts, 5_000);
    }

    #[test]
    fn top_frecent_orders_by_decayed_score() {
        let store = make_store();
        let hot = PathBuf::from("hot.rs");
        let warm = PathBuf::from("warm.rs");
        let cold = PathBuf::from("cold.rs");
        // Same last_access_ts; hit counts differ.
        store.bump(&[hot.clone()], 100).unwrap();
        store.bump(&[hot.clone()], 100).unwrap();
        store.bump(&[warm.clone()], 100).unwrap();
        // Cold: 1 hit, but decayed by 5 half-lives.
        store.bump(&[cold.clone()], 100 - HALF_LIFE_SECS * 5).unwrap();
        let top = store.top_frecent(3, 100).unwrap();
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].0, hot);
        assert_eq!(top[1].0, warm);
        assert_eq!(top[2].0, cold);
    }

    #[test]
    fn top_frecent_respects_n_limit() {
        let store = make_store();
        for i in 0..20 {
            store
                .bump(&[PathBuf::from(format!("f{i}.rs"))], 0)
                .unwrap();
        }
        assert_eq!(store.top_frecent(5, 0).unwrap().len(), 5);
    }

    #[test]
    fn reset_first_session_noops_and_stores_head() {
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        store.bump(&[p.clone()], 0).unwrap();
        let outcome = store
            .reset_or_halve_on_head_change(None, "abc123", Some(1_000))
            .unwrap();
        assert_eq!(outcome, ResetOutcome::NoOp);
        assert_eq!(store.score(&p, 0).unwrap(), 1.0);
        assert_eq!(store.last_head().unwrap().as_deref(), Some("abc123"));
    }

    #[test]
    fn reset_same_head_noops() {
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        store.bump(&[p.clone()], 0).unwrap();
        let outcome = store
            .reset_or_halve_on_head_change(Some("sha"), "sha", Some(0))
            .unwrap();
        assert_eq!(outcome, ResetOutcome::NoOp);
        assert_eq!(store.score(&p, 0).unwrap(), 1.0);
    }

    #[test]
    fn reset_below_50_commits_noops() {
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        for _ in 0..4 {
            store.bump(&[p.clone()], 0).unwrap();
        }
        let outcome = store
            .reset_or_halve_on_head_change(Some("old"), "new", Some(49))
            .unwrap();
        assert_eq!(outcome, ResetOutcome::NoOp);
        assert_eq!(store.score(&p, 0).unwrap(), 4.0);
    }

    #[test]
    fn reset_at_50_halves_hits() {
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        for _ in 0..10 {
            store.bump(&[p.clone()], 0).unwrap();
        }
        let outcome = store
            .reset_or_halve_on_head_change(Some("old"), "new", Some(50))
            .unwrap();
        assert_eq!(outcome, ResetOutcome::Halved);
        assert_eq!(store.score(&p, 0).unwrap(), 5.0);
    }

    #[test]
    fn reset_at_500_halves_hits() {
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        for _ in 0..10 {
            store.bump(&[p.clone()], 0).unwrap();
        }
        let outcome = store
            .reset_or_halve_on_head_change(Some("old"), "new", Some(500))
            .unwrap();
        assert_eq!(outcome, ResetOutcome::Halved);
        assert_eq!(store.score(&p, 0).unwrap(), 5.0);
    }

    #[test]
    fn reset_above_500_zeros_hits() {
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        for _ in 0..10 {
            store.bump(&[p.clone()], 0).unwrap();
        }
        let outcome = store
            .reset_or_halve_on_head_change(Some("old"), "new", Some(501))
            .unwrap();
        assert_eq!(outcome, ResetOutcome::Zeroed);
        assert_eq!(store.score(&p, 0).unwrap(), 0.0);
    }

    #[test]
    fn reset_unrelated_history_zeros_hits() {
        let store = make_store();
        let p = PathBuf::from("src/foo.rs");
        for _ in 0..10 {
            store.bump(&[p.clone()], 0).unwrap();
        }
        let outcome = store
            .reset_or_halve_on_head_change(Some("old"), "new", None)
            .unwrap();
        assert_eq!(outcome, ResetOutcome::Zeroed);
        assert_eq!(store.score(&p, 0).unwrap(), 0.0);
    }

    #[test]
    fn reset_updates_stored_head_across_outcomes() {
        let store = make_store();
        store.bump(&[PathBuf::from("src/foo.rs")], 0).unwrap();
        store
            .reset_or_halve_on_head_change(Some("a"), "b", Some(10))
            .unwrap();
        assert_eq!(store.last_head().unwrap().as_deref(), Some("b"));
        store
            .reset_or_halve_on_head_change(Some("b"), "c", Some(200))
            .unwrap();
        assert_eq!(store.last_head().unwrap().as_deref(), Some("c"));
        store
            .reset_or_halve_on_head_change(Some("c"), "d", Some(10_000))
            .unwrap();
        assert_eq!(store.last_head().unwrap().as_deref(), Some("d"));
    }

    #[test]
    fn path_normalization_treats_backslash_and_forward_slash_as_same_row() {
        let store = make_store();
        let windows = PathBuf::from("src\\foo.rs");
        let unix = PathBuf::from("src/foo.rs");
        store.bump(&[windows], 0).unwrap();
        store.bump(&[unix.clone()], 1_000).unwrap();
        assert_eq!(store.score(&unix, 1_000).unwrap(), 2.0);
        assert_eq!(store.last_10_bumps().unwrap().len(), 1);
    }

    #[test]
    fn open_file_backed_creates_db_and_parent_dir_and_persists() {
        let tmp = tempfile::TempDir::new().unwrap();
        let nested = tmp.path().join("nested").join("frecency.db");
        {
            let store = FrecencyStore::open(&nested).unwrap();
            store.bump(&[PathBuf::from("src/foo.rs")], 0).unwrap();
        }
        assert!(nested.exists(), "db file should be created");
        let store2 = FrecencyStore::open(&nested).unwrap();
        assert_eq!(store2.score(Path::new("src/foo.rs"), 0).unwrap(), 1.0);
    }
}
