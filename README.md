![SymForge](./symforge-banner.png)

A code-native MCP server that gives AI coding agents structured, symbol-aware access to codebases. Built in Rust with tree-sitter, it replaces raw file scanning with tools that understand code as symbols, references, dependency graphs, and git history — through a single MCP connection.

Works with any MCP-compatible client — CLI agents (Claude Code, Codex, Gemini CLI), VS Code extensions (Kilo Code, Roo Code, Cline, Continue), JetBrains plugins, and custom agents.

> [!IMPORTANT]
> **Rust-native** ◆ **25 tools** ◆ **19 source languages** ◆ **5 config formats** ◆ **Built-in prompts and resources**
>
> **SymForge First** ◆ for source-code reads, search, repo orientation, and symbol tracing.
> **Literal raw reads are still correct** ◇ for docs and config when exact wording is the point.
> **Kilo Code is workspace-local** ◆ and should be initialized from the project directory.

## Why SymForge

AI coding agents spend most of their token budget on orientation — reading files, grepping for patterns, figuring out what code is where. SymForge replaces that with structured tools that resolve symbols, references, and dependencies server-side.

- **Fewer tool calls** — one `get_symbol_context(bundle=true)` returns a symbol's body plus all referenced type definitions, resolved recursively. That's one call instead of reading 3-5 files sequentially.
- **Lower token cost** — structured responses strip boilerplate, returning only what the agent needs. Measured savings below.
- **Better accuracy** — symbol-aware search finds the right code faster than text matching
- **Git intelligence** — churn scores, ownership, and co-change coupling inform which files matter most
- **Server-side edits** — edit tools modify code by symbol name. The agent sends a name and replacement body; the server resolves byte positions, splices, writes atomically, and re-indexes.

## Workflow Ownership

SymForge is intended to be the primary path for semantic code-inspection workflows:

- **Source-code read and orientation** — file outlines, symbol-aware context, and repo-start overview
- **Source-code search** — symbol lookup, text search with enclosing context, and dependency tracing
- **Prompt-context enrichment** — file, symbol, and path-hint resolution for focused context injection
- **Post-edit impact** — reindexing, affected symbol reporting, and follow-on caller/dependency guidance
- **Code-review inspection** — changed-file and changed-symbol inspection backed by the index

Shell and raw tools are still the right default for non-semantic workflows:

- **Process control and execution** — builds, tests, package managers, Docker, process inspection
- **Literal document/config reads** — when exact wording in docs or config files is the point
- **General system tasks** — filesystem manipulation, environment checks, OS-level diagnostics

SymForge is intentionally moving toward RTK-style path-of-least-resistance adoption for the workflows above, but it is **not** trying to become a generic shell-output summarizer. The product boundary is: let SymForge own semantic code understanding, and let the shell own process/runtime/system work.

## How It Works

SymForge maintains a live index of every file in your project. On startup, it parses all source files using tree-sitter grammars (19 source languages), config files using native Rust parsers (5 formats), and extracts symbols (functions, classes, structs, selectors, variables, keys, etc.), their byte ranges, and cross-references between them. This index stays current via a file watcher that re-indexes changed files with debouncing.

**Why this is efficient for LLMs:**

Traditional agent workflows look like this:
```
Agent: read file A (4000 tokens)      → finds import
Agent: read file B (6000 tokens)      → finds function signature
Agent: read file C (3000 tokens)      → finds type definition
Agent: grep for callers (2000 tokens) → finds 3 call sites
Total: 4 tool calls, ~15,000 tokens consumed
```

With SymForge:
```
Agent: get_symbol_context(name="handler", bundle=true)
Server: resolves symbol + all referenced types from the index
Agent receives: symbol body + type definitions (~800 tokens)
Total: 1 tool call, ~800 tokens consumed
```

The server does the graph traversal, the agent gets a focused answer. The index lookup is O(1) — no file I/O needed for symbol resolution.

