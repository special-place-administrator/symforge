![SymForge](./symforge-banner_02.png)

A code-native MCP server that gives AI coding agents structured, symbol-aware navigation across your codebase. Built in Rust with tree-sitter, it replaces raw file scanning with tools that understand code as symbols, references, dependency graphs, and git history through a single MCP connection.

Works with MCP-compatible clients including Claude Code, Claude Desktop, Codex, Gemini CLI, VS Code MCP, Kilo Code, Roo Code, Cline, Continue, JetBrains plugins, and custom agents.

> [!IMPORTANT]
> **Rust-native** · **30 tools** · **19 source languages** · **5 config formats** · **6 prompts** · **Built-in resources**
>
> **Use SymForge first** for source-code reads, search, repo orientation, symbol tracing, and structural edits.
> **Use raw file reads** for docs and config when exact wording is the point.
> **Use shell tools** for builds, tests, package managers, Docker, and general system tasks.

## What's new in v7.4

- **ast-grep structural search** — `search_text` now supports `structural=true` for AST-pattern matching. Use `$VAR` for single-node metavariables and `$$$` for multi-node wildcards (e.g., `fn $NAME($$$) { $$$ }`). Powered by ast-grep-core with full tree-sitter integration across all 19 languages.
- **Adaptive detail levels** — Tools with `max_tokens` auto-cascade verbosity from full to compact to signature to summary to stay within budget. No more truncated output — responses degrade gracefully.
- **Lock-free concurrent reads** — Replaced `RwLock` with `ArcSwap` for the shared index handle. Zero reader contention under concurrent tool calls.
- **Per-result confidence scores** — Search and navigation tools now report confidence (high/medium/low) on each result based on match quality, caller count, and churn.
- **Token budget enforcement** — 11 search/navigation tools accept `max_tokens` and truncate at line boundaries when exceeded.
- **MCP tool annotations** — All tools now declare `readOnlyHint` and `openWorldHint` per the MCP spec, enabling smarter client-side tool selection.
- **Claude Desktop support** — `symforge init --client claude-desktop` registers the MCP server in Claude Desktop's config. On Windows, generates a `.cmd` wrapper to avoid the System32 CWD issue.
- **Smarter PreToolUse hooks** — When the SymForge sidecar is already running, tool-preference hints are suppressed to reduce noise for agents that are actively using SymForge.
- **Release profile optimization** (v7.4.2) — LTO, single codegen unit, and symbol stripping reduce the release binary by ~3%. Cross-crate inlining across 330 packages improves runtime performance.
- **Aho-Corasick multi-term search** (v7.4.2) — Multi-term OR searches in `search_text` now use a single-pass Aho-Corasick automaton instead of sequential substring matching, eliminating per-line allocations for case-insensitive queries.

## When to use SymForge

Use SymForge when an agent needs to:

- understand a repo without reading large files blindly
- find symbols, call sites, dependencies, and changed code
- search code by AST structure instead of text patterns
- edit code structurally by symbol instead of by raw text
- reindex and inspect impact after edits

Do not expect SymForge to replace normal shell workflows for process execution, runtime debugging, package management, or OS-level tasks.

## Install

**Prerequisite:** Node.js 18+

**Prebuilt binaries:** Windows x64, Linux x64, macOS arm64, macOS x64

```bash
npm install -g symforge
```

This installs the npm wrapper and downloads the platform binary to `~/.symforge/bin/symforge` (or `symforge.exe` on Windows). Set `SYMFORGE_HOME` to override the default home directory.

### Auto-configured clients

During global install, SymForge auto-configures these home-scoped clients if their home directories already exist:

- Claude Code
- Claude Desktop
- Codex
- Gemini CLI

Kilo Code is workspace-local:

```bash
symforge init --client kilo-code
```

Run that from the target project directory. It writes `.kilocode/mcp.json`, `.kilocode/rules/symforge.md`, and `.symforge/` in that workspace.

### Re-run setup manually

```bash
symforge init
symforge init --client claude
symforge init --client claude-desktop
symforge init --client codex
symforge init --client gemini
symforge init --client kilo-code
symforge init --client all
```

After setup, confirm in your client that the SymForge MCP server is connected or ready.

## Tool reference

### Orientation and context

