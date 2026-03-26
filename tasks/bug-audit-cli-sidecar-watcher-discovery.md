# Bug Audit: CLI, Sidecar, Watcher, Discovery Subsystems

**Date:** 2026-03-20
**Scope:** src/cli/, src/sidecar/, src/watcher/, src/discovery/, src/main.rs, src/lib.rs

---

## Bug 1: `check_stale` silently swallows address parse failures, connects to wrong address

**File:** `src/sidecar/port_file.rs`, lines 105-108
**Severity:** Medium (wrong behavior)

```rust
match TcpStream::connect_timeout(
    &addr
        .parse()
        .unwrap_or_else(|_| "127.0.0.1:0".parse().unwrap()),
    Duration::from_millis(200),
)
```

**What it does wrong:** When `addr` fails to parse (e.g., the bind host is an unusual format), the fallback silently connects to `127.0.0.1:0` — port 0. This will always fail to connect (no server listens on port 0), causing `check_stale` to return `true` and **delete the port/PID files of a live sidecar**. The running sidecar becomes unreachable because its port file is deleted.

**Correct behavior:** If the address cannot be parsed, the function should return `false` (assume not stale / cannot determine) rather than falling through to a connection attempt that will always fail and trigger cleanup.

---

## Bug 2: `classify_tool` does not classify single-file edit tools as needing the write gate

**File:** `src/sidecar/governor.rs`, lines 111-126
**Severity:** Low-Medium (potential data corruption under concurrency)

```rust
pub fn classify_tool(tool_name: &str) -> ToolWeight {
    match tool_name {
        "index_folder" | "batch_edit" | "batch_rename" | "batch_insert" => ToolWeight::Heavy,
        "replace_symbol_body"
        | "edit_within_symbol"
        | "insert_symbol"
        | "delete_symbol"
        | "analyze_file_impact" => ToolWeight::Medium,
        _ => ToolWeight::Light,
    }
}
```

**What it does wrong:** `replace_symbol_body`, `edit_within_symbol`, `insert_symbol`, and `delete_symbol` are all **write operations** that mutate files and the index. They are classified as `ToolWeight::Medium`, which does **not** acquire the write gate (`needs_write_gate()` returns `false` for Medium). This means these single-file write operations can run concurrently with each other and with batch write operations that DO hold the write gate.

Two concurrent `replace_symbol_body` calls on the same file could read stale pre-edit state and overwrite each other's changes. A `replace_symbol_body` running concurrently with a `batch_edit` (which holds the write gate as Heavy) would not be blocked because Medium only takes the read side of the RwLock gate.

**Correct behavior:** Single-file write operations should either be Heavy (requiring exclusive write gate) or at minimum use the write gate. The governor's write gate should protect all mutations, not just batch ones.

---

## Bug 3: `handle_edit_impact` reads mtime AFTER content (TOCTOU)

**File:** `src/sidecar/handlers.rs`, lines 603-611
**Severity:** Low-Medium (silently stale index)

```rust
let outcome = tokio::task::spawn_blocking(move || match std::fs::read(&abs_path) {
    Ok(bytes) => {
        let result = crate::parsing::process_file(&path_owned, &bytes, language);
        let mtime_secs = std::fs::metadata(&abs_path)  // <-- mtime read AFTER content
            .and_then(|m| m.modified())
            ...
```

**What it does wrong:** The code reads the file content first, then reads the mtime. If the file is modified between these two operations, the stored mtime will be newer than the content that was actually parsed. This means the watcher's `freshen_file_if_stale` check will see the mtime matches disk and skip re-indexing, permanently hiding the change.

**Note:** The watcher's `maybe_reindex` function (watcher/mod.rs line 226) correctly reads mtime BEFORE content with an explicit comment explaining why. The handlers version has the opposite (wrong) order.

**Correct behavior:** Read mtime before content, as `maybe_reindex` does, so that any write between stat and read produces a stale-looking mtime that triggers future re-indexing.

---

## Bug 4: `handle_new_file_impact` also reads mtime AFTER content (same TOCTOU)

**File:** `src/sidecar/handlers.rs`, lines 469-478
**Severity:** Low-Medium (same class as Bug 3)

```rust
let (bytes, result, mtime_secs) =
    tokio::task::spawn_blocking(move || -> Result<_, StatusCode> {
        let bytes = std::fs::read(&abs_path).map_err(|_| StatusCode::NOT_FOUND)?;
        let result = crate::parsing::process_file(&path_owned, &bytes, lang_clone);
        let mtime_secs = std::fs::metadata(&abs_path)  // <-- AFTER content read
            ...
```

Same issue as Bug 3. Mtime should be read before content.

---