**Key architectural decisions:**
- **Symbol-addressed operations** — tools accept symbol names, not file content. The server resolves names to byte ranges via the index, eliminating the need for agents to track positions.
- **Tree-sitter parsing** — deterministic, incremental parsing across 19 source languages plus native parsers for 5 config formats. Each symbol gets a byte range, line range, and an attached doc comment range.
- **Persistent snapshots** — the index serializes to `.symforge/index.bin` for fast restarts (~88ms for a 326-file project).
- **Daemon mode** — multiple terminal sessions share one index via a local loopback daemon. No redundant re-indexing.
- **Non-poisoning locks** — all shared state uses `parking_lot::RwLock`, which never poisons. A panicked thread releases its lock instead of crashing the daemon.
- **Panic-safe index mutations** — all index write operations (`update_file`, `add_file`, `remove_file`) are wrapped in `catch_unwind`. If any dependency or code path panics mid-mutation, the index auto-repairs auxiliary indices (reverse refs, trigram, path lookups) from the always-consistent primary store. Files never vanish from the index, even under concurrent agent stress.
- **Insert-first ordering** — new file data is written to the primary store before auxiliary indices are updated. This guarantees the file is always discoverable even if auxiliary index updates are interrupted.

## Token Savings — Measured

Every applicable tool response includes a footer showing estimated tokens saved compared to reading the raw file. These are real measurements from SymForge's own codebase (~159 files, ~7500 symbols):

> [!TIP]
> Real session wins on SymForge's own codebase reached **99.7% peak savings** ◆ and a **largest single win of ~66,800 tokens** ◆
> The `health` tool reports both **token savings** ◇ and **owned-workflow adoption** ◇ for the current session.

| Operation | Raw file approach | SymForge | Savings |
|-----------|------------------|-----------|---------|
| Understand a 5700-line file's structure | `cat` the file: ~67,000 tokens | `get_file_context(sections=['outline'])`: ~200 tokens | **~66,800 tokens saved (99.7%)** |
| Read a function + all its type dependencies | Read 3-5 files: ~15,000 tokens | `get_symbol_context(bundle=true)`: ~800 tokens | **~64,000 tokens saved (98.8%)** |
| Understand a 1600-line file's structure | `cat` the file: ~16,000 tokens | `get_file_context(sections=['outline'])`: ~800 tokens | **~15,200 tokens saved (95%)** |
| Find callers of a function | grep + read enclosing functions: ~5,000 tokens | `get_symbol_context`: ~700 tokens | **~15,300 tokens saved (96%)** |
| Edit a function by name | Read file, find position, send full content: ~5,000 tokens | `replace_symbol_body(name=..., new_body=...)`: ~200 tokens | **~4,800 tokens saved (96%)** |
| Explore a concept | grep + read results + follow imports: ~10,000 tokens | `explore(query=..., depth=2)`: ~1,500 tokens | **~8,500 tokens saved (85%)** |

Savings scale with file size. On large files (5000+ lines), `get_file_context` routinely saves 50,000-70,000 tokens per call. Over a coding session, cumulative savings typically reach 200,000-400,000 tokens.

Token savings and owned-workflow hook adoption are tracked per session and reported by the `health` tool. Skeptical? Run a session with SymForge, check `health`, then try the same tasks with raw file reads and compare. The numbers speak for themselves on any codebase.

## Tools

25 unique tools + 7 backward-compatible aliases, organized by workflow stage. Edit tools accept symbol names — no need to read files first.

### Orientation

| Tool | Purpose |
|------|---------|
| `health` | Index status, file counts, load time, watcher state, session token savings, hook adoption metrics, git temporal status |
| `get_repo_map` | Start here. Adjustable detail: compact overview (~500 tokens), `detail='full'` for complete symbol outline, `detail='tree'` for browsable file tree with symbol counts |
| `explore` | Concept-driven exploration — "how does authentication work?" returns related symbols, patterns, and files. Multi-term queries score symbols by how many terms match. Set `depth=2` for signatures and dependents, `depth=3` for implementations and type chains. Vendor/generated files hidden by default; set `include_noise=true` to include |

### Reading Code

