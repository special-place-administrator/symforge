# Parallelism & Concurrency Review

**Date:** 2026-03-15
**Focus:** Why parallel subagents stall when using Tokenizor MCP simultaneously

---

## Architecture Summary

```
Agent 1 (stdio) ─┐
Agent 2 (stdio) ─┤──► Shared Daemon (HTTP, single process)
Agent 3 (stdio) ─┤      │
Agent 4 (stdio) ─┘      ├─ Governor (8 permits, write gate)
                         ├─ spawn_blocking → execute_tool_call
                         ├─ SharedIndex (std::sync::RwLock × 4)
                         └─ Watcher (tokio::spawn, no spawn_blocking)
```

Each agent runs its own `tokenizor` stdio process, but all proxy to **one shared daemon**. The daemon has a **single governor** controlling all concurrent tool executions across all agents.

---

## Root Causes of Stalls (ranked by impact)

### 1. Governor semaphore is too small for multi-agent use
**File:** `src/sidecar/governor.rs:21` | **Impact: HIGH**

The governor has **8 total permits** shared across ALL agents. Tool weights:
- Light (1 permit): `get_symbol`, `search_text`, `get_file_context`, etc.
- Medium (2 permits): `replace_symbol_body`, `analyze_file_impact`
- Heavy (3 permits): `batch_edit`, `batch_rename`, `index_folder`

**Scenario:** 4 subagents each fire 2 concurrent Light calls = 8 permits consumed. Any 9th request from any agent queues for up to **15 seconds** before timeout error.

**Scenario:** 1 agent does `batch_rename` (Heavy, 3 permits + exclusive write gate) → all other agents blocked until it finishes.

**Recommendation:**
- Increase default permits to 16-24 for multi-agent workloads
- Make permit count configurable (env var or config file)
- Consider per-session fair-share scheduling (each session gets guaranteed minimum permits) instead of global FIFO
- Consider splitting read vs write permit pools — reads don't compete with each other at the index level

### 2. Heavy ops take exclusive write gate, blocking ALL reads
**File:** `src/sidecar/governor.rs` (write_gate: `tokio::sync::RwLock<()>`) | **Impact: HIGH**

When any Heavy tool (batch_edit, batch_rename, index_folder) executes, it acquires `write_gate.write()`. ALL Light/Medium tools acquire `write_gate.read()`. This means **one batch_edit blocks every concurrent search, get_symbol, etc. across all agents** — even though reads don't actually conflict with each other at the data level.

**Recommendation:**
- The write gate is overly coarse. The actual data conflict is at the `live` RwLock level, which already handles read/write exclusion. Consider removing the governor-level write gate and relying on the index-level RwLock instead, or making the write gate per-project rather than global.

### 3. `index_folder` bypasses governor AND spawn_blocking
**File:** `src/daemon.rs:1073-1078` | **Impact: HIGH**

`index_folder` is special-cased in `call_tool_handler` to skip both the governor and `spawn_blocking`. It runs a full Rayon reload (reads every file, parses every file, rebuilds trigram index) **directly on an async worker thread** while holding `projects.write()`.

Effects:
- Steals a tokio worker thread for seconds
- Holds `projects.write()`, blocking every `session_runtime()` call (needed by every tool)
- Holds `live.write()` for the full I/O + parse duration, blocking all concurrent reads

**Recommendation:**
- Wrap in `spawn_blocking` like every other tool
- Route through governor with Heavy weight
- Consider a staged reload: build new index in background, then swap under write lock (brief)

### 4. `reload()` holds write lock for entire file I/O duration
**File:** `src/live_index/store.rs:386-391, 802-898` | **Impact: MEDIUM-HIGH**

`SharedIndexHandle::reload()` acquires `live.write()` and then does:
1. Rayon parallel `std::fs::read` on every file
2. Rayon parallel tree-sitter parse
3. Full trigram index rebuild
4. Reverse index + path index rebuild

All concurrent `search_text`, `get_symbol`, etc. are blocked for the entire duration.

**Recommendation:**
- Build the new `LiveIndex` in a separate allocation without any lock
- Only acquire `live.write()` to swap the old index with the new one (milliseconds instead of seconds)
- This is the single highest-impact architectural change for parallelism

