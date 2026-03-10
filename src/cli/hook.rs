//! Hook binary logic — reads `.tokenizor/sidecar.port`, calls sidecar over sync HTTP,
//! and outputs a single JSON line to stdout.
//!
//! Design constraints (HOOK-10):
//! - The ONLY thing written to stdout is the final JSON line.
//! - No tokio runtime. No tracing to stdout. No eprintln except for genuine errors.
//! - Sync I/O throughout — hooks must complete in well under 100 ms.
//! - Fail-open: if the sidecar is unreachable for any reason, output empty additionalContext
//!   JSON so Claude Code continues normally.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::cli::HookSubcommand;

const PORT_FILE: &str = ".tokenizor/sidecar.port";
/// Hard HTTP timeout — leaves margin within HOOK-03's 100 ms total budget.
const HTTP_TIMEOUT: Duration = Duration::from_millis(50);

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Entry point called by main.rs for `tokenizor hook <subcommand>`.
///
/// Reads the sidecar port from `.tokenizor/sidecar.port`, determines the
/// appropriate endpoint for the given subcommand, makes a sync HTTP GET, and
/// prints one JSON line to stdout.  Never returns an error — failures produce
/// the fail-open empty JSON.
pub fn run_hook(subcommand: &HookSubcommand) -> anyhow::Result<()> {
    let event_name = event_name_for(subcommand);

    // Step 1 — read port file.
    let port = match read_port_file() {
        Ok(p) => p,
        Err(_) => {
            // Sidecar not running — fail open silently.
            println!("{}", fail_open_json(event_name));
            return Ok(());
        }
    };

    // Step 2 — determine endpoint + query string.
    let (path, query) = endpoint_for(subcommand);

    // Step 3/4 — make sync HTTP GET with 50 ms timeout.
    let body = match sync_http_get(port, path, query) {
        Ok(b) => b,
        Err(_) => {
            println!("{}", fail_open_json(event_name));
            return Ok(());
        }
    };

    // Step 5/6 — output result JSON.
    println!("{}", success_json(event_name, &body));
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers (pub for unit-testing, not part of the public module API)
// ---------------------------------------------------------------------------

/// Returns the `hookEventName` string for a given subcommand.
pub fn event_name_for(subcommand: &HookSubcommand) -> &'static str {
    match subcommand {
        HookSubcommand::SessionStart => "SessionStart",
        _ => "PostToolUse",
    }
}

/// Maps a hook subcommand to `(path, query_string)`.
///
/// The query string is built from environment variables set by Claude Code:
/// - `TOKENIZOR_HOOK_FILE_PATH` — relative file path for Read/Edit hooks
/// - `TOKENIZOR_HOOK_QUERY`     — search query string for Grep hook
///
/// Phase 5 uses env vars as a simple stand-in for full stdin JSON parsing
/// (deferred to Phase 6). The HTTP plumbing works end-to-end with this approach.
pub fn endpoint_for(subcommand: &HookSubcommand) -> (&'static str, String) {
    match subcommand {
        HookSubcommand::Read => {
            let file = std::env::var("TOKENIZOR_HOOK_FILE_PATH").unwrap_or_default();
            let query = if file.is_empty() {
                String::new()
            } else {
                format!("path={}", url_encode(&file))
            };
            ("/outline", query)
        }
        HookSubcommand::Edit => {
            let file = std::env::var("TOKENIZOR_HOOK_FILE_PATH").unwrap_or_default();
            let query = if file.is_empty() {
                String::new()
            } else {
                format!("path={}", url_encode(&file))
            };
            ("/impact", query)
        }
        HookSubcommand::Grep => {
            let q = std::env::var("TOKENIZOR_HOOK_QUERY").unwrap_or_default();
            let query = if q.is_empty() {
                String::new()
            } else {
                format!("name={}", url_encode(&q))
            };
            ("/symbol-context", query)
        }
        HookSubcommand::SessionStart => ("/repo-map", String::new()),
    }
}

/// Returns the fail-open JSON: empty `additionalContext`.
pub fn fail_open_json(event_name: &str) -> String {
    format!(
        r#"{{"hookSpecificOutput":{{"hookEventName":"{event_name}","additionalContext":""}}}}"#
    )
}

