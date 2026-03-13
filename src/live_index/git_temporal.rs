//! Git temporal intelligence — enriches the index with git history metadata.
//!
//! Computes per-file churn scores (exponential-decay weighted), ownership
//! distribution, co-change coupling (Jaccard coefficient), and repo-wide
//! hotspot summaries using libgit2 via [`crate::git::GitRepo`].
//!
//! Design principles:
//! - In-process git access: uses libgit2 (via git2 crate) — no child
//!   processes, no console windows, faster execution.
//! - Bounded: max 500 commits OR 90 days, whichever is smaller.
//! - Exponential decay: half-life of 14 days so recent activity dominates.
//! - Rank-normalized churn: percentile position across all tracked files
//!   (0.0 = coldest, 1.0 = hottest in repo) — meaningful regardless of
//!   absolute activity level.
//! - Jaccard co-change: `|A∩B| / |A∪B|` filters out high-frequency noise
//!   files (lock files, CI configs) that appear in many unrelated commits.
//! - Mega-commit filter: commits touching >50 files are excluded from
//!   co-change analysis to avoid pollution from bulk reformats/merges.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use super::store::SharedIndex;

// ── Background computation ──────────────────────────────────────────────

/// Spawn a background task that computes the git temporal index and swaps
/// it into the shared handle. Non-blocking — returns immediately.
///
/// Call after `LiveIndex::load()` or `SharedIndexHandle::reload()` completes.
pub fn spawn_git_temporal_computation(index: SharedIndex, repo_root: PathBuf) {
    // Guard: only spawn if a tokio runtime is available (not the case in some sync tests).
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }

    // If data is already Ready, keep serving it while we recompute in the background.
    let was_ready = index.git_temporal().state == GitTemporalState::Ready;

    if !was_ready {
        index.update_git_temporal(GitTemporalIndex {
            state: GitTemporalState::Computing,
            ..GitTemporalIndex::pending()
        });
    }

    tokio::spawn(async move {
        // Run the computation on a blocking thread (it uses libgit2 which does I/O).
        let result =
            tokio::task::spawn_blocking(move || GitTemporalIndex::compute(&repo_root)).await;

        match result {
            Ok(temporal) => {
                tracing::info!(
                    files = temporal.files.len(),
                    commits = temporal.stats.total_commits_analyzed,
                    duration_ms = temporal.stats.compute_duration.as_millis() as u64,
                    "git temporal index computed"
                );
                index.update_git_temporal(temporal);
            }
            Err(error) => {
                tracing::warn!("git temporal computation panicked: {error}");
                if !was_ready {
                    index.update_git_temporal(GitTemporalIndex::unavailable(format!(
                        "computation panicked: {error}"
                    )));
                }
            }
        }
    });
}

// ── Configuration constants ─────────────────────────────────────────────

const MAX_COMMITS: u32 = 500;
const WINDOW_DAYS: u32 = 90;
/// Exponential decay half-life in days. A commit 14 days ago has half the
/// weight of one today; 28 days ago has a quarter, etc.
const HALF_LIFE_DAYS: f64 = 14.0;
/// Maximum co-changed files shown per file.
const CO_CHANGE_CAP_PER_FILE: usize = 5;
/// Top hotspots in the repo-wide stats.
const HOTSPOT_CAP: usize = 10;
/// Top coupled pairs in the repo-wide stats.
const COUPLED_PAIRS_CAP: usize = 10;
/// Minimum shared commits before a co-change pair is considered.
const MIN_SHARED_COMMITS: u32 = 2;
/// Minimum Jaccard coefficient to keep a co-change entry.
const MIN_JACCARD: f32 = 0.15;
/// Maximum contributors shown per file.
const CONTRIBUTOR_CAP: usize = 5;
/// Commits touching more files than this are excluded from co-change
/// analysis (likely merges, formatting runs, bulk renames).
const MEGA_COMMIT_THRESHOLD: usize = 50;

// ── Public data types ───────────────────────────────────────────────────

/// Per-file temporal metadata derived from git history.
#[derive(Debug, Clone)]
pub struct GitFileHistory {
    /// Total commits touching this file within the analysis window.
    pub commit_count: u32,
    /// Recency-weighted churn score, rank-normalized to 0.0–1.0 across the
    /// entire repo. Uses exponential decay with a 14-day half-life so
    /// recent commits dominate. Rank-normalized means the hottest file in
    /// the repo is always ~1.0, the coldest ~0.0.
    pub churn_score: f32,
    /// Most recent commit touching this file.
    pub last_commit: CommitSummary,
    /// Ownership distribution — who actually maintains this file, sorted by
    /// commit share descending. Capped at top 5 contributors.
    pub contributors: Vec<ContributorShare>,
    /// Files that co-change with this one, ranked by Jaccard coupling
    /// strength. Capped at top 5.
    pub co_changes: Vec<CoChangeEntry>,
}

/// Summary of a single git commit (cheap to clone, display-ready).
#[derive(Debug, Clone)]
pub struct CommitSummary {
    /// Short hash (7 chars).
    pub hash: String,
    /// ISO 8601 author date for display.
    pub timestamp: String,
    /// Author name.
    pub author: String,
    /// First line of commit message, truncated to 72 chars.
    pub message_head: String,
    /// Days ago from the time of computation (for relative time display).
    pub days_ago: f64,
}

/// One contributor's share of a file's commit history.
#[derive(Debug, Clone)]
pub struct ContributorShare {
    pub author: String,
    pub commit_count: u32,
    /// Percentage of total commits to this file (0.0–100.0).
    pub percentage: f32,
}

