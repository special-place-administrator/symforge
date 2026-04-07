![SymForge](./symforge-banner_02.png)

A code-native MCP server that gives AI coding agents structured, symbol language-aware navigation across your codebase. Built in Rust with tree-sitter, it replaces raw file scanning with tools that understand code as symbols, references, dependency graphs, and git history through a single MCP connection.

Works with MCP-compatible clients including Claude Code, Codex, Gemini CLI, VS Code MCP, Kilo Code, Roo Code, Cline, Continue, JetBrains plugins, and custom agents.

> [!IMPORTANT]
> **Rust-native** · **31 tools** · **19 source languages** · **5 config formats** · **6 prompts** · **Built-in resources**
>
> **Use SymForge first** for source-code reads, search, repo orientation, symbol tracing, and structural edits.
> **Use raw file reads** for docs and config when exact wording is the point.
> **Use shell tools** for builds, tests, package managers, Docker, and general system tasks.

## When to use SymForge

Use SymForge when an agent needs to:

- understand a repo without reading large files blindly
- find symbols, call sites, dependencies, and changed code
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
| `get_repo_map` | Structured overview of the entire repository |
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
| `get_symbol_context` | Symbol body + callers + callees + type dependencies |

### Searching

| Tool | Purpose |
|------|---------|
| `search_symbols` | Find symbols by name, kind, language, path prefix |
| `search_text` | Full-text search with enclosing symbol context |
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

### Practical defaults

- Call `get_file_context` before reading a source file
- Use `search_text` or `search_symbols` before broad grep or raw file scans
- Use `get_file_content` when exact docs/config text matters
- Run `analyze_file_impact` after small edits; `index_folder` after larger multi-file work
- `edit_plan` accepts a bare symbol, a file path, or `path::symbol`
- `batch_edit` and `batch_insert` accept shorthand strings like `src/lib.rs::helper => delete`

## Agent setup prompt

If your AI agent still falls back to built-in file reads, grep, or text-based edits after SymForge is installed, give it the setup prompt from the wiki:

**[Agent Setup Prompt](https://github.com/special-place-administrator/symforge/wiki/Agent-Setup-Prompt)**

This prompt detects installed clients, configures SymForge for each, updates instruction files, and validates the setup.

## Operational notes

- `symforge daemon` is optional if you want a shared index across multiple terminal sessions.
- Index snapshots persist at `.symforge/index.bin` for fast restarts.
- Use `validate_file_syntax` when a config file may be malformed — it now reports tree-sitter parse diagnostics with line and column locations.

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
cargo test
```

The Cargo package name is `symforge`.

## License

SymForge is licensed under [PolyForm Noncommercial License 1.0.0](./LICENSE). The official license text is also available from the [PolyForm Project](https://polyformproject.org/licenses/noncommercial/1.0.0/).

You may inspect, study, and use the source code for noncommercial purposes, but commercial use is prohibited unless separately licensed.
