# Tokenizor MCP

In-memory code intelligence for Claude Code and Codex. Tokenizor keeps your project indexed in RAM, exposes a standard MCP server for both clients, and adds transparent hook-based context enrichment for Claude Code.

## What It Does

Tokenizor runs as an MCP server alongside Claude Code, Codex, and other MCP clients. On startup it indexes your project into an in-memory LiveIndex.

For **Claude Code**, Tokenizor can also install hooks, so:

- **Read hook** — injects a symbol outline and key references for the file you just read
- **Edit hook** — re-indexes the file and shows callers that may need review (impact analysis)
- **Write hook** — indexes the new file immediately
- **Grep hook** — adds symbol context to matched lines
- **SessionStart hook** — injects a compact repo map (~500 tokens)
- **UserPromptSubmit hook** — refreshes context from file, symbol, or repo-map hints in your prompt before Claude answers

All Claude enrichment happens in <100ms via an HTTP sidecar that shares memory with the MCP server. The model never needs to call a special tool — it gets richer context for free.

For **Codex**, Tokenizor installs the MCP server entry in `~/.codex/config.toml`, tunes the server timeouts for heavier code-intelligence calls, and adds Tokenizor guidance to `~/.codex/AGENTS.md`. It also teaches Codex to fall back to project `CLAUDE.md` files when `AGENTS.md` is absent. Tokenizor does **not** install transparent post-tool/session-start enrichment for Codex, because the current Codex CLI help and OpenAI Codex docs document MCP server registration but do not document a Claude-style hook/session enrichment mechanism.

To close that gap honestly, Tokenizor now exposes the same underlying context through standard MCP tools, resources, and prompts:
- `get_repo_map`
- `get_file_context`
- `get_symbol_context`
- `analyze_file_impact`
- `tokenizor://repo/*`, `tokenizor://file/*`, and `tokenizor://symbol/*` resources
- `code-review`, `architecture-map`, and `failure-triage` MCP prompts

## Installation

**Prerequisite:** Node.js 18+. No Rust toolchain needed.

Prebuilt binaries: **Windows x64**, **Linux x64**, **macOS ARM64**, **macOS x64**.

### Initialize Clients

Install once, then initialize the clients you want.

**Step 1 — Install globally**

```bash
npm install -g tokenizor-mcp
```

> **Do NOT use `npx`.** The init step writes the binary's absolute path into your Claude Code config. `npx` runs from a temporary cache directory that gets cleaned up, which silently breaks hooks. A global install gives a stable path.

**Step 2 — Initialize**

```bash
tokenizor-mcp init
```

`tokenizor-mcp init` now defaults to `--client all`.

Available targets:

```bash
tokenizor-mcp init --client claude
tokenizor-mcp init --client codex
tokenizor-mcp init --client all
```

### Claude Code

`tokenizor-mcp init` or `tokenizor-mcp init --client claude`:

- Registers the MCP server in `~/.claude.json`
- Installs PostToolUse, SessionStart, and UserPromptSubmit hooks into `~/.claude/settings.json`
- Appends a bounded Tokenizor guidance block to `~/.claude/CLAUDE.md`

**Optional — Auto-approve tools**

All tokenizor tools are read-only or local indexing. To skip approval prompts, add to `~/.claude/settings.json` or your project's `.claude/settings.json`:

```json
{
  "permissions": {
    "allow": ["mcp__tokenizor__*"]
  }
}
```

**Verify it works:**

Start a new Claude Code session in any git repo. You should see:
- `tokenizor` shows as connected in `/mcp`
- When you read a file, extra symbol context appears after the file contents

If hooks aren't firing, run `tokenizor-mcp init --client claude` again.

### Codex

`tokenizor-mcp init` or `tokenizor-mcp init --client codex`:

- Registers the MCP server in `~/.codex/config.toml` under `[mcp_servers.tokenizor]`
- Sets `startup_timeout_sec = 30` and `tool_timeout_sec = 120` for Tokenizor
- Merges `CLAUDE.md` into `project_doc_fallback_filenames` so Codex can reuse Claude-oriented project docs when needed
- Appends a bounded Tokenizor guidance block to `~/.codex/AGENTS.md`
- Preserves unrelated Codex config
- Uses the absolute native binary path, so `codex mcp list` and `codex mcp get tokenizor` see the same entry Codex would write itself

Verify it works:

```bash
codex mcp list
codex mcp get tokenizor
```

Expected result:
- `tokenizor` shows as `enabled`
- `codex mcp get tokenizor` shows the installed binary path

