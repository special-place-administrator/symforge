# Daemon Tool Proxy Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Route MCP tool traffic through the shared local daemon so concurrent Claude, Codex, and other stdio clients in the same project share one authoritative project instance.

**Architecture:** The daemon becomes the owner of project-scoped runtime state: shared index, watcher state, hook token stats, and per-session membership. Each stdio MCP process registers a daemon session, then proxies tool invocations to session-scoped daemon endpoints. Claude hook traffic reuses the existing cwd-local hook files, but those files now point to daemon-backed routes instead of a per-process in-memory sidecar.

**Tech Stack:** Rust, axum, rmcp, tokio, reqwest, serde_json

---

### Task 1: Add failing daemon proxy tests

**Files:**
- Modify: `src/daemon.rs`
- Modify: `src/protocol/tools.rs`
- Modify: `src/cli/hook.rs`

**Step 1: Write failing daemon HTTP tests**

- Add a daemon integration test that:
  - opens a project session
  - calls a new session-scoped tool endpoint
  - verifies the tool output comes from the daemon-owned project instance
- Add a daemon integration test that:
  - calls a new session-scoped hook/sidecar endpoint
  - verifies the response matches the current hook contract

**Step 2: Write failing proxy server tests**

- Add a protocol test that:
  - creates a remote-backed `TokenizorServer`
  - invokes at least `get_repo_outline` and `search_symbols`
  - confirms the calls succeed through the daemon instead of a local index

**Step 3: Write failing hook routing tests**

- Add unit tests for hook request path construction when a daemon session id is present
- Verify backward compatibility when no session file exists

### Task 2: Make the daemon the owner of shared project runtime

**Files:**
- Modify: `src/daemon.rs`
- Modify: `src/watcher/mod.rs` if needed

**Step 1: Extend `ProjectInstance`**

- Add watcher state ownership:
  - `watcher_info: Arc<Mutex<WatcherInfo>>`
  - `watcher_task: JoinHandle<()>`
- Add hook/session shared state:
  - `token_stats: Arc<TokenStats>`
  - `symbol_cache: Arc<RwLock<HashMap<String, Vec<SymbolSnapshot>>>>`

**Step 2: Start project watcher inside daemon project creation**

- When a project instance is created:
  - load the index once
  - start the watcher once
  - keep both tied to the project instance

**Step 3: Stop project runtime cleanly**

- When the last session leaves a project:
  - abort the watcher task
  - remove the project instance from the registry

### Task 3: Add daemon tool execution endpoints

**Files:**
- Modify: `src/daemon.rs`
- Modify: `src/protocol/tools.rs`
- Modify: `src/protocol/mod.rs`

**Step 1: Extract reusable tool execution helpers**

- Move the actual local tool logic behind reusable functions that accept a shared execution context
- Keep `TokenizorServer` methods thin wrappers around either:
  - local execution helpers
  - remote daemon proxy calls

**Step 2: Add session-scoped daemon tool dispatch**

- Add a daemon endpoint like:
  - `POST /v1/sessions/{session_id}/tools/{tool_name}`
- Deserialize parameters from `serde_json::Value`
- Execute against the daemon-owned project instance
- Return plain text tool output

**Step 3: Support session-aware `index_folder`**

- If the requested path is the current project root:
  - reload that project in place
- If the requested path points to a different root:
  - rebind the session to the canonical target project
  - create or join the target project instance
  - restart watcher ownership on the updated project when needed

### Task 4: Add daemon client and stdio MCP proxy wiring

**Files:**
- Modify: `src/daemon.rs`
- Modify: `src/main.rs`
- Modify: `src/protocol/mod.rs`
- Modify: `src/protocol/tools.rs`
- Modify: `Cargo.toml`

**Step 1: Add daemon client helpers**

- Add a daemon client type that can:
  - ensure a daemon is running
  - open a session
  - heartbeat
  - close session
  - call tool endpoints

**Step 2: Auto-start or connect to the daemon**

- On stdio MCP startup:
  - discover project root
  - connect to existing daemon from daemon port file if alive
  - otherwise spawn a detached `tokenizor daemon`
  - wait for daemon health to respond

**Step 3: Build remote-backed `TokenizorServer`**

- In daemon-backed mode:
  - do not load a second authoritative project index for MCP tools
  - construct the server with daemon proxy state
  - keep session lifecycle tied to stdio process shutdown

### Task 5: Proxy Claude hook traffic through daemon session routes

**Files:**
- Modify: `src/daemon.rs`
- Modify: `src/cli/hook.rs`
- Modify: `src/sidecar/port_file.rs`
- Modify: `src/main.rs`
- Modify: `tests/sidecar_integration.rs`

**Step 1: Add daemon-backed hook endpoints**

- Add session-scoped daemon routes for:
  - `health`
  - `outline`
  - `impact`
  - `symbol-context`
  - `repo-map`
  - `stats`
- Reuse current sidecar formatting/behavior against daemon-owned shared state

**Step 2: Write proxy hook files from stdio startup**

- In daemon-backed mode, write cwd-local files so hooks can resolve:
  - daemon port
  - daemon session id

**Step 3: Update hook client routing**

- If a session id file exists:
  - call daemon session hook routes
- Otherwise:
  - keep the existing local sidecar route behavior

### Task 6: Verify and document

**Files:**
- Modify: `README.md`

**Step 1: Run focused tests**

- `cargo test daemon -- --nocapture`
- `cargo test protocol::tools::tests::... -- --nocapture`
- `cargo test --test sidecar_integration -- --nocapture`

**Step 2: Run full suite**

- `cargo test`

**Step 3: Update docs**

- Document:
  - daemon-backed MCP proxying
  - shared project/session behavior
  - current limitation if any hook path still remains transitional