## Bug 5: `run_watcher` uses blocking `recv_timeout` on an async task without `spawn_blocking`

**File:** `src/watcher/mod.rs`, lines 531-533
**Severity:** Low (performance / starvation risk, mitigated by short timeout)

```rust
match handle
    .event_rx
    .recv_timeout(Duration::from_millis(RECV_TIMEOUT_MS))
```

**What it does wrong:** `run_watcher` is an `async fn` spawned as a tokio task. It calls `recv_timeout` (a blocking std::sync::mpsc operation) directly on a tokio worker thread. While the 50ms timeout limits the blocking window, under high load with many concurrent tasks, repeatedly blocking a tokio worker for up to 50ms per iteration can degrade throughput.

The code comments acknowledge this and use `yield_now()` on timeout, but the blocking call itself still ties up a worker thread. The `process_events` call IS correctly offloaded to `spawn_blocking`, making this inconsistency notable.

**Correct behavior:** Either use `tokio::sync::mpsc` instead of `std::sync::mpsc`, or wrap the recv loop in `spawn_blocking`. The current approach is a known tradeoff documented in comments but is still technically incorrect async usage.

---

## Bug 6: `discover_files` and `discover_all_files` may return non-canonical absolute paths

**File:** `src/discovery/mod.rs`, lines 50, 99
**Severity:** Low (subtle path mismatch bugs)

```rust
let path = entry.path().to_path_buf();
```

**What it does wrong:** The `absolute_path` field of `DiscoveredFile`/`DiscoveredEntry` is set to the raw path from the walker, which may contain symlinks or non-canonical components. Meanwhile, `find_project_root` uses raw `current_dir()` and `.git` existence checks without canonicalization. If the project root is accessed via a symlink, the `absolute_path` in discovered files may not be consistent with `repo_root`, causing `strip_prefix` to fail for some files.

The watcher's `normalize_event_path` handles this for events (stripping `\\?\` prefix), but the initial discovery does not apply equivalent normalization.

**Correct behavior:** Canonicalize the root path and discovered paths consistently, or at minimum document the assumption that the walker is invoked with a non-symlinked root.

---

## Bug 7: `sync_http_get_with_timeout` does not handle chunked transfer encoding

**File:** `src/cli/hook.rs`, lines 838-889
**Severity:** Low (wrong results with chunked responses)

```rust
let mut response = String::new();
stream.read_to_string(&mut response)?;

let (headers, body) = response
    .split_once("\r\n\r\n")
    .ok_or_else(|| anyhow::anyhow!("malformed HTTP response: no header/body separator"))?;
```

**What it does wrong:** The response parser assumes the body is a raw string after the header separator. If the sidecar ever returns a chunked transfer-encoding response (which axum can do under certain conditions), the body will contain chunk size markers mixed into the actual content, producing garbled output shown to the user.

Currently axum with `Connection: close` typically does not chunk, so this is latent. But if axum's behavior changes or a proxy is introduced, this will silently corrupt hook output.

**Correct behavior:** Either set `Connection: close` (already done) AND verify `Transfer-Encoding` is not chunked, or parse chunked encoding. Given the `Connection: close` header is sent, this is mostly theoretical but worth noting.

---

## Bug 8: `session_errors` counter can overflow and wrap for persistent errors

**File:** `src/watcher/mod.rs`, line 592
**Severity:** Very Low (edge case)

```rust
session_errors += errors.len() as u32;
```

**What it does wrong:** If `errors.len()` returns a very large value (e.g., the notify backend reports thousands of errors at once), the addition could overflow `u32::MAX`. With Rust's default debug-mode panic on overflow, this would crash the watcher. In release mode, it would wrap around, potentially preventing the `>= MAX_SESSION_ERRORS` check from triggering.

**Correct behavior:** Use `session_errors = session_errors.saturating_add(errors.len() as u32)`.

---

## Non-bugs investigated but cleared:

1. **`TokenStats::tool_calls` uses `std::sync::Mutex` inside async context** — The lock is held only briefly for HashMap insert/read, and `parking_lot::Mutex` is used elsewhere. The `std::sync::Mutex` here is fine since it's never held across await points.

2. **`port_file` functions use relative paths from CWD** — This is by design; hooks are invoked with CWD set to the project root.

3. **`build_with_budget` first-item-exceeds-budget path** — Correctly handled with the post-loop check at line 204.

4. **`governor` peak_in_flight tracking** — Uses `fetch_max` which is correct for tracking the high-water mark.

5. **`normalize_event_path` Windows `\\?\` stripping** — Correctly handles both prefixed and non-prefixed paths with fallback logic.

6. **`is_forbidden_root` canonicalization** — Uses `unwrap_or` on canonicalize failure, which is acceptable since the fallback just uses the original path.