Current limitation:
- Codex gets the same shared context through explicit MCP tools, resources, prompts, and AGENTS guidance
- Claude-only transparent hook enrichment remains unavailable in Codex until OpenAI documents an equivalent supported integration point

### Shared Daemon

Tokenizor now has a local daemon entry point:

```bash
tokenizor-mcp daemon
```

What it does today:
- Tracks shared project instances by canonical project root
- Tracks multiple client sessions per project across concurrent terminals and MCP clients
- Owns the authoritative project runtime for daemon-backed sessions:
  - one shared LiveIndex per project
  - one shared watcher per project
  - shared hook token stats and symbol cache per project
- Proxies stdio MCP tool calls through session-scoped daemon routes, so Claude, Codex, and other stdio MCP clients in the same project hit the same backend instance
- Routes Claude hook traffic through session-scoped daemon endpoints when the stdio session is daemon-backed
- Exposes local HTTP daemon endpoints for project health and session inspection
- Uses stable, URL-safe project ids derived from the canonical root

How it behaves:
- Starting a stdio MCP session in a project now prefers the shared daemon automatically
- If no daemon is running, Tokenizor starts one locally and opens a project session
- Multiple terminals in the same project share one project instance instead of building duplicate indexes
- Different projects stay isolated by canonical project root

Current limitations:
- The daemon is intentionally local loopback-only for same-machine CLI clients
- Claude hook enrichment still depends on Claude's documented hook system; Codex uses the same backend intelligence through explicit MCP tools instead of transparent hook/session enrichment
- Prompt and resource reads share the same daemon-backed runtime as tools, so all connected clients stay project/session scoped on one backend instance

### Cursor

Add to `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "tokenizor": {
      "command": "npx",
      "args": ["-y", "tokenizor-mcp"]
    }
  }
}
```

On Windows, use `"command": "cmd"` and `"args": ["/c", "npx", "-y", "tokenizor-mcp"]`.

`npx` is fine for Cursor — MCP servers are launched fresh each session, so there are no persisted hook paths to break.

### Other MCP clients

Standard stdio MCP server:
- **Command:** `tokenizor-mcp` (if installed globally) or `npx -y tokenizor-mcp`
- No environment variables required
- Auto-indexes on startup when `.git` is present in the working directory

### Updating

```bash
npm update -g tokenizor-mcp
tokenizor-mcp init          # re-run to refresh Claude and Codex config if the binary moved
```

### Uninstalling

```bash
npm uninstall -g tokenizor-mcp
```

Then remove the tokenizor entries from `~/.claude/settings.json` (any hook whose command contains `tokenizor`) and run `claude mcp remove tokenizor`.

For Codex, remove `[mcp_servers.tokenizor]` from `~/.codex/config.toml` or run:

```bash
codex mcp remove tokenizor
```

## Claude vs Codex

| Client | MCP tools | Transparent context enrichment |
|------|-------------|--------------------------------|
| Claude Code | Yes, plus MCP prompts/resources and CLAUDE.md guidance | Yes, via PostToolUse, SessionStart, and UserPromptSubmit hooks |
| Codex | Yes, plus MCP prompts/resources, AGENTS guidance, Codex timeout tuning, and `CLAUDE.md` fallback discovery | No documented Claude-style hook/session enrichment API found |

Migration note:
- If you already used `tokenizor-mcp init` for Claude, rerun `tokenizor-mcp init` once on the new build to add Codex automatically.
- If you previously added Tokenizor to Codex manually with `codex mcp add tokenizor -- ...`, rerunning `tokenizor-mcp init --client codex` is safe and idempotent.

## MCP Tools (18)

| Tool | Description |
|------|-------------|
| `health` | LiveIndex stats, watcher status, token savings |
| `index_folder` | Trigger full reload of the index |
| `get_file_outline` | Symbol list for a file |
| `get_repo_outline` | File list with coverage stats |
| `get_repo_map` | Compact repo map used for session-start style orientation |
| `get_file_context` | File outline plus key external references |
| `get_symbol_context` | Grouped references for a symbol with enclosing-symbol annotations |
| `analyze_file_impact` | Re-read a file from disk, update the index, and report symbol impact |
| `get_file_tree` | Directory tree with symbol counts |
| `get_symbol` | Lookup symbol by file + name |
| `get_symbols` | Batch lookup (symbols and code slices) |
| `get_file_content` | Serve file from memory with optional line range |
| `search_symbols` | Substring search with Exact > Prefix > Substring ranking and optional kind filter |
| `search_text` | Full-text search with literal, multi-term OR, and regex modes |
| `find_references` | All call sites for a symbol with context |
| `find_dependents` | Files that import or type-depend on a given file, including C#/Java namespace/package heuristics |
| `get_context_bundle` | Full context: symbol + callers + callees + type usages |
| `what_changed` | Files changed since a timestamp, relative to a git ref, or in the current uncommitted worktree |