| Tool | Purpose |
|------|---------|
| `get_file_content` | Read files with line ranges, `around_line`, `around_match`, `around_symbol`, or chunked paging |
| `get_file_context` | Rich file summary: symbol outline, imports, consumers, references, git activity. Use `sections=['outline']` for symbol-only outline |
| `get_symbol` | Look up symbol(s) by file path and name. Single mode or batch mode with `targets[]` array for multiple symbols or byte-range code slices |
| `get_symbol_context` | Three modes: (1) Default — definition + callers + callees + type usages. (2) `bundle=true` — symbol body + all referenced type definitions, resolved recursively. (3) `sections=[...]` — trace analysis with dependents, siblings, implementations, git activity. Supports `verbosity` levels (`signature`, `compact`, `full`) |

### Searching

| Tool | Purpose |
|------|---------|
| `search_symbols` | Find symbols by name, filtered by kind/language/path/scope. Auto-disambiguates cross-kind matches (e.g., C# class vs constructor) using kind-tier priority. Test and generated files hidden by default (`include_tests`, `include_generated`); symbols inside inline `mod tests` blocks are also filtered |
| `search_text` | Full-text search with enclosing symbol context, `group_by` modes, `follow_refs` for inline callers. Set `ranked=true` for semantic re-ranking by caller connectivity, git churn, and symbol kind. Test/generated noise hidden by default (`include_tests`, `include_generated`). Auto-corrects double-escaped regex patterns common in LLM tool calls |
| `search_files` | Ranked file path discovery. `changed_with=path` for git co-change coupling. `resolve=true` for exact path resolution from partial hints |

### References and Dependencies

| Tool | Purpose |
|------|---------|
| `find_references` | Two modes: (1) Default — call sites, imports, type usages grouped by file. (2) `mode='implementations'` — trait/interface implementors bidirectionally with `direction` control |
| `find_dependents` | File-level dependency graph — which files import the given file. Supports text, Mermaid, and Graphviz output with true reference counts per file. Set `compact=true` for 60-75% smaller output |
| `inspect_match` | Deep-dive a `search_text` match — full symbol context with callers and type dependencies |

### Git Intelligence

| Tool | Purpose |
|------|---------|
| `what_changed` | Files changed since a timestamp, git ref, or uncommitted. Filter with `path_prefix`, `language`, or `code_only=true` to exclude non-source files |
| `analyze_file_impact` | Re-read a file from disk, update the index, report symbol-level impact. Set `include_co_changes=true` for git temporal coupling data |
| `diff_symbols` | Symbol-level diff between git refs — added, removed, and modified symbols per file. Filter by `language` or `path_prefix` |

### Validation

| Tool | Purpose |
|------|---------|
| `validate_file_syntax` | Parse and validate config files with exact diagnostics when available. Best for malformed TOML/JSON/YAML and other config reads where parser truth matters more than semantic summary |

### Editing — Single Symbol

| Tool | Purpose |
|------|---------|
| `replace_symbol_body` | Replace a symbol's entire definition by name. Includes attached doc comments. Auto-indents. Reports stale references on signature changes |
| `insert_symbol` | Insert code before or after a named symbol. Set `position='before'` or `'after'` (default). Inserts above doc comments when targeting a documented symbol. Auto-indented |
| `delete_symbol` | Remove a symbol and its attached doc comments entirely by name. Cleans up surrounding blank lines |
| `edit_within_symbol` | Find-and-replace scoped to a symbol's byte range (including doc comments) — won't affect code outside it |

### Editing — Batch Operations

| Tool | Purpose |
|------|---------|
| `batch_edit` | Apply multiple symbol-addressed edits atomically across files. All symbols validated before any writes. Overlap detection includes doc comment ranges |
| `batch_rename` | Rename a symbol and update all references project-wide — uses indexed references plus supplemental literal scan to catch path-qualified usages like `Type::new()` |
| `batch_insert` | Insert the same code before/after multiple symbols across files |

### Indexing

| Tool | Purpose |
|------|---------|
| `index_folder` | Reindex a directory from scratch. Use when switching projects |

## Edit Tools — How They Work

Edit tools accept **symbol names** instead of raw file content. The server resolves byte positions via the index, splices the new content, writes atomically (temp + rename), and re-indexes the file — all in one tool call.

```
Agent sends:  replace_symbol_body(path="src/auth.rs", name="validate_token", new_body="...")
Server does:  resolve symbol → splice bytes → atomic write → reindex → return summary
Agent gets:   "src/auth.rs — replaced fn `validate_token` (342 → 287 bytes)"
```

**Key behaviors:**
- **Doc comment awareness** — edit operations (replace, delete, insert_before, edit_within) include attached doc comments (`///`, `/** */`, `#`, etc.) in the operation range. Deleting a function also deletes its doc comments. Inserting before a documented function inserts above the doc comments.
- **Auto-indentation** — new code is indented to match the target symbol's context
- **Disambiguation** — when multiple symbols share a name across different kinds (e.g., C# class and constructor), the highest-priority kind wins automatically (class > module > function > other). Use `kind` and `symbol_line` for same-tier disambiguation
- **Stale warnings** — `replace_symbol_body` detects signature changes and lists affected callers
- **Atomic batches** — `batch_edit` validates all symbols before writing anything; overlapping edits are rejected

## Prompts

| Prompt | Purpose |
|--------|---------|
| `symforge-review` | Structured code review plan using SymForge context surfaces |
| `symforge-architecture` | Architecture mapping plan using repo-level context and cross-reference tools |
| `symforge-triage` | Debugging and failure-triage plan using health, changed files, and local context |

## Resources

Static resources:
- `symforge://repo/health`
- `symforge://repo/outline`
- `symforge://repo/map`
- `symforge://repo/changes/uncommitted`

Resource templates:
- `symforge://file/context?path={path}&max_tokens={max_tokens}`
- `symforge://file/content?path={path}&start_line={start_line}&end_line={end_line}&around_line={around_line}&around_match={around_match}&context_lines={context_lines}&show_line_numbers={show_line_numbers}&header={header}`
- `symforge://symbol/detail?path={path}&name={name}&kind={kind}`
- `symforge://symbol/context?name={name}&file={file}`

## Supported Languages

### Source Languages (19)

Tree-sitter extractors for 19 languages:

| Tier | Languages |
|------|-----------|
| Quality Focus | Rust, Python, JavaScript, TypeScript, Go |
| Broader | Java, C, C++, C#, Ruby, PHP, Swift, Kotlin, Dart, Perl, Elixir |
| Frontend Assets | HTML, CSS, SCSS |

Doc comment detection per language — `///`, `/** */`, `#`, `@doc` patterns are recognized and attached to their symbols during parsing.

**HTML/Angular templates** are parsed with `tree-sitter-html`. Angular-specific constructs (`@if`, `@for`, `@switch`, `@defer`, `@let`, template refs `#name`) are extracted via supplemental text scanning. AST-backed extraction covers elements, custom elements (tag contains `-`), and `<ng-template>`.

**CSS** extracts selectors, custom properties (`--var`), `@media`, and `@keyframes`. **SCSS** extends CSS with `$variable`, `@mixin`, and `@function` extraction; skips `@include`/`@use`/`@forward`.

### Config Formats (5)

Native Rust parsers for config files:

| Format | Extensions | Symbols extracted |
|--------|-----------|-------------------|
| JSON | `.json` | Nested key paths (dot notation) |
| TOML | `.toml` | Table headers, key paths, array-of-tables |
| YAML | `.yaml`, `.yml` | Nested key paths |
| Markdown | `.md` | Section headers (dot-joined hierarchy) |
| Env | `.env` | Variable names |

Config files have capability-gated editing: JSON, TOML, and YAML support structural edits (`replace_symbol_body`); Markdown and Env support scoped text edits only (`edit_within_symbol`). HTML, CSS, and SCSS are gated at text-edit-safe — `edit_within_symbol` works, but `replace_symbol_body` is blocked until grammar accuracy is validated on real projects.

## Installation

**Prerequisite:** Node.js 18+

**Prebuilt binaries:** Windows x64, Linux x64, macOS arm64, macOS x64

> [!NOTE]
> `npm install -g symforge` auto-configures **Claude Code**, **Codex**, and **Gemini CLI** when their home directories already exist.
> **Kilo Code is different** ◆ run `symforge init --client kilo-code` from the workspace you want to configure.

```bash
npm install -g symforge
```

The installer downloads the platform binary to `~/.symforge/bin/`. Set `SYMFORGE_HOME` to override.

### What happens when you install

Running `npm install -g symforge` does the following automatically:

1. **Downloads the npm wrapper** from the registry
2. **Downloads your platform's pre-built binary** from GitHub releases and places it at `~/.symforge/bin/symforge[.exe]`
3. **Detects which home-scoped AI CLI tools you have installed** by checking for `~/.claude`, `~/.codex`, and `~/.gemini` directories
4. **Runs `symforge init` from your home directory** on the downloaded binary, which configures each detected home-scoped client:

   | Client | Files written |
   |--------|--------------|
   | **Claude Code** | `~/.claude.json` — MCP server entry with `alwaysAllow` for all 25 tools<br>`~/.claude/settings.json` — hook entries (PostToolUse, PreToolUse, SessionStart, UserPromptSubmit)<br>`~/.claude/CLAUDE.md` — SymForge guidance block (Decision Rules + Tooling Preference) |
   | **Codex** | `~/.codex/config.toml` — MCP server entry with timeouts and allowed tools<br>`~/.codex/AGENTS.md` — guidance block |
   | **Gemini CLI** | `~/.gemini/settings.json` — MCP server entry with `trust: true`<br>`~/.gemini/GEMINI.md` — guidance block |

Everything is idempotent — re-running install or `symforge init` is safe and updates configs to the latest format without duplicating entries or losing existing settings.

**Updates** work the same way — `npm install -g symforge` replaces the binary. During update, the installer stops running SymForge processes first so the binary can be replaced in place.

**Auto-init** runs after every install/update for home-scoped clients only: Claude Code, Codex, and Gemini CLI. Workspace-local clients such as Kilo Code are intentionally not auto-configured during global npm install because postinstall does not know which project workspace you want to modify.

For **Kilo Code**, run `symforge init --client kilo-code` from the project directory you want to configure. That writes `.kilocode/mcp.json`, `.kilocode/rules/symforge.md`, and creates `.symforge/` for workspace-local runtime state.

If your platform isn't listed, build from source instead.

## Client Setup

Claude Code, Codex, and Gemini CLI are auto-configured during global install when their home directories already exist. To re-run manually:

> [!IMPORTANT]
> **Auto-configured during global install** ◆ Claude Code, Codex, Gemini CLI
>
> **Workspace-local and manual by design** ◆ Kilo Code

```bash
symforge init                      # auto-detect clients
symforge init --client claude      # Claude Code only
symforge init --client codex       # Codex only
symforge init --client gemini      # Gemini CLI only
symforge init --client kilo-code   # Kilo Code in the current workspace
symforge init --client all         # all clients; workspace-local clients use the current directory
```

### Claude Code

Updates `~/.claude.json`, `~/.claude/settings.json`, `~/.claude/CLAUDE.md`. Installs MCP server registration, hook entries (`read`, `edit`, `write`, `grep`, `session-start`, `prompt-submit`), guidance block, and auto-allows all 25 SymForge tools.

### Codex

Updates `~/.codex/config.toml`, `~/.codex/AGENTS.md`. Installs MCP server config with timeouts, allowed tools list, and guidance block.

### Gemini CLI

Updates `~/.gemini/settings.json`, `~/.gemini/GEMINI.md`. Registers the MCP server as a stdio transport with `trust: true` (bypasses per-tool confirmation prompts) and a 120-second timeout. Writes a guidance block to `GEMINI.md` so Gemini knows to prefer SymForge tools for codebase navigation.

**Manual setup** (if auto-init didn't run or you need to reconfigure):

```bash
symforge init --client gemini
```

**Verify inside Gemini CLI:**

```
/mcp
```

You should see `symforge — Ready` with 25 tools listed. If the server shows `DISCONNECTED`, check that the binary exists at `~/.symforge/bin/symforge` (or `symforge.exe` on Windows).

### VS Code Extensions (Kilo Code, Roo Code, Cline, etc.)

Any VS Code extension with MCP support can use SymForge. For **Kilo Code**, run init manually from the workspace you want to configure:

```bash
symforge init --client kilo-code
```

This creates `.kilocode/mcp.json` and `.kilocode/rules/symforge.md` in your workspace, plus `.symforge/` for workspace-local runtime state. Global `npm install -g symforge` intentionally does not auto-write Kilo workspace files because npm postinstall runs from the package install directory, not from your project. For other extensions, configure manually through their MCP settings.

**Kilo Code** manual config (sidebar → gear icon → MCP Servers, or `.kilocode/mcp.json`):

```json
{
  "mcpServers": {
    "symforge": {
      "command": "C:\\Users\\<you>\\.symforge\\bin\\symforge.exe",
      "args": [],
      "disabled": false,
      "alwaysAllow": [
        "health", "get_repo_map", "explore", "get_file_content",
        "validate_file_syntax", "get_file_context", "get_symbol", "get_symbol_context",
        "search_symbols", "search_text", "search_files",
        "find_references", "find_dependents", "inspect_match",
        "what_changed", "analyze_file_impact", "diff_symbols",
        "index_folder", "replace_symbol_body", "edit_within_symbol",
        "insert_symbol", "delete_symbol", "batch_edit",
        "batch_rename", "batch_insert"
      ]
    }
  }
}
```

On macOS/Linux, use `~/.symforge/bin/symforge` (no `.exe`). The `alwaysAllow` list bypasses per-tool approval prompts.

**Kilo / Codex / Gemini schema errors:** if a strict provider shows an error like `Invalid schema for function ... array schema missing items`, the named function usually belongs to a different MCP server in the active client config, not SymForge. Strict providers reject the whole tool list if any loaded MCP server advertises invalid JSON Schema. In Kilo Code, check both workspace and global MCP settings for extra servers such as Claude hook bridges or flow servers, and temporarily disable the server named in the error.

Other VS Code extensions and MCP clients follow a similar pattern — point the MCP stdio transport at the SymForge binary with no arguments. The standard MCP handshake handles the rest.

### Getting the Most Out of SymForge

The `init` command writes a guidance block to your agent's system file (`CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, or `.kilocode/rules/symforge.md` for Kilo Code), but agents don't always follow it — they tend to fall back to built-in file reads and grep out of habit. For best results, add the following to your global or per-project system file so your agent treats SymForge as the primary code navigation layer:

```markdown
## Tooling Preference

When SymForge MCP is available, prefer its tools for repository and code
inspection before falling back to direct file reads.

Use SymForge first for:
- symbol discovery
- text/code search
- file outlines and context
- repository outlines
- targeted symbol/source retrieval
- surgical editing (symbol replacements, renames)
- impact analysis (what changed, what breaks)
- inspection of implementation code under `src/`, `tests/`, and similar
  code-bearing directories

Preferred tools for reading:
- `search_text` — full-text search with enclosing symbol context
- `search_symbols` — find symbols by name, kind, language, path
- `search_files` — ranked file path discovery, co-change coupling
- `get_file_context` — rich file summary with outline, imports, consumers
- `get_file_content` — read files with line ranges or around a symbol
- `get_repo_map` — repository overview at adjustable detail levels
- `get_symbol` — look up symbols by name, batch mode supported
- `get_symbol_context` — symbol body + callers + callees + type deps
- `find_references` — call sites, imports, type usages, implementations
- `find_dependents` — file-level dependency graph
- `inspect_match` — deep-dive a search match with full symbol context
- `analyze_file_impact` — re-read file, update index, report impact
- `what_changed` — files changed since timestamp, ref, or uncommitted
- `diff_symbols` — symbol-level diff between git refs
- `explore` — concept-driven exploration across the codebase

Preferred tools for editing:
- `replace_symbol_body` — replace a symbol's entire definition by name
- `edit_within_symbol` — scoped find-and-replace within a symbol's range
- `insert_symbol` — insert code before or after a named symbol
- `delete_symbol` — remove a symbol and its doc comments by name
- `batch_edit` — multiple symbol-addressed edits atomically across files
- `batch_rename` — rename a symbol and update all references project-wide
- `batch_insert` — insert code before/after multiple symbols across files

Default rule:
- use SymForge to narrow and target code inspection first
- use direct file reads only when exact full-file source or surrounding
  context is still required after tool-based narrowing
- use SymForge editing tools (`replace_symbol_body`, `batch_edit`,
  `edit_within_symbol`) over text-based find-and-replace whenever
  possible to ensure structural integrity and automatic re-indexing

Direct file reads are still appropriate for:
- exact document text in `docs/` or planning artifacts where literal
  wording matters
- configuration files where exact raw contents are the point of inspection

Do not default to broad raw file reads for source-code inspection when
SymForge can answer the question more directly.
```

## Runtime Model

### Startup

1. If `SYMFORGE_AUTO_INDEX` is not `false`, SymForge discovers a project root
2. Tries to connect to or start a local daemon for shared state across terminals
3. Falls back to local in-process mode if daemon connection fails
4. Starts with an empty index if no project root is found

### Daemon Mode

```bash
symforge daemon
```

The daemon binds to local loopback, tracks projects by canonical root, supports multiple concurrent sessions, and persists metadata (`daemon.port`, `daemon.pid`) under `SYMFORGE_HOME`.

If the daemon becomes unreachable mid-session, the next tool call automatically reconnects or falls back to local in-process mode.

### Hooks and Sidecar

Claude Code hook integration uses project-local files under `.symforge/` (`sidecar.port`, `sidecar.pid`, `sidecar.session`, `hook-adoption.log`). Hooks intercept read, edit, write, grep, session-start, and prompt-submit events to enrich responses transparently, and the adoption log lets `health` report routed vs fail-open hook outcomes for SymForge-owned workflows.

When the sidecar is unavailable, hooks fail open with structured diagnostics in the adoption log — distinguishing "port file missing" from "port file stale" with the project root path. Set `SYMFORGE_HOOK_VERBOSE=1` for real-time stderr output showing routing decisions, daemon state, and suggestions. A one-time hint is shown on first failure per session (30-minute cooldown) to guide users toward starting the sidecar.

### Persistence

Index snapshots persist at `.symforge/index.bin` for fast restarts.

### Parameter Handling

All tool parameters accept both native JSON types and stringified values for compatibility with MCP clients that stringify parameters:
- Booleans: `"true"` / `"false"` accepted alongside native `true` / `false`
- Numbers: `"5"` accepted alongside native `5`
- Arrays: `"[{\"path\": \"...\"}]"` (stringified JSON array) accepted alongside native arrays — enables batch tools (`get_symbol` targets, `batch_edit` edits, `search_text` terms, etc.) to work with clients like Kilo Code that stringify array parameters

## Environment Variables

| Variable | Default | Effect |
|----------|---------|--------|
| `SYMFORGE_AUTO_INDEX` | `true` | Enables project discovery and startup indexing |
| `SYMFORGE_CB_THRESHOLD` | `0.20` | Parse-failure circuit-breaker threshold (proportion, e.g. 0.20 = 20%) |
| `SYMFORGE_SIDECAR_BIND` | `127.0.0.1` | Sidecar bind host for local in-process mode |
| `SYMFORGE_HOME` | `~/.symforge` | Home directory for daemon metadata and npm-managed binary |
| `SYMFORGE_HOOK_VERBOSE` | unset | Set to `1` to enable stderr diagnostic output during hook execution (port status, daemon state, routing decisions) |

## Build From Source

```bash
cargo build --release
cargo test
```

The Cargo package name is `symforge`.

## Release Process

Managed through `release-please` + GitHub Actions. Details in [docs/release-process.md](docs/release-process.md).

```bash
python execution/release_ops.py guide     # interactive guide
python execution/release_ops.py status    # current state
python execution/release_ops.py preflight # pre-release checks
python execution/version_sync.py check    # version consistency
```

## Naming

This project was originally called `Tokenizor`. The rename to `SymForge` is complete, but a few historical docs and paths may still mention the old name.

## Codex Integration

For Codex integration details and known limitations, see [Codex Integration Ceiling](docs/codex-integration-ceiling.md).

## License

SymForge is licensed under [PolyForm Noncommercial License 1.0.0](./LICENSE). The official license text is also available from the [PolyForm Project](https://polyformproject.org/licenses/noncommercial/1.0.0/).

You may inspect, study, and use the source code for noncommercial purposes, but commercial use is prohibited unless separately licensed.