/// One co-change relationship for a file.
#[derive(Debug, Clone)]
pub struct CoChangeEntry {
    /// Path of the co-changed file.
    pub path: String,
    /// Jaccard coefficient: `|shared_commits| / |union_commits|`, 0.0–1.0.
    pub coupling_score: f32,
    /// Raw number of commits where both files changed together.
    pub shared_commits: u32,
}

/// Repo-wide temporal summary for health reports.
#[derive(Debug, Clone)]
pub struct GitTemporalStats {
    /// Total commits analyzed in this computation.
    pub total_commits_analyzed: u32,
    /// Analysis window in days (from config, currently 90).
    pub analysis_window_days: u32,
    /// Top hotspot files by churn score.
    pub hotspots: Vec<(String, f32)>,
    /// Top coupled file pairs by Jaccard coefficient.
    pub most_coupled: Vec<(String, String, f32)>,
    /// Wall-clock time when computation completed.
    pub computed_at: SystemTime,
    /// Time spent computing the temporal index.
    pub compute_duration: Duration,
}

/// The full temporal index — a side-table that lives parallel to the
/// main `LiveIndex` on `SharedIndexHandle`.
#[derive(Debug, Clone)]
pub struct GitTemporalIndex {
    /// Per-file temporal metadata, keyed by relative path (forward-slash
    /// normalized, same key space as `LiveIndex::files`).
    pub files: HashMap<String, GitFileHistory>,
    /// Repo-wide summary statistics.
    pub stats: GitTemporalStats,
    /// Current state of the temporal index.
    pub state: GitTemporalState,
}

/// Lifecycle state of the temporal index.
#[derive(Debug, Clone, PartialEq)]
pub enum GitTemporalState {
    /// Not yet computed (initial state).
    Pending,
    /// Background computation is in progress.
    Computing,
    /// Computation completed — data is available.
    Ready,
    /// Git is unavailable or the directory is not a git repo.
    Unavailable(String),
}

// ── Intermediate parsing types (private) ────────────────────────────────

#[derive(Debug)]
struct ParsedCommit {
    hash: String,
    timestamp: String,
    author: String,
    message: String,
    /// Days before computation time (0.0 = today).
    days_ago: f64,
    /// Relative file paths touched by this commit.
    files: Vec<String>,
}

// ── Rendering helpers (public) ──────────────────────────────────────────

/// Render a 10-character visual churn bar: `██████░░░░`
pub fn churn_bar(score: f32) -> String {
    let clamped = score.clamp(0.0, 1.0);
    let filled = (clamped * 10.0).round() as usize;
    let empty = 10_usize.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// Human-readable churn label for a normalized score.
pub fn churn_label(score: f32) -> &'static str {
    if score >= 0.8 {
        "critical"
    } else if score >= 0.6 {
        "hot"
    } else if score >= 0.4 {
        "warm"
    } else if score >= 0.2 {
        "cool"
    } else {
        "frozen"
    }
}

/// Format days-ago as a compact relative time string: "3d ago", "2w ago", etc.
pub fn relative_time(days_ago: f64) -> String {
    if days_ago < 0.0 {
        return "just now".to_string();
    }
    if days_ago < 1.0 {
        return "today".to_string();
    }
    if days_ago < 7.0 {
        return format!("{}d ago", days_ago.round() as u32);
    }
    if days_ago < 30.0 {
        return format!("{}w ago", (days_ago / 7.0).round() as u32);
    }
    format!("{}mo ago", (days_ago / 30.0).round() as u32)
}

// ── Core implementation ─────────────────────────────────────────────────

impl GitTemporalIndex {
    /// Construct a pending (empty) temporal index.
    pub fn pending() -> Self {
        Self {
            files: HashMap::new(),
            stats: GitTemporalStats {
                total_commits_analyzed: 0,
                analysis_window_days: WINDOW_DAYS,
                hotspots: Vec::new(),
                most_coupled: Vec::new(),
                computed_at: SystemTime::now(),
                compute_duration: Duration::ZERO,
            },
            state: GitTemporalState::Pending,
        }
    }

    /// Construct an unavailable temporal index with a reason.
    pub fn unavailable(reason: String) -> Self {
        Self {
            state: GitTemporalState::Unavailable(reason),
            ..Self::pending()
        }
    }