### Query Examples

```json
{"query":"MinioService","kind":"class"}
```

```json
{"terms":["TODO","FIXME","HACK"]}
```

```json
{"query":"TODO|FIXME|HACK","regex":true}
```

```json
{}
```

```json
{"git_ref":"HEAD~5"}
```

Notes:
- `search_symbols` kind filters use the displayed symbol kinds such as `fn`, `class`, `struct`, or `interface`.
- `search_text` with `terms` uses OR semantics across all provided terms.
- `what_changed` with `{}` defaults to uncommitted git changes when Tokenizor knows the repo root.
- If git-based change detection is unavailable, pass `{"since": 1700000000}` to use timestamp mode explicitly.

## MCP Prompts (3)

| Prompt | Description |
|------|-------------|
| `code-review` | Reviews a target path with Tokenizor repo health, repo map, and optional file context resources attached |
| `architecture-map` | Starts architecture exploration with repo outline and repo map resources |
| `failure-triage` | Starts debugging/triage with repo health, repo changes, and optional file context resources |

## MCP Resources

Static resources:
- `tokenizor://repo/health`
- `tokenizor://repo/outline`
- `tokenizor://repo/map`
- `tokenizor://repo/changes/uncommitted`

Resource templates:
- `tokenizor://file/context?path={path}&max_tokens={max_tokens}`
- `tokenizor://file/content?path={path}&start_line={start_line}&end_line={end_line}`
- `tokenizor://symbol/detail?path={path}&name={name}&kind={kind}`
- `tokenizor://symbol/context?name={name}&file={file}`

## Languages (16)

Full symbol extraction + cross-references:

Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, PHP, Swift, Perl, Kotlin, Dart, Elixir

## How It Works

```
┌─────────────┐     stdio      ┌──────────────────┐
│ Claude Code  │◄──────────────►│  MCP Server      │
│             │                │  (18 tools +      │
│             │                │   prompts/resources)│
│  Read file  │                │       │           │
│      │      │                │  ┌────▼────┐      │
│      ▼      │   HTTP <100ms  │  │LiveIndex│      │
│ PostToolUse ├───────────────►│  │  (RAM)  │      │
│   hook      │                │  └────┬────┘      │
│      │      │                │       │           │
│      ▼      │                │  ┌────▼────┐      │
│ +context    │                │  │ Watcher │      │
│ injected    │                │  │ (notify)│      │
└─────────────┘                └──┴─────────┴──────┘
```

1. **Startup**: LiveIndex loads all source files into RAM using tree-sitter parsing. If a serialized snapshot exists, loads from disk in <100ms instead of re-parsing.
2. **File watcher**: notify crate detects changes within 200ms. Content-hash skip prevents redundant reparse.
3. **MCP tools**: Query the LiveIndex with O(1) lookups. All responses are compact human-readable text.
4. **HTTP sidecar**: axum server on ephemeral port, shares `Arc<LiveIndex>` with MCP tools.
5. **Hooks**: Rust binary reads stdin JSON, calls sidecar over sync HTTP (<50ms), returns enrichment as `additionalContext`.
6. **Persistence**: On shutdown, serializes index to disk via postcard. On restart, loads snapshot and verifies in background.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `TOKENIZOR_AUTO_INDEX` | `true` | Auto-index on startup when .git exists |
| `TOKENIZOR_CB_THRESHOLD` | `20` | Circuit breaker: abort if >N% of files fail parsing |
| `TOKENIZOR_SIDECAR_BIND` | `127.0.0.1` | Sidecar bind address |

## Building from Source

Requires [Rust toolchain](https://rustup.rs) (edition 2024).

```bash
cargo build --release
cargo test
```

## Tech Stack

- **Rust** (edition 2024) — core engine
- **tree-sitter** 0.26 — parsing and cross-reference extraction for 16 languages
- **rmcp** 1.1 — MCP protocol over stdio
- **tokio** — async runtime
- **axum** 0.8 — HTTP sidecar
- **notify** 8 — file watching with debouncing
- **postcard** 1.1 — index serialization (safe, no RUSTSEC advisories)
- **std RwLock + HashMap** — concurrent LiveIndex (via `Arc<RwLock<LiveIndex>>`)

## License

MIT