| Tool | Purpose |
|------|---------|
| `health` | Index status, file/symbol counts, watcher state, parse diagnostics |
| `get_repo_map` | Structured overview of the entire repository (auto-adapts detail to token budget) |
| `explore` | Concept-driven exploration with stemmed matching and convention enrichment |
| `ask` | Natural language questions routed to the right tool internally |
| `conventions` | Auto-detect project coding patterns |
| `context_inventory` | See what symbols and files you've already fetched this session |
| `investigation_suggest` | Find gaps in your loaded context |

### Reading code

| Tool | Purpose |
|------|---------|
| `get_file_context` | File outline, imports, consumers — call before reading a source file |
| `get_file_content` | Exact raw text with optional line ranges — for docs, config, or when you need the literal source |
| `get_symbol` | Full source of a function, struct, class, etc. by name (batch mode supported) |
| `get_symbol_context` | Symbol body + callers + callees + type dependencies (supports bundle mode for edit prep) |

### Searching

| Tool | Purpose |
|------|---------|
| `search_symbols` | Find symbols by name, kind, language, path prefix |
| `search_text` | Full-text search with enclosing symbol context. Supports literal, OR-terms, regex, and structural AST patterns (`structural=true`) |
| `search_files` | Ranked file path discovery with co-change coupling |

### Tracing impact

| Tool | Purpose |
|------|---------|
| `find_references` | Call sites, imports, type usages, implementations |
| `find_dependents` | File-level dependency graph |
| `trace_symbol` | Multi-hop caller/callee chains for a symbol |
| `what_changed` | Files changed since a timestamp, ref, or uncommitted |
| `diff_symbols` | Symbol-level diff between git refs (AST-based for supported languages) |
| `analyze_file_impact` | Re-index a file after editing and report affected dependents |
| `inspect_match` | Deep-dive a search match with full symbol context |

### Editing code

| Tool | Purpose |
|------|---------|
| `edit_plan` | Analyze impact and suggest the right edit tool sequence |
| `replace_symbol_body` | Replace a symbol's entire definition by name |
| `edit_within_symbol` | Scoped find-and-replace within a symbol's range |
| `insert_symbol` | Insert code before or after a named symbol |
| `delete_symbol` | Remove a symbol and its doc comments by name |
| `batch_edit` | Multiple symbol-addressed edits atomically across files |
| `batch_insert` | Insert code before/after multiple symbols across files |
| `batch_rename` | Rename a symbol and update all references project-wide |

### Validation and indexing

| Tool | Purpose |
|------|---------|
| `validate_file_syntax` | Parse diagnostics with line/column location for code and config files |
| `index_folder` | Full reindex of a directory |

### Structural search examples