    /// Compute the full temporal index from git history.
    ///
    /// Uses libgit2 via [`crate::git::GitRepo`] to walk the commit log
    /// and build per-file metrics, co-change relationships, and repo-wide
    /// stats. Designed to run on a blocking thread.
    pub fn compute(repo_root: &Path) -> Self {
        let start = Instant::now();

        let commits = match load_commits(repo_root) {
            Ok(c) => c,
            Err(reason) => return Self::unavailable(reason),
        };
        if commits.is_empty() {
            return Self {
                files: HashMap::new(),
                stats: GitTemporalStats {
                    total_commits_analyzed: 0,
                    analysis_window_days: WINDOW_DAYS,
                    hotspots: Vec::new(),
                    most_coupled: Vec::new(),
                    computed_at: SystemTime::now(),
                    compute_duration: start.elapsed(),
                },
                state: GitTemporalState::Ready,
            };
        }

        let total_commits = commits.len() as u32;
        let decay_lambda = (2.0_f64).ln() / HALF_LIFE_DAYS;

        // ── Phase 1: Per-file aggregation ───────────────────────────────

        // file -> list of commit indices (for co-change Jaccard denominators)
        let mut file_commit_indices: HashMap<String, Vec<usize>> = HashMap::new();
        // file -> author -> commit count
        let mut file_authors: HashMap<String, HashMap<String, u32>> = HashMap::new();
        // file -> index of most recent commit (smallest days_ago)
        let mut file_last_commit_idx: HashMap<String, usize> = HashMap::new();
        // file -> sum of decay-weighted commit scores
        let mut file_raw_churn: HashMap<String, f64> = HashMap::new();

        for (idx, commit) in commits.iter().enumerate() {
            let weight = (-decay_lambda * commit.days_ago).exp();

            for file_path in &commit.files {
                file_commit_indices
                    .entry(file_path.clone())
                    .or_default()
                    .push(idx);

                *file_authors
                    .entry(file_path.clone())
                    .or_default()
                    .entry(commit.author.clone())
                    .or_insert(0) += 1;

                file_last_commit_idx
                    .entry(file_path.clone())
                    .and_modify(|existing| {
                        if commit.days_ago < commits[*existing].days_ago {
                            *existing = idx;
                        }
                    })
                    .or_insert(idx);

                *file_raw_churn.entry(file_path.clone()).or_insert(0.0) += weight;
            }
        }

        // ── Phase 2: Rank-normalize churn scores ────────────────────────

        let mut churn_entries: Vec<(String, f64)> = file_raw_churn.into_iter().collect();
        churn_entries.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let file_count = churn_entries.len();
        let mut normalized_churn: HashMap<String, f32> = HashMap::with_capacity(file_count);
        for (rank, (path, _raw)) in churn_entries.iter().enumerate() {
            let score = if file_count <= 1 {
                if churn_entries[0].1 > 0.0 { 1.0 } else { 0.0 }
            } else {
                rank as f32 / (file_count - 1) as f32
            };
            normalized_churn.insert(path.clone(), score);
        }

        // ── Phase 3: Co-change matrix (Jaccard) ────────────────────────

        // Count how many commits each pair of files shares.
        let mut pair_counts: HashMap<(String, String), u32> = HashMap::new();

        for commit in &commits {
            let mut sorted_files: Vec<&str> = commit.files.iter().map(|s| s.as_str()).collect();
            sorted_files.sort_unstable();
            sorted_files.dedup();

            // Skip mega-commits (merges, bulk reformats, etc.)
            if sorted_files.len() > MEGA_COMMIT_THRESHOLD {
                continue;
            }

            for i in 0..sorted_files.len() {
                for j in (i + 1)..sorted_files.len() {
                    // Canonical ordering: alphabetically smaller path first.
                    let key = (sorted_files[i].to_string(), sorted_files[j].to_string());
                    *pair_counts.entry(key).or_insert(0) += 1;
                }
            }
        }

        // Compute Jaccard for each qualifying pair.
        let mut file_co_changes: HashMap<String, Vec<CoChangeEntry>> = HashMap::new();

        for ((file_a, file_b), shared) in &pair_counts {
            if *shared < MIN_SHARED_COMMITS {
                continue;
            }
            let count_a = file_commit_indices
                .get(file_a)
                .map(|v| v.len() as u32)
                .unwrap_or(0);
            let count_b = file_commit_indices
                .get(file_b)
                .map(|v| v.len() as u32)
                .unwrap_or(0);
            let union = count_a + count_b - shared;
            if union == 0 {
                continue;
            }
            let jaccard = *shared as f32 / union as f32;
            if jaccard < MIN_JACCARD {
                continue;
            }

            // Bidirectional: A sees B, B sees A.
            file_co_changes
                .entry(file_a.clone())
                .or_default()
                .push(CoChangeEntry {
                    path: file_b.clone(),
                    coupling_score: jaccard,
                    shared_commits: *shared,
                });
            file_co_changes
                .entry(file_b.clone())
                .or_default()
                .push(CoChangeEntry {
                    path: file_a.clone(),
                    coupling_score: jaccard,
                    shared_commits: *shared,
                });
        }

        // Sort by coupling strength descending and cap per file.
        for entries in file_co_changes.values_mut() {
            entries.sort_by(|a, b| {
                b.coupling_score
                    .partial_cmp(&a.coupling_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            entries.truncate(CO_CHANGE_CAP_PER_FILE);
        }

        // ── Phase 4: Assemble GitFileHistory per file ───────────────────

        let mut files: HashMap<String, GitFileHistory> = HashMap::with_capacity(file_count);

        for (path, _commit_indices) in &file_commit_indices {
            let commit_count = _commit_indices.len() as u32;
            let churn_score = normalized_churn.get(path).copied().unwrap_or(0.0);

            // Last commit
            let last_idx = file_last_commit_idx.get(path).copied().unwrap_or(0);
            let last = &commits[last_idx];
            let last_commit = CommitSummary {
                hash: last.hash.clone(),
                timestamp: last.timestamp.clone(),
                author: last.author.clone(),
                message_head: truncate_message(&last.message, 72),
                days_ago: last.days_ago,
            };

            // Contributors
            let contributors = file_authors
                .get(path)
                .map(|authors| {
                    let total = authors.values().sum::<u32>() as f32;
                    let mut shares: Vec<ContributorShare> = authors
                        .iter()
                        .map(|(author, count)| ContributorShare {
                            author: author.clone(),
                            commit_count: *count,
                            percentage: (*count as f32 / total) * 100.0,
                        })
                        .collect();
                    shares.sort_by(|a, b| {
                        b.percentage
                            .partial_cmp(&a.percentage)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    shares.truncate(CONTRIBUTOR_CAP);
                    shares
                })
                .unwrap_or_default();

            let co_changes = file_co_changes.remove(path).unwrap_or_default();

            files.insert(
                path.clone(),
                GitFileHistory {
                    commit_count,
                    churn_score,
                    last_commit,
                    contributors,
                    co_changes,
                },
            );
        }

        // ── Phase 5: Repo-wide stats ────────────────────────────────────

        let mut hotspots: Vec<(String, f32)> = files
            .iter()
            .map(|(path, h)| (path.clone(), h.churn_score))
            .collect();
        hotspots.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        hotspots.truncate(HOTSPOT_CAP);

        let mut most_coupled: Vec<(String, String, f32)> = pair_counts
            .iter()
            .filter_map(|((a, b), shared)| {
                if *shared < MIN_SHARED_COMMITS {
                    return None;
                }
                let count_a = file_commit_indices
                    .get(a)
                    .map(|v| v.len() as u32)
                    .unwrap_or(0);
                let count_b = file_commit_indices
                    .get(b)
                    .map(|v| v.len() as u32)
                    .unwrap_or(0);
                let union = count_a + count_b - shared;
                if union == 0 {
                    return None;
                }
                let jaccard = *shared as f32 / union as f32;
                if jaccard < MIN_JACCARD {
                    return None;
                }
                Some((a.clone(), b.clone(), jaccard))
            })
            .collect();
        most_coupled.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        most_coupled.truncate(COUPLED_PAIRS_CAP);

        Self {
            files,
            stats: GitTemporalStats {
                total_commits_analyzed: total_commits,
                analysis_window_days: WINDOW_DAYS,
                hotspots,
                most_coupled,
                computed_at: SystemTime::now(),
                compute_duration: start.elapsed(),
            },
            state: GitTemporalState::Ready,
        }
    }
}

// ── Git log via libgit2 ─────────────────────────────────────────────────

/// Load commits from git history using libgit2 (no child processes).
fn load_commits(repo_root: &Path) -> Result<Vec<ParsedCommit>, String> {
    use crate::git::GitRepo;

    let repo = GitRepo::open(repo_root)?;
    let entries = repo.log_with_stats(MAX_COMMITS as usize, WINDOW_DAYS)?;

    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as f64;

    Ok(entries
        .into_iter()
        .map(|e| {
            let days_ago = (now - e.unix_timestamp as f64) / 86400.0;
            ParsedCommit {
                hash: e.hash,
                timestamp: e.timestamp,
                author: e.author,
                message: e.message,
                days_ago: days_ago.max(0.0),
                files: e.files,
            }
        })
        .collect())
}

/// Truncate a message to `max_len` characters, appending "..." if truncated.
fn truncate_message(msg: &str, max_len: usize) -> String {
    if msg.len() <= max_len {
        msg.to_string()
    } else {
        let truncated: String = msg.chars().take(max_len.saturating_sub(3)).collect();
        format!("{truncated}...")
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

// ── Test-only parsing infrastructure ────────────────────────────────────
//
// These were the original CLI-based parsing functions. They are retained
// solely to support the existing unit tests that feed synthetic `git log`
// strings into `compute_from_log`. Compiled only in test builds.

#[cfg(test)]
const COMMIT_DELIMITER: &str = "TOKENIZOR_GIT_TEMPORAL_DELIM";

#[cfg(test)]
fn parse_git_log(raw: &str, now_unix: u64) -> Vec<ParsedCommit> {
    let mut commits: Vec<ParsedCommit> = Vec::new();
    let mut current: Option<ParsedCommitBuilder> = None;

    for line in raw.lines() {
        let line = line.trim_end();

        if line == COMMIT_DELIMITER {
            if let Some(builder) = current.take() {
                if let Some(commit) = builder.build() {
                    commits.push(commit);
                }
            }
            current = Some(ParsedCommitBuilder::new());
            continue;
        }

        let Some(builder) = current.as_mut() else {
            continue;
        };

        if let Some(rest) = line.strip_prefix("H:") {
            builder.hash = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("U:") {
            if let Ok(unix_ts) = rest.parse::<u64>() {
                builder.unix_timestamp = Some(unix_ts);
                builder.days_ago = (now_unix as f64 - unix_ts as f64) / 86400.0;
            }
        } else if let Some(rest) = line.strip_prefix("D:") {
            builder.timestamp = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("A:") {
            builder.author = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("M:") {
            builder.message = rest.to_string();
        } else if !line.is_empty() {
            if let Some(path) = parse_numstat_line(line) {
                builder.files.push(normalize_git_path(&path));
            }
        }
    }

    if let Some(builder) = current.take() {
        if let Some(commit) = builder.build() {
            commits.push(commit);
        }
    }

    commits
}

#[cfg(test)]
fn parse_numstat_line(line: &str) -> Option<String> {
    let mut parts = line.splitn(3, '\t');
    let added = parts.next()?;
    let _removed = parts.next()?;
    let path = parts.next()?;

    if added == "-" {
        return None;
    }
    if !added.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if path.is_empty() {
        return None;
    }

    Some(path.to_string())
}

#[cfg(test)]
fn normalize_git_path(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
#[derive(Debug, Default)]
struct ParsedCommitBuilder {
    hash: String,
    timestamp: String,
    author: String,
    message: String,
    unix_timestamp: Option<u64>,
    days_ago: f64,
    files: Vec<String>,
}

#[cfg(test)]
impl ParsedCommitBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn build(self) -> Option<ParsedCommit> {
        if self.hash.is_empty() || self.files.is_empty() {
            return None;
        }
        Some(ParsedCommit {
            hash: self.hash,
            timestamp: self.timestamp,
            author: self.author,
            message: self.message,
            days_ago: self.days_ago,
            files: self.files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Rendering helpers ───────────────────────────────────────────────

    #[test]
    fn test_churn_bar_zero() {
        assert_eq!(churn_bar(0.0), "░░░░░░░░░░");
    }

    #[test]
    fn test_churn_bar_half() {
        assert_eq!(churn_bar(0.5), "█████░░░░░");
    }

    #[test]
    fn test_churn_bar_full() {
        assert_eq!(churn_bar(1.0), "██████████");
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
    fn test_churn_label_frozen() {
        assert_eq!(churn_label(0.1), "frozen");
    }

    #[test]
    fn test_churn_label_cool() {
        assert_eq!(churn_label(0.3), "cool");
    }

    #[test]
    fn test_churn_label_warm() {
        assert_eq!(churn_label(0.5), "warm");
    }

    #[test]
    fn test_churn_label_hot() {
        assert_eq!(churn_label(0.7), "hot");
    }

    #[test]
    fn test_churn_label_critical() {
        assert_eq!(churn_label(0.9), "critical");
    }

    #[test]
    fn test_relative_time_today() {
        assert_eq!(relative_time(0.3), "today");
    }

    #[test]
    fn test_relative_time_days() {
        assert_eq!(relative_time(3.0), "3d ago");
    }

    #[test]
    fn test_relative_time_weeks() {
        assert_eq!(relative_time(14.0), "2w ago");
    }

    #[test]
    fn test_relative_time_months() {
        assert_eq!(relative_time(60.0), "2mo ago");
    }

    #[test]
    fn test_relative_time_negative() {
        assert_eq!(relative_time(-1.0), "just now");
    }

    // ── Truncation ──────────────────────────────────────────────────────

    #[test]
    fn test_truncate_message_short() {
        assert_eq!(truncate_message("hello", 72), "hello");
    }

    #[test]
    fn test_truncate_message_exact_boundary() {
        let msg = "a".repeat(72);
        assert_eq!(truncate_message(&msg, 72), msg);
    }

    #[test]
    fn test_truncate_message_long() {
        let msg = "a".repeat(100);
        let result = truncate_message(&msg, 72);
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), 72);
    }

    // ── Numstat parsing ─────────────────────────────────────────────────

    #[test]
    fn test_parse_numstat_line_normal() {
        assert_eq!(
            parse_numstat_line("3\t1\tsrc/foo.rs"),
            Some("src/foo.rs".to_string())
        );
    }

    #[test]
    fn test_parse_numstat_line_binary() {
        assert_eq!(parse_numstat_line("-\t-\timage.png"), None);
    }

    #[test]
    fn test_parse_numstat_line_empty_path() {
        assert_eq!(parse_numstat_line("3\t1\t"), None);
    }

    #[test]
    fn test_parse_numstat_line_not_numstat() {
        assert_eq!(parse_numstat_line("some random line"), None);
    }

    #[test]
    fn test_parse_numstat_line_zero_changes() {
        assert_eq!(
            parse_numstat_line("0\t0\tsrc/lib.rs"),
            Some("src/lib.rs".to_string())
        );
    }

    // ── Git log parsing ─────────────────────────────────────────────────

    fn sample_git_log() -> String {
        // Simulate two commits: one 2 days ago, one 10 days ago.
        let now_unix = 1_741_520_000_u64;
        let ts_2d = now_unix - (2 * 86400);
        let ts_10d = now_unix - (10 * 86400);
        format!(
            "{delim}\n\
             H:abc1234\n\
             U:{ts_2d}\n\
             D:2026-03-07T10:00:00+00:00\n\
             A:Alice\n\
             M:fix: resolve parsing bug\n\
             \n\
             3\t1\tsrc/protocol/tools.rs\n\
             5\t2\tsrc/protocol/format.rs\n\
             {delim}\n\
             H:def5678\n\
             U:{ts_10d}\n\
             D:2026-02-27T10:00:00+00:00\n\
             A:Bob\n\
             M:feat: add search feature\n\
             \n\
             10\t0\tsrc/protocol/tools.rs\n\
             20\t5\tsrc/live_index/query.rs\n",
            delim = COMMIT_DELIMITER,
        )
    }

    #[test]
    fn test_parse_git_log_extracts_commits() {
        let log = sample_git_log();
        let now_unix = 1_741_520_000_u64;
        let commits = parse_git_log(&log, now_unix);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].hash, "abc1234");
        assert_eq!(commits[0].author, "Alice");
        assert_eq!(commits[0].files.len(), 2);
        assert_eq!(commits[1].hash, "def5678");
        assert_eq!(commits[1].author, "Bob");
        assert_eq!(commits[1].files.len(), 2);
    }

    #[test]
    fn test_parse_git_log_computes_days_ago() {
        let log = sample_git_log();
        let now_unix = 1_741_520_000_u64;
        let commits = parse_git_log(&log, now_unix);
        assert!((commits[0].days_ago - 2.0).abs() < 0.01);
        assert!((commits[1].days_ago - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_git_log_empty_input() {
        let commits = parse_git_log("", 1_741_520_000);
        assert!(commits.is_empty());
    }

    #[test]
    fn test_parse_git_log_skips_commit_with_no_files() {
        let log = format!(
            "{delim}\nH:aaa1111\nU:1741520000\nD:2026-03-09\nA:Eve\nM:empty commit\n",
            delim = COMMIT_DELIMITER,
        );
        let commits = parse_git_log(&log, 1_741_520_000);
        assert!(commits.is_empty());
    }

    // ── Churn score computation ─────────────────────────────────────────

    #[test]
    fn test_churn_score_recent_beats_old() {
        // Build a minimal log: file_a changed today, file_b changed 60 days ago.
        let now_unix = 1_741_520_000_u64;
        let ts_today = now_unix;
        let ts_old = now_unix - (60 * 86400);
        let log = format!(
            "{delim}\nH:aaa\nU:{ts_today}\nD:2026-03-09\nA:X\nM:today\n\n1\t0\tfile_a.rs\n\
             {delim}\nH:bbb\nU:{ts_old}\nD:2026-01-08\nA:Y\nM:old\n\n1\t0\tfile_b.rs\n",
            delim = COMMIT_DELIMITER,
        );
        let index = GitTemporalIndex::compute_from_log(&log, now_unix);
        let a = index.files.get("file_a.rs").unwrap();
        let b = index.files.get("file_b.rs").unwrap();
        assert!(
            a.churn_score > b.churn_score,
            "recent file ({}) should have higher churn than old file ({})",
            a.churn_score,
            b.churn_score
        );
    }

    #[test]
    fn test_churn_score_multiple_commits_beats_single() {
        let now_unix = 1_741_520_000_u64;
        let ts1 = now_unix - (1 * 86400);
        let ts2 = now_unix - (3 * 86400);
        let ts3 = now_unix - (5 * 86400);
        let log = format!(
            "{delim}\nH:a1\nU:{ts1}\nD:d1\nA:X\nM:m1\n\n1\t0\thot.rs\n\
             {delim}\nH:a2\nU:{ts2}\nD:d2\nA:X\nM:m2\n\n1\t0\thot.rs\n\
             {delim}\nH:a3\nU:{ts3}\nD:d3\nA:X\nM:m3\n\n1\t0\thot.rs\n1\t0\tcold.rs\n",
            delim = COMMIT_DELIMITER,
        );
        let index = GitTemporalIndex::compute_from_log(&log, now_unix);
        let hot = index.files.get("hot.rs").unwrap();
        let cold = index.files.get("cold.rs").unwrap();
        assert!(hot.churn_score > cold.churn_score);
    }

    // ── Co-change / Jaccard ─────────────────────────────────────────────

    #[test]
    fn test_co_change_jaccard_basic() {
        // Two commits: both touch A+B. One extra commit touches only A.
        // shared=2, union = 3+2-2 = 3, jaccard = 2/3 ≈ 0.667
        let now_unix = 1_741_520_000_u64;
        let ts1 = now_unix - 86400;
        let ts2 = now_unix - 2 * 86400;
        let ts3 = now_unix - 3 * 86400;
        let log = format!(
            "{d}\nH:c1\nU:{ts1}\nD:d\nA:X\nM:m\n\n1\t0\ta.rs\n1\t0\tb.rs\n\
             {d}\nH:c2\nU:{ts2}\nD:d\nA:X\nM:m\n\n1\t0\ta.rs\n1\t0\tb.rs\n\
             {d}\nH:c3\nU:{ts3}\nD:d\nA:X\nM:m\n\n1\t0\ta.rs\n",
            d = COMMIT_DELIMITER,
        );
        let index = GitTemporalIndex::compute_from_log(&log, now_unix);
        let a = index.files.get("a.rs").unwrap();
        assert_eq!(a.co_changes.len(), 1);
        assert_eq!(a.co_changes[0].path, "b.rs");
        assert_eq!(a.co_changes[0].shared_commits, 2);
        // Jaccard: 2 / (3 + 2 - 2) = 2/3
        assert!((a.co_changes[0].coupling_score - 0.6667).abs() < 0.01);
    }

    #[test]
    fn test_co_change_mega_commit_excluded() {
        // One commit touches 60 files — should be excluded from co-change.
        let now_unix = 1_741_520_000_u64;
        let ts = now_unix - 86400;
        let mut files_section = String::new();
        for i in 0..60 {
            files_section.push_str(&format!("1\t0\tfile_{i}.rs\n"));
        }
        // Add a second commit that only touches file_0 and file_1 to have data.
        let ts2 = now_unix - 2 * 86400;
        let log = format!(
            "{d}\nH:mega\nU:{ts}\nD:d\nA:X\nM:mega commit\n\n{files}\
             {d}\nH:small\nU:{ts2}\nD:d\nA:X\nM:small\n\n1\t0\tfile_0.rs\n1\t0\tfile_1.rs\n",
            d = COMMIT_DELIMITER,
            files = files_section,
        );
        let index = GitTemporalIndex::compute_from_log(&log, now_unix);
        // file_0 should only see file_1 as co-change from the small commit,
        // not all 59 other files from the mega commit.
        let f0 = index.files.get("file_0.rs").unwrap();
        // Only 1 shared commit (the small one), which is below MIN_SHARED_COMMITS (2).
        // So no co-changes should appear.
        assert!(f0.co_changes.is_empty());
    }

    #[test]
    fn test_co_change_below_min_shared_excluded() {
        // Two files share only 1 commit — below MIN_SHARED_COMMITS threshold.
        let now_unix = 1_741_520_000_u64;
        let ts = now_unix - 86400;
        let log = format!(
            "{d}\nH:c1\nU:{ts}\nD:d\nA:X\nM:m\n\n1\t0\ta.rs\n1\t0\tb.rs\n",
            d = COMMIT_DELIMITER,
        );
        let index = GitTemporalIndex::compute_from_log(&log, now_unix);
        let a = index.files.get("a.rs").unwrap();
        assert!(a.co_changes.is_empty());
    }

    // ── Contributors ────────────────────────────────────────────────────

    #[test]
    fn test_contributors_sorted_by_percentage() {
        let now_unix = 1_741_520_000_u64;
        let mut log = String::new();
        // Alice: 3 commits, Bob: 1 commit to same file.
        for (i, author) in ["Alice", "Alice", "Alice", "Bob"].iter().enumerate() {
            let ts = now_unix - (i as u64 + 1) * 86400;
            log.push_str(&format!(
                "{d}\nH:c{i}\nU:{ts}\nD:d\nA:{author}\nM:m{i}\n\n1\t0\tshared.rs\n",
                d = COMMIT_DELIMITER,
            ));
        }
        let index = GitTemporalIndex::compute_from_log(&log, now_unix);
        let h = index.files.get("shared.rs").unwrap();
        assert_eq!(h.contributors.len(), 2);
        assert_eq!(h.contributors[0].author, "Alice");
        assert_eq!(h.contributors[0].commit_count, 3);
        assert!((h.contributors[0].percentage - 75.0).abs() < 0.1);
        assert_eq!(h.contributors[1].author, "Bob");
    }

    // ── Last commit ─────────────────────────────────────────────────────

    #[test]
    fn test_last_commit_is_most_recent() {
        let now_unix = 1_741_520_000_u64;
        let ts_recent = now_unix - 86400;
        let ts_old = now_unix - 30 * 86400;
        let log = format!(
            "{d}\nH:old111\nU:{ts_old}\nD:old-date\nA:Old\nM:old commit\n\n1\t0\tf.rs\n\
             {d}\nH:new222\nU:{ts_recent}\nD:new-date\nA:New\nM:new commit\n\n1\t0\tf.rs\n",
            d = COMMIT_DELIMITER,
        );
        let index = GitTemporalIndex::compute_from_log(&log, now_unix);
        let h = index.files.get("f.rs").unwrap();
        assert_eq!(h.last_commit.hash, "new222");
        assert_eq!(h.last_commit.author, "New");
    }

    // ── Hotspots ────────────────────────────────────────────────────────

    #[test]
    fn test_hotspots_sorted_descending() {
        let now_unix = 1_741_520_000_u64;
        let mut log = String::new();
        // hot.rs: 5 recent commits. cold.rs: 1 old commit.
        for i in 0..5 {
            let ts = now_unix - (i + 1) * 86400;
            log.push_str(&format!(
                "{d}\nH:h{i}\nU:{ts}\nD:d\nA:X\nM:m\n\n1\t0\thot.rs\n",
                d = COMMIT_DELIMITER,
            ));
        }
        let ts_old = now_unix - 80 * 86400;
        log.push_str(&format!(
            "{d}\nH:c0\nU:{ts_old}\nD:d\nA:Y\nM:m\n\n1\t0\tcold.rs\n",
            d = COMMIT_DELIMITER,
        ));
        let index = GitTemporalIndex::compute_from_log(&log, now_unix);
        assert!(!index.stats.hotspots.is_empty());
        assert_eq!(index.stats.hotspots[0].0, "hot.rs");
        assert!(index.stats.hotspots[0].1 > index.stats.hotspots.last().unwrap().1);
    }

    // ── Pending / unavailable states ────────────────────────────────────

    #[test]
    fn test_pending_state() {
        let idx = GitTemporalIndex::pending();
        assert_eq!(idx.state, GitTemporalState::Pending);
        assert!(idx.files.is_empty());
    }

    #[test]
    fn test_unavailable_state() {
        let idx = GitTemporalIndex::unavailable("no git".to_string());
        assert_eq!(
            idx.state,
            GitTemporalState::Unavailable("no git".to_string())
        );
    }

    // ── Path normalization ──────────────────────────────────────────────

    #[test]
    fn test_normalize_git_path_backslash() {
        assert_eq!(normalize_git_path("src\\foo\\bar.rs"), "src/foo/bar.rs");
    }

    #[test]
    fn test_normalize_git_path_forward_slash_unchanged() {
        assert_eq!(normalize_git_path("src/foo/bar.rs"), "src/foo/bar.rs");
    }
}

// ── Test-only helper (separate impl block to keep test infra out of prod) ──

#[cfg(test)]
impl GitTemporalIndex {
    /// Compute from a pre-built log string (skips the `git log` subprocess).
    fn compute_from_log(raw_log: &str, now_unix: u64) -> Self {
        let start = Instant::now();
        let commits = parse_git_log(raw_log, now_unix);
        // Re-use the same computation logic — just inline the post-parse path.
        // We duplicate a bit to avoid making the `run_git_log` call.
        Self::compute_from_parsed(commits, start)
    }

    fn compute_from_parsed(commits: Vec<ParsedCommit>, start: Instant) -> Self {
        if commits.is_empty() {
            return Self {
                files: HashMap::new(),
                stats: GitTemporalStats {
                    total_commits_analyzed: 0,
                    analysis_window_days: WINDOW_DAYS,
                    hotspots: Vec::new(),
                    most_coupled: Vec::new(),
                    computed_at: SystemTime::now(),
                    compute_duration: start.elapsed(),
                },
                state: GitTemporalState::Ready,
            };
        }

        let total_commits = commits.len() as u32;
        let decay_lambda = (2.0_f64).ln() / HALF_LIFE_DAYS;

        let mut file_commit_indices: HashMap<String, Vec<usize>> = HashMap::new();
        let mut file_authors: HashMap<String, HashMap<String, u32>> = HashMap::new();
        let mut file_last_commit_idx: HashMap<String, usize> = HashMap::new();
        let mut file_raw_churn: HashMap<String, f64> = HashMap::new();

        for (idx, commit) in commits.iter().enumerate() {
            let weight = (-decay_lambda * commit.days_ago).exp();
            for file_path in &commit.files {
                file_commit_indices
                    .entry(file_path.clone())
                    .or_default()
                    .push(idx);
                *file_authors
                    .entry(file_path.clone())
                    .or_default()
                    .entry(commit.author.clone())
                    .or_insert(0) += 1;
                file_last_commit_idx
                    .entry(file_path.clone())
                    .and_modify(|existing| {
                        if commit.days_ago < commits[*existing].days_ago {
                            *existing = idx;
                        }
                    })
                    .or_insert(idx);
                *file_raw_churn.entry(file_path.clone()).or_insert(0.0) += weight;
            }
        }

        let mut churn_entries: Vec<(String, f64)> = file_raw_churn.into_iter().collect();
        churn_entries.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let file_count = churn_entries.len();
        let mut normalized_churn: HashMap<String, f32> = HashMap::with_capacity(file_count);
        for (rank, (path, _)) in churn_entries.iter().enumerate() {
            let score = if file_count <= 1 {
                if churn_entries[0].1 > 0.0 { 1.0 } else { 0.0 }
            } else {
                rank as f32 / (file_count - 1) as f32
            };
            normalized_churn.insert(path.clone(), score);
        }

        let mut pair_counts: HashMap<(String, String), u32> = HashMap::new();
        for commit in &commits {
            let mut sorted_files: Vec<&str> = commit.files.iter().map(|s| s.as_str()).collect();
            sorted_files.sort_unstable();
            sorted_files.dedup();
            if sorted_files.len() > MEGA_COMMIT_THRESHOLD {
                continue;
            }
            for i in 0..sorted_files.len() {
                for j in (i + 1)..sorted_files.len() {
                    let key = (sorted_files[i].to_string(), sorted_files[j].to_string());
                    *pair_counts.entry(key).or_insert(0) += 1;
                }
            }
        }

        let mut file_co_changes: HashMap<String, Vec<CoChangeEntry>> = HashMap::new();
        for ((file_a, file_b), shared) in &pair_counts {
            if *shared < MIN_SHARED_COMMITS {
                continue;
            }
            let count_a = file_commit_indices
                .get(file_a)
                .map(|v| v.len() as u32)
                .unwrap_or(0);
            let count_b = file_commit_indices
                .get(file_b)
                .map(|v| v.len() as u32)
                .unwrap_or(0);
            let union = count_a + count_b - shared;
            if union == 0 {
                continue;
            }
            let jaccard = *shared as f32 / union as f32;
            if jaccard < MIN_JACCARD {
                continue;
            }
            file_co_changes
                .entry(file_a.clone())
                .or_default()
                .push(CoChangeEntry {
                    path: file_b.clone(),
                    coupling_score: jaccard,
                    shared_commits: *shared,
                });
            file_co_changes
                .entry(file_b.clone())
                .or_default()
                .push(CoChangeEntry {
                    path: file_a.clone(),
                    coupling_score: jaccard,
                    shared_commits: *shared,
                });
        }
        for entries in file_co_changes.values_mut() {
            entries.sort_by(|a, b| {
                b.coupling_score
                    .partial_cmp(&a.coupling_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            entries.truncate(CO_CHANGE_CAP_PER_FILE);
        }

        let mut files: HashMap<String, GitFileHistory> = HashMap::with_capacity(file_count);
        for (path, commit_indices) in &file_commit_indices {
            let commit_count = commit_indices.len() as u32;
            let churn_score = normalized_churn.get(path).copied().unwrap_or(0.0);
            let last_idx = file_last_commit_idx.get(path).copied().unwrap_or(0);
            let last = &commits[last_idx];
            let last_commit = CommitSummary {
                hash: last.hash.clone(),
                timestamp: last.timestamp.clone(),
                author: last.author.clone(),
                message_head: truncate_message(&last.message, 72),
                days_ago: last.days_ago,
            };
            let contributors = file_authors
                .get(path)
                .map(|authors| {
                    let total = authors.values().sum::<u32>() as f32;
                    let mut shares: Vec<ContributorShare> = authors
                        .iter()
                        .map(|(author, count)| ContributorShare {
                            author: author.clone(),
                            commit_count: *count,
                            percentage: (*count as f32 / total) * 100.0,
                        })
                        .collect();
                    shares.sort_by(|a, b| {
                        b.percentage
                            .partial_cmp(&a.percentage)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    shares.truncate(CONTRIBUTOR_CAP);
                    shares
                })
                .unwrap_or_default();
            let co_changes = file_co_changes.remove(path).unwrap_or_default();
            files.insert(
                path.clone(),
                GitFileHistory {
                    commit_count,
                    churn_score,
                    last_commit,
                    contributors,
                    co_changes,
                },
            );
        }

        let mut hotspots: Vec<(String, f32)> = files
            .iter()
            .map(|(p, h)| (p.clone(), h.churn_score))
            .collect();
        hotspots.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        hotspots.truncate(HOTSPOT_CAP);

        let mut most_coupled: Vec<(String, String, f32)> = pair_counts
            .iter()
            .filter_map(|((a, b), shared)| {
                if *shared < MIN_SHARED_COMMITS {
                    return None;
                }
                let ca = file_commit_indices
                    .get(a)
                    .map(|v| v.len() as u32)
                    .unwrap_or(0);
                let cb = file_commit_indices
                    .get(b)
                    .map(|v| v.len() as u32)
                    .unwrap_or(0);
                let union = ca + cb - shared;
                if union == 0 {
                    return None;
                }
                let j = *shared as f32 / union as f32;
                if j < MIN_JACCARD {
                    return None;
                }
                Some((a.clone(), b.clone(), j))
            })
            .collect();
        most_coupled.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        most_coupled.truncate(COUPLED_PAIRS_CAP);

        Self {
            files,
            stats: GitTemporalStats {
                total_commits_analyzed: total_commits,
                analysis_window_days: WINDOW_DAYS,
                hotspots,
                most_coupled,
                computed_at: SystemTime::now(),
                compute_duration: start.elapsed(),
            },
            state: GitTemporalState::Ready,
        }
    }
}