/// Returns the success JSON with `context` as the `additionalContext` value.
///
/// The `context` string is JSON-escaped (backslash + quote safe) so it can be
/// embedded as a JSON string value.
pub fn success_json(event_name: &str, context: &str) -> String {
    let escaped = json_escape(context);
    format!(
        r#"{{"hookSpecificOutput":{{"hookEventName":"{event_name}","additionalContext":"{escaped}"}}}}"#
    )
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Read `.tokenizor/sidecar.port` from the current working directory.
fn read_port_file() -> std::io::Result<u16> {
    let contents = std::fs::read_to_string(PORT_FILE)?;
    contents
        .trim()
        .parse::<u16>()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Make a synchronous HTTP/1.1 GET request to `127.0.0.1:{port}{path}?{query}`.
///
/// Uses a raw `TcpStream` (no HTTP client crate) so there is no async runtime
/// and the startup cost is near zero.  The timeout covers both connect and read.
fn sync_http_get(port: u16, path: &str, query: String) -> anyhow::Result<String> {
    let addr = format!("127.0.0.1:{port}");
    let sock_addr: std::net::SocketAddr = addr.parse()?;

    let mut stream = TcpStream::connect_timeout(&sock_addr, HTTP_TIMEOUT)?;
    stream.set_read_timeout(Some(HTTP_TIMEOUT))?;
    stream.set_write_timeout(Some(HTTP_TIMEOUT))?;

    // Build the request line, including the query string if present.
    let request_path = if query.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{query}")
    };

    let request = format!(
        "GET {request_path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    );

    stream.write_all(request.as_bytes())?;

    // Read the full response (headers + body).
    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    // Split on the blank-line separator between headers and body.
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or("")
        .to_string();

    Ok(body)
}

/// Minimal percent-encoding for query parameter values.
///
/// Only encodes characters that are unsafe in a query string: space, `&`, `=`, `+`,
/// `%`, and non-ASCII bytes.  This is sufficient for file paths and symbol names.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'/'
            | b':' => out.push(b as char),
            b => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Minimal JSON string escape — handles backslash, double-quote, and common
/// control characters.  Sufficient for embedding sidecar response bodies.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    // --- fail_open_json ---

    #[test]
    fn test_fail_open_json_is_valid() {
        let json = fail_open_json("PostToolUse");
        let v: Value = serde_json::from_str(&json)
            .expect("fail_open_json must produce valid JSON");

        let output = &v["hookSpecificOutput"];
        assert_eq!(output["hookEventName"], "PostToolUse");
        assert_eq!(output["additionalContext"], "");
    }

    #[test]
    fn test_fail_open_json_session_start_event_name() {
        let json = fail_open_json("SessionStart");
        let v: Value = serde_json::from_str(&json).expect("must be valid JSON");
        assert_eq!(v["hookSpecificOutput"]["hookEventName"], "SessionStart");
    }

    // --- success_json ---

    #[test]
    fn test_success_json_is_valid() {
        let json = success_json("PostToolUse", "hello world");
        let v: Value = serde_json::from_str(&json)
            .expect("success_json must produce valid JSON");

        let output = &v["hookSpecificOutput"];
        assert_eq!(output["hookEventName"], "PostToolUse");
        assert_eq!(output["additionalContext"], "hello world");
    }

    #[test]
    fn test_success_json_escapes_special_chars() {
        let context = r#"{"key":"value"}"#;
        let json = success_json("PostToolUse", context);
        // The outer JSON must parse correctly.
        let v: Value = serde_json::from_str(&json)
            .expect("success_json with embedded quotes must be valid JSON");
        // The additionalContext value is the escaped string, not a nested object.
        let ctx = v["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("additionalContext must be a string");
        assert_eq!(ctx, context);
    }

    // --- endpoint_for ---

    #[test]
    fn test_hook_subcommand_to_endpoint_read() {
        // Without env var set, query is empty.
        let (path, _query) = endpoint_for(&HookSubcommand::Read);
        assert_eq!(path, "/outline");
    }

    #[test]
    fn test_hook_subcommand_to_endpoint_edit() {
        let (path, _query) = endpoint_for(&HookSubcommand::Edit);
        assert_eq!(path, "/impact");
    }

    #[test]
    fn test_hook_subcommand_to_endpoint_grep() {
        let (path, _query) = endpoint_for(&HookSubcommand::Grep);
        assert_eq!(path, "/symbol-context");
    }

    #[test]
    fn test_hook_subcommand_to_endpoint_session_start() {
        let (path, query) = endpoint_for(&HookSubcommand::SessionStart);
        assert_eq!(path, "/repo-map");
        assert!(query.is_empty(), "repo-map has no query params");
    }

    #[test]
    fn test_hook_subcommand_read_includes_file_path_in_query() {
        // SAFETY: env var mutation — test isolation is best-effort; tests run in
        // separate processes or with --test-threads=1 if this causes flakiness.
        // `set_var`/`remove_var` are unsafe in Rust 2024 due to potential UB in
        // multi-threaded contexts; we accept this for test-only usage.
        unsafe {
            std::env::set_var("TOKENIZOR_HOOK_FILE_PATH", "src/main.rs");
        }
        let (path, query) = endpoint_for(&HookSubcommand::Read);
        unsafe {
            std::env::remove_var("TOKENIZOR_HOOK_FILE_PATH");
        }
        assert_eq!(path, "/outline");
        assert!(query.contains("src"), "query must include the file path");
    }

    // --- event_name_for ---

    #[test]
    fn test_event_name_for_session_start() {
        assert_eq!(event_name_for(&HookSubcommand::SessionStart), "SessionStart");
    }

    #[test]
    fn test_event_name_for_post_tool_use_variants() {
        for sub in [HookSubcommand::Read, HookSubcommand::Edit, HookSubcommand::Grep] {
            assert_eq!(
                event_name_for(&sub),
                "PostToolUse",
                "Read/Edit/Grep must produce PostToolUse event name"
            );
        }
    }
}