With `structural=true`, the `search_text` tool uses [ast-grep](https://ast-grep.github.io/) pattern syntax to match code by AST structure rather than text:

```
# Find all functions in Rust
search_text(query="fn $NAME($$$) { $$$ }", structural=true, language="Rust")

# Find all React useState hooks
search_text(query="const [$STATE, $SETTER] = useState($$$)", structural=true, language="TypeScript")

# Find all try-catch blocks in Java
search_text(query="try { $$$ } catch ($E) { $$$ }", structural=true, language="Java")
```

Metavariable syntax: `$NAME` matches a single AST node, `$$$` matches zero or more nodes. Captures are shown in results.

### Practical defaults

- Call `get_file_context` before reading a source file
- Use `search_text` or `search_symbols` before broad grep or raw file scans
- Use `structural=true` when you need pattern matching that respects code structure (ignores comments, whitespace, formatting)
- Use `get_file_content` when exact docs/config text matters
- Run `analyze_file_impact` after small edits; `index_folder` after larger multi-file work
- `edit_plan` accepts a bare symbol, a file path, or `path::symbol`
- `batch_edit` and `batch_insert` accept shorthand strings like `src/lib.rs::helper => delete`
- Use `max_tokens` on any search/navigation tool to control response size — output adapts verbosity automatically

## Agent setup prompt

If your AI agent still falls back to built-in file reads, grep, or text-based edits after SymForge is installed, give it the setup prompt from the wiki:

**[Agent Setup Prompt](https://github.com/special-place-administrator/symforge/wiki/Agent-Setup-Prompt)**

This prompt detects installed clients, configures SymForge for each, updates instruction files, and validates the setup.

## Architecture

SymForge is organized around a tree-sitter index, a set of query layers over that index, and the MCP tool surface. For the full runtime and module map, see [Architecture and How It Works](https://github.com/special-place-administrator/symforge/wiki/Architecture-and-How-It-Works) in the wiki.

### Extension points

Two trait-based registries let feature code plug into the shared edit and ranker paths without amending the handlers themselves.

**`EditHook`** wraps the per-edit lifecycle for the seven edit tools (`replace_symbol_body`, `edit_within_symbol`, `insert_symbol`, `delete_symbol`, `batch_edit`, `batch_insert`, `batch_rename`). Implementations register at startup; the handlers delegate to the registry to resolve the target path before writing and to run bookkeeping after the edit commits. For example, a worktree-aware feature registers a hook that rewrites a symbol's indexed path onto the active worktree before the write lands.

Each of the seven edit tools accepts an optional `working_directory` parameter pointing at a `git worktree` sibling of the indexed repo. When supplied, SymForge reroutes the write into that worktree and includes `rerouted: true`, `wrote_to:`, and `indexed_path:` lines in the response so callers can verify the target. Set `SYMFORGE_WORKTREE_AWARE=1` to enable this routing. Example:

```json
{
  "path": "src/lib.rs",
  "name": "hello",
  "new_body": "fn hello() { println!(\"hi\"); }",
  "working_directory": "/abs/path/to/sibling/worktree"
}
```

**`RankSignal`** wraps `search_files` scoring contributions. Each signal carries a name, a weight, and a `score()` function, and the ranker combines registered signals into a weighted sum. The current path-match and co-change signals ship as default registrations; additional signals — frecency, for example — register at startup and join the fusion without touching the handler or the other signals.

See [ADR 0012](docs/decisions/0012-edit-and-ranker-hook-architecture.md) for the rationale and the feature plug-in pattern.

## Operational notes

- `symforge daemon` is optional if you want a shared index across multiple terminal sessions.
- Index snapshots persist at `.symforge/index.bin` for fast restarts.
- Use `validate_file_syntax` when a config file may be malformed — it reports tree-sitter parse diagnostics with line and column locations.
- PreToolUse hooks auto-suppress when the sidecar is active — no redundant "use SymForge" hints when you're already using it.

## Environment variables

| Variable | Default | Effect |
|----------|---------|--------|
| `SYMFORGE_HOME` | `~/.symforge` | Home directory for the binary and daemon metadata |
| `SYMFORGE_AUTO_INDEX` | `true` | Enables project discovery and startup indexing |
| `SYMFORGE_HOOK_VERBOSE` | unset | Set to `1` for stderr hook diagnostics |
| `SYMFORGE_CB_THRESHOLD` | `0.20` | Parse-failure circuit-breaker threshold |
| `SYMFORGE_RECONCILE_INTERVAL` | `30` | Watcher reconciliation interval in seconds; `0` disables periodic sweeps |
| `SYMFORGE_SIDECAR_BIND` | `127.0.0.1` | Sidecar bind host for local in-process mode |
| `SYMFORGE_DAEMON_BIND` | `127.0.0.1` | Daemon bind host for shared local daemon |

For platform-specific setup scripts (PowerShell, CMD, bash, zsh), see the wiki:

**[Environment Setup Scripts](https://github.com/special-place-administrator/symforge/wiki/Environment-Setup-Scripts)**

## Deeper reference

- [SymForge Wiki Home](https://github.com/special-place-administrator/symforge/wiki)
- [Architecture and How It Works](https://github.com/special-place-administrator/symforge/wiki/Architecture-and-How-It-Works)
- [Tool Reference](https://github.com/special-place-administrator/symforge/wiki/Tool-Reference)
- [Runtime Model](https://github.com/special-place-administrator/symforge/wiki/Runtime-Model)
- [Supported Languages and Config Formats](https://github.com/special-place-administrator/symforge/wiki/Supported-Languages-and-Config-Formats)
- [Benchmarks and Token Savings](https://github.com/special-place-administrator/symforge/wiki/Benchmarks-and-Token-Savings)

## Build from source

```bash
cargo build --release
cargo test --all-targets -- --test-threads=1
```

The release profile enables LTO and single codegen unit for smaller binaries and better cross-crate optimization. Release builds take longer (~4 min) than dev builds (~15 sec). The Cargo package name is `symforge`.

## License

SymForge is licensed under [PolyForm Noncommercial License 1.0.0](./LICENSE). The official license text is also available from the [PolyForm Project](https://polyformproject.org/licenses/noncommercial/1.0.0/).

You may inspect, study, and use the source code for noncommercial purposes, but commercial use is prohibited unless separately licensed.
