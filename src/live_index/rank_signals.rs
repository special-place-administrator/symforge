//! Rank-signal extension point for the search ranker fusion.
//!
//! Feature tentacles register additional [`RankSignal`] impls to contribute to
//! the weighted sum computed by [`combine`]. This layer has no knowledge of
//! specific features — see ADR 0012 for the pattern. The two default signals
//! (`PathMatchSignal`, `CoChangeSignal`) wrap today's path-match and
//! git-temporal co-change inputs with no-op scoring; the search layer still
//! uses its existing comparator-based path ordering until the fusion is
//! migrated over (see todo #4, second bullet).

use std::path::Path;
use std::sync::{OnceLock, RwLock};

/// Contextual inputs shared by every registered `RankSignal` when scoring a
/// candidate path. Fields are borrowed from the caller for the duration of a
/// single `combine()` invocation.
#[derive(Debug, Clone, Copy)]
pub struct RankCtx<'a> {
    /// Normalized user query that produced the candidate set (may be empty).
    pub query: &'a str,
    /// Tokenized query as interpreted by the caller (e.g., path components).
    pub tokens: &'a [String],
    /// Optional current editor file used for proximity-style boosts.
    pub current_file: Option<&'a str>,
    /// Optional anchor path for co-change fusion (`changed_with=...`).
    pub target_path: Option<&'a str>,
}

impl<'a> RankCtx<'a> {
    /// Construct an empty context with no query, tokens, or anchors.
    pub const fn empty() -> Self {
        Self {
            query: "",
            tokens: &[],
            current_file: None,
            target_path: None,
        }
    }
}

impl Default for RankCtx<'_> {
    fn default() -> Self {
        Self::empty()
    }
}

/// Extension point for contributing to the search ranker's weighted sum.
///
/// Implementations must be object-safe so they can be stored as
/// `Box<dyn RankSignal>` inside the process-wide registry.
pub trait RankSignal: Send + Sync {
    /// Stable identifier used for diagnostics.
    fn name(&self) -> &'static str;

    /// Per-signal weight applied to its `score()` contribution during fusion.
    fn weight(&self) -> f32;

    /// Score the given `path` against the shared `ctx`. Return `0.0` when the
    /// signal has nothing to say — this keeps the fusion well-defined when
    /// required inputs are missing.
    fn score(&self, path: &Path, ctx: &RankCtx<'_>) -> f32;
}

/// Path-match signal — reserves the slot for today's lexical path-match
/// contribution. The default impl returns `0.0`; the search layer continues
/// to rely on its tier-based comparator until the fusion is migrated over.
pub struct PathMatchSignal;

impl RankSignal for PathMatchSignal {
    fn name(&self) -> &'static str {
        "path_match"
    }

    fn weight(&self) -> f32 {
        1.0
    }

    fn score(&self, _path: &Path, _ctx: &RankCtx<'_>) -> f32 {
        0.0
    }
}

/// Co-change signal — reserves the slot for today's git-temporal coupling
/// contribution. The default impl returns `0.0`; feature tentacles can
/// register their own impl to augment or replace it.
pub struct CoChangeSignal;

impl RankSignal for CoChangeSignal {
    fn name(&self) -> &'static str {
        "co_change"
    }

    fn weight(&self) -> f32 {
        1.0
    }

    fn score(&self, _path: &Path, _ctx: &RankCtx<'_>) -> f32 {
        0.0
    }
}

fn registry() -> &'static RwLock<Vec<Box<dyn RankSignal>>> {
    static REGISTRY: OnceLock<RwLock<Vec<Box<dyn RankSignal>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let defaults: Vec<Box<dyn RankSignal>> =
            vec![Box::new(PathMatchSignal), Box::new(CoChangeSignal)];
        RwLock::new(defaults)
    })
}

/// Append a `RankSignal` to the process-wide registry. Feature tentacles call
/// this at their own initialization time; the registry never empties.
pub fn register(signal: Box<dyn RankSignal>) {
    let mut guard = registry().write().expect("rank_signals registry poisoned");
    guard.push(signal);
}

/// Weighted-sum fusion over every registered `RankSignal`.
pub fn combine(path: &Path, ctx: &RankCtx<'_>) -> f32 {
    let guard = registry().read().expect("rank_signals registry poisoned");
    guard
        .iter()
        .map(|signal| signal.weight() * signal.score(path, ctx))
        .sum()
}

#[cfg(test)]
fn registered_count() -> usize {
    registry()
        .read()
        .expect("rank_signals registry poisoned")
        .len()
}

#[cfg(test)]
fn reset_for_tests() {
    let mut guard = registry().write().expect("rank_signals registry poisoned");
    guard.clear();
    guard.push(Box::new(PathMatchSignal));
    guard.push(Box::new(CoChangeSignal));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_register_two_signals() {
        reset_for_tests();
        assert_eq!(registered_count(), 2);
    }

    #[test]
    fn default_signals_expose_stable_names() {
        assert_eq!(PathMatchSignal.name(), "path_match");
        assert_eq!(CoChangeSignal.name(), "co_change");
    }

    #[test]
    fn default_signals_score_zero_on_any_input() {
        let ctx = RankCtx::empty();
        assert_eq!(PathMatchSignal.score(Path::new("foo.rs"), &ctx), 0.0);
        assert_eq!(CoChangeSignal.score(Path::new("foo.rs"), &ctx), 0.0);
    }

    #[test]
    fn rank_signal_is_object_safe() {
        let _boxed: Box<dyn RankSignal> = Box::new(PathMatchSignal);
        let _erased: &dyn RankSignal = &CoChangeSignal;
    }

    #[test]
    fn combine_with_defaults_returns_zero() {
        reset_for_tests();
        let ctx = RankCtx::empty();
        assert_eq!(combine(Path::new("src/live_index/rank_signals.rs"), &ctx), 0.0);
    }

    #[test]
    fn combine_sums_weighted_scores_from_registered_signals() {
        struct FixedTwoTimesThree;
        impl RankSignal for FixedTwoTimesThree {
            fn name(&self) -> &'static str {
                "__test_two_times_three"
            }
            fn weight(&self) -> f32 {
                2.0
            }
            fn score(&self, _path: &Path, _ctx: &RankCtx<'_>) -> f32 {
                3.0
            }
        }

        struct FixedHalfTimesFour;
        impl RankSignal for FixedHalfTimesFour {
            fn name(&self) -> &'static str {
                "__test_half_times_four"
            }
            fn weight(&self) -> f32 {
                0.5
            }
            fn score(&self, _path: &Path, _ctx: &RankCtx<'_>) -> f32 {
                4.0
            }
        }

        reset_for_tests();
        register(Box::new(FixedTwoTimesThree));
        register(Box::new(FixedHalfTimesFour));

        let ctx = RankCtx::empty();
        let total = combine(Path::new("anything"), &ctx);
        // defaults contribute 0.0; (2.0 * 3.0) + (0.5 * 4.0) = 8.0
        assert!((total - 8.0).abs() < f32::EPSILON);

        reset_for_tests();
    }

    #[test]
    fn rank_ctx_default_matches_empty() {
        let default = RankCtx::default();
        let empty = RankCtx::empty();
        assert_eq!(default.query, empty.query);
        assert_eq!(default.tokens.len(), empty.tokens.len());
        assert_eq!(default.current_file, empty.current_file);
        assert_eq!(default.target_path, empty.target_path);
    }
}
