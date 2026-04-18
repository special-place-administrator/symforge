//! Frecency ranking — bump hook surface for commitment tools.
//!
//! This module exposes the [`bump`] surface that commitment tools
//! (`get_file_context`, `get_file_content`, `get_symbol`, `get_symbol_context`)
//! call at the end of their happy path. The actual SQLite-backed store that
//! persists bumps is owned by a later todo of the `frecency-ranking` tentacle
//! (see `.octogent/tentacles/frecency-ranking/todo.md` item 2); this file
//! currently carries only the bump surface + a test-observability sink so
//! wiring tests have something to assert on.
//!
//! The bump surface is a stable contract:
//! * `bump` no-ops when `SYMFORGE_FRECENCY` is not `"1"` — flag off means zero
//!   side effects and zero cost.
//! * Callers pass an already-deduplicated slice of paths; batch tools collect
//!   into a `HashSet<PathBuf>` and pass the final set. Discovery tools
//!   (`search_files`, `search_text`, `search_symbols`) deliberately never
//!   call this function — see the spec for the positive-feedback-loop
//!   rationale (wiki `[[SymForge Frecency-Weighted File Ranking]]`
//!   §"Search tools deliberately do NOT bump").

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Env var that gates every bump call. `"1"` enables recording; anything
/// else (including unset) treats [`bump`] as a no-op.
pub const FRECENCY_FLAG_ENV: &str = "SYMFORGE_FRECENCY";

fn sink() -> &'static Mutex<Vec<PathBuf>> {
    static SINK: OnceLock<Mutex<Vec<PathBuf>>> = OnceLock::new();
    SINK.get_or_init(|| Mutex::new(Vec::new()))
}

/// Record that the given paths were accessed by a commitment tool.
///
/// No-op when `SYMFORGE_FRECENCY` is unset or not `"1"`. Infallible — callers
/// never need to handle errors; failure to record a bump is silently dropped
/// so the feature cannot break the tool it hooks into.
///
/// `paths` is expected to already be deduplicated (batch tools collect into a
/// `HashSet<PathBuf>` before calling).
pub fn bump(paths: &[PathBuf]) {
    if std::env::var(FRECENCY_FLAG_ENV).as_deref() != Ok("1") {
        return;
    }
    if paths.is_empty() {
        return;
    }
    if let Ok(mut guard) = sink().lock() {
        guard.extend(paths.iter().cloned());
    }
}

/// Drain and return every path recorded by [`bump`] since the last drain/clear.
///
/// Intended for wiring tests that need to observe whether a tool handler
/// actually called [`bump`]. Kept `pub` (behind `#[doc(hidden)]`) because the
/// integration-test crate lives outside the library crate and cannot use
/// `#[cfg(test)]`-gated items.
#[doc(hidden)]
pub fn drain_test_bumps() -> Vec<PathBuf> {
    match sink().lock() {
        Ok(mut guard) => std::mem::take(&mut *guard),
        Err(_) => Vec::new(),
    }
}

/// Clear the test-observability sink without returning its contents.
#[doc(hidden)]
pub fn clear_test_bumps() {
    if let Ok(mut guard) = sink().lock() {
        guard.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize tests that mutate the shared env var + sink. Tests run
    // single-threaded per CLAUDE.md; this lock is belt-and-suspenders so
    // any future parallel runner does not interleave env mutations.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_flag_on() {
        // SAFETY: tests hold ENV_LOCK and run with --test-threads=1; no
        // concurrent env readers can observe the transition.
        unsafe { std::env::set_var(FRECENCY_FLAG_ENV, "1") };
    }

    fn clear_flag() {
        // SAFETY: see set_flag_on.
        unsafe { std::env::remove_var(FRECENCY_FLAG_ENV) };
    }

    #[test]
    fn bump_is_noop_when_flag_unset() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_flag();
        clear_test_bumps();
        bump(&[PathBuf::from("src/lib.rs")]);
        assert!(
            drain_test_bumps().is_empty(),
            "bump with flag unset must not record"
        );
    }

    #[test]
    fn bump_is_noop_when_flag_not_one() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: see set_flag_on.
        unsafe { std::env::set_var(FRECENCY_FLAG_ENV, "0") };
        clear_test_bumps();
        bump(&[PathBuf::from("src/lib.rs")]);
        let recorded = drain_test_bumps();
        clear_flag();
        assert!(
            recorded.is_empty(),
            "bump with flag != 1 must not record, got {recorded:?}"
        );
    }

    #[test]
    fn bump_records_paths_when_flag_on() {
        let _g = ENV_LOCK.lock().unwrap();
        set_flag_on();
        clear_test_bumps();
        bump(&[PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")]);
        let recorded = drain_test_bumps();
        clear_flag();
        assert_eq!(
            recorded,
            vec![PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")],
            "bump with flag on must record every supplied path in order"
        );
    }

    #[test]
    fn bump_empty_slice_is_noop_when_flag_on() {
        let _g = ENV_LOCK.lock().unwrap();
        set_flag_on();
        clear_test_bumps();
        bump(&[]);
        let recorded = drain_test_bumps();
        clear_flag();
        assert!(
            recorded.is_empty(),
            "empty bump must not record even with flag on"
        );
    }

    #[test]
    fn drain_is_idempotent() {
        let _g = ENV_LOCK.lock().unwrap();
        set_flag_on();
        clear_test_bumps();
        bump(&[PathBuf::from("src/lib.rs")]);
        let first = drain_test_bumps();
        let second = drain_test_bumps();
        clear_flag();
        assert_eq!(first.len(), 1, "first drain returns recorded bump");
        assert!(second.is_empty(), "second drain returns nothing");
    }
}