### 5. Watcher does blocking I/O on async runtime
**File:** `src/watcher/mod.rs:210, 238` | **Impact: MEDIUM**

`run_watcher` is a `tokio::spawn` task. Inside `maybe_reindex`:
- `std::fs::read` (blocking I/O) at line 210
- tree-sitter parse (CPU-heavy) at line 238

During a file-change burst (e.g., `git checkout` switching branches), this steals tokio worker threads.

**Recommendation:**
- Wrap `maybe_reindex` in `tokio::task::spawn_blocking`

### 6. `heartbeat` takes `sessions.write()` for a timestamp update
**File:** `src/daemon.rs:248-264` | **Impact: LOW-MEDIUM**

`heartbeat()` acquires `sessions.write()` just to update `last_seen_at`. With N agents sending heartbeats, this serializes all heartbeats AND blocks concurrent `sessions.read()` callers (every tool call goes through `session_runtime` which reads sessions).

**Recommendation:**
- Use `AtomicU64` (epoch millis) for `last_seen_at` instead of holding the write lock
- Or use a per-session `Mutex` for the timestamp, separate from the sessions map lock

---

## Lock Inventory (for reference)

| Lock | Type | Location | Held During |
|------|------|----------|-------------|
| `governor.semaphore` | `tokio::sync::Semaphore(8)` | governor.rs | Every tool call |
| `governor.write_gate` | `tokio::sync::RwLock<()>` | governor.rs | Every tool call (read for Light/Medium, write for Heavy) |
| `live` | `std::sync::RwLock<LiveIndex>` | store.rs:358 | Every query (read) and every file update (write) |
| `published_state` | `std::sync::RwLock<Arc<...>>` | store.rs | Health checks, brief |
| `published_repo_outline` | `std::sync::RwLock<Arc<...>>` | store.rs | Outline reads, brief |
| `git_temporal` | `std::sync::RwLock<Arc<...>>` | store.rs | Git history queries, brief |
| `projects` | `std::sync::RwLock<HashMap>` | daemon.rs | Session routing, reload |
| `sessions` | `std::sync::RwLock<HashMap>` | daemon.rs | Session routing, heartbeat |
| `symbol_cache` | `std::sync::RwLock<HashMap>` | daemon.rs | Pre-edit snapshots |
| `watcher_info` | `std::sync::Mutex` | daemon.rs | Watcher health stats |
| `governor.active` | `std::sync::Mutex` | governor.rs | Request tracking |

**Lock ordering (correct, no deadlock cycles):**
- `projects` → `sessions` (documented, consistently followed)
- `live` → `published_state` → `published_repo_outline` (nested in `publish_locked`)

---

## What's Done Right

- Lock ordering is documented and consistently followed — no deadlock risk
- Most tool calls use `spawn_blocking` correctly
- `reqwest::Client` has timeouts configured (5s connect, 60s total)
- Governor has queue timeout (15s) preventing infinite hangs
- `published_state` provides a fast path for health/status that avoids the main index lock
- Watcher's `maybe_reindex` does file I/O outside the index lock (correct discipline)
- Git temporal computation uses `spawn_blocking` correctly

---

## Recommended Changes (prioritized)

| Priority | Change | Impact | Effort |
|----------|--------|--------|--------|
| **P0** | Build new index outside lock, swap under write lock | Eliminates multi-second read stalls during reload | Medium |
| **P0** | Wrap `index_folder` in `spawn_blocking` + governor | Stops async thread starvation + projects.write() stall | Low |
| **P1** | Increase governor permits to 16-24, make configurable | Immediate relief for multi-agent workloads | Low |
| **P1** | Remove or per-project-ify the write gate | Reads stop being blocked by unrelated writes | Medium |
| **P2** | Wrap watcher `maybe_reindex` in `spawn_blocking` | Prevents tokio thread starvation during file bursts | Low |
| **P2** | Use `AtomicU64` for heartbeat `last_seen_at` | Eliminates sessions write lock contention from heartbeats | Low |
| **P3** | Per-session fair-share scheduling in governor | Prevents one agent from starving others | Medium-High |
