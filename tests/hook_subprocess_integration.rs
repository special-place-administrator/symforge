//! Subprocess-level end-to-end test for the hook adoption log wire-up.
//!
//! Follow-up to the in-crate tests added in `src/cli/hook.rs` by the
//! daemon-and-sidecar tentacle (swarm-2). Those tests pin the metric
//! rendering + the counter wire-up from `record_hook_outcome` into
//! `ADOPTION_LOG_FILE`, but they bypass `run_hook` entirely by calling
//! `record_hook_outcome` directly. That leaves one code-review-guarded
//! hop: someone could remove a `record_hook_outcome*` call site inside
//! `run_hook` and nothing automated would notice.
//!
//! This test closes that last hop for the no-sidecar dispatch site by
//! spawning the real `symforge` binary in a tempdir with no sidecar
//! running, piping a `PostToolUse/Read` hook payload on stdin, and
//! asserting that a `source-read` event is appended to
//! `.symforge/hook-adoption.log` under the tempdir.
//!
//! Remaining caveats (intentional scope for a future follow-up):
//!   - Only the no-sidecar site (port file missing + daemon fallback
//!     fails) is exercised here. The stale-port site and the routed
//!     success site remain code-review-guarded.
//!   - Windows file-locking on the just-created binary should not bite
//!     us because `Command::spawn` inherits the test binary's handle,
//!     not the target exe directly.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use tempfile::TempDir;

/// Matches `ADOPTION_LOG_FILE` in `src/cli/hook.rs`. Duplicated here by
/// design: if the constant is renamed the source-of-truth test in
/// `src/cli/hook.rs::tests::test_record_hook_outcome_writes_to_adoption_log_file_constant`
/// fails; if the log-file wire-up in `run_hook` is removed, this test
/// fails. The pair together pin the full chain.
const ADOPTION_LOG_RELATIVE: &str = ".symforge/hook-adoption.log";

/// Pin that `symforge hook`, invoked in a clean tempdir with no sidecar
/// port file and no matching daemon session, records a `no-sidecar`
/// adoption event for the `source-read` workflow.
///
/// Failure modes this test guards against:
///   - A refactor that removes the `record_hook_outcome_with_detail(...)`
///     call in `run_hook`'s no-sidecar branch — no log entry lands and
///     `health` silently drops to `0/N`.
///   - A rename of `ADOPTION_LOG_FILE` that drifts apart from its
///     consumers — the log file appears under the old name and the
///     snapshot loader can't find it.
///   - An accidental early-return before the dispatch path's
///     `record_hook_outcome*` call — same silent-failure shape.
#[test]
fn run_hook_no_sidecar_writes_source_read_adoption_event() {
    let tmp = TempDir::new().expect("tempdir creation");
    let bin = env!("CARGO_BIN_EXE_symforge");

    // Fresh tempdir: no .symforge/sidecar-port, no .symforge/ at all.
    // `run_hook` will take the "port file missing" branch, attempt a
    // daemon fallback, fail to find a matching project (tempdir path is
    // not a tracked daemon project), and record NoSidecar.
    let mut child = Command::new(bin)
        .arg("hook")
        .current_dir(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("symforge binary should spawn");

    // Minimal PostToolUse/Read payload. `tool_input.file_path` must end
    // in a source extension so `should_fail_open_read` lets this through
    // to the tracked `SourceRead` workflow (otherwise workflow_for_subcommand
    // returns PassThrough and no record_hook_outcome call fires — which
    // would turn this into a no-op rather than a regression test).
    let payload = r#"{"tool_name":"Read","tool_input":{"file_path":"src/foo.rs"}}"#;
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(payload.as_bytes())
        .expect("write hook payload to child stdin");
    drop(child.stdin.take());

    // `run_hook` has a ~500ms daemon fallback budget plus a ~50ms HTTP
    // timeout; a full subprocess round trip on a warm binary finishes
    // well under a second. Bound the wait so a hang here fails loudly
    // instead of wedging the test runner.
    let status = wait_with_timeout(&mut child, Duration::from_secs(15))
        .expect("hook subprocess should exit within 15s")
        .expect("hook subprocess status readable");
    assert!(
        status.success(),
        "symforge hook exited non-zero: status={status:?}"
    );

    let log_path = tmp.path().join(ADOPTION_LOG_RELATIVE);
    assert!(
        log_path.exists(),
        "run_hook must append to {ADOPTION_LOG_RELATIVE} under the child's cwd; \
         missing at {}. This usually means a record_hook_outcome* call was \
         removed from run_hook's no-sidecar dispatch branch.",
        log_path.display()
    );

    let contents = std::fs::read_to_string(&log_path).expect("log readable");
    // Tab-separated: session_id \t workflow_name \t outcome_label. Assert
    // on the (workflow, outcome) pair without pinning the session id —
    // it's unset in this subprocess (no daemon session file), which
    // `append_hook_adoption_event` normalizes to "-".
    assert!(
        contents.contains("\tsource-read\tno-sidecar"),
        "log must contain a tab-separated `source-read\\tno-sidecar` entry; \
         got:\n{contents}"
    );
}

/// Poll the child for exit with a timeout. Returns `Ok(Some(status))`
/// on clean exit, `Ok(None)` if the timeout elapses (child killed),
/// or `Err` on a `wait` failure. Kept local to avoid pulling in an
/// async runtime for one test.
fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> std::io::Result<Option<std::process::ExitStatus>> {
    let start = std::time::Instant::now();
    loop {
        match child.try_wait()? {
            Some(status) => return Ok(Some(status)),
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(None);
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
}
