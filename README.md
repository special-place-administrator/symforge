![SymForge](./symforge-banner_02.png)

A code-native MCP server that gives AI coding agents structured, symbol-aware access to codebases. Built in Rust with tree-sitter, it replaces raw file scanning with tools that understand code as symbols, references, dependency graphs, and git history through a single MCP connection.

Works with MCP-compatible clients including Claude Code, Codex, Gemini CLI, VS Code MCP, Kilo Code, Roo Code, Cline, Continue, JetBrains plugins, and custom agents.

> [!IMPORTANT]
> **Rust-native** ◆ **31 tools** ◆ **19 source languages** ◆ **5 config formats** ◆ **6 prompts** ◆ **Built-in resources**
>
> **Use SymForge first** for source-code reads, search, repo orientation, symbol tracing, and structural edits.
> **Use raw file reads** for docs and config when exact wording is the point.
> **Use shell tools** for builds, tests, package managers, Docker, and general system tasks.
> **Kilo Code is workspace-local** and should be initialized from the target project directory.

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

Kilo Code is different:

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

## Core workflows

| Goal | Use these tools first |
|------|------------------------|
| Start in a repo | `health`, `get_repo_map`, `explore` |
| Read code | `get_file_context`, `get_symbol`, `get_symbol_context` |
| Exact raw read | `get_file_content` |
| Find code | `search_symbols`, `search_text`, `search_files` |
| Trace impact | `find_references`, `find_dependents`, `what_changed`, `diff_symbols` |
| Edit code | `edit_plan`, `replace_symbol_body`, `edit_within_symbol`, `insert_symbol`, `delete_symbol`, `batch_edit`, `batch_rename`, `batch_insert` |
| Refresh index after edits | `analyze_file_impact`, `index_folder` |
| Validate configs | `validate_file_syntax` |
| Ask in plain English | `ask` |

Practical defaults:

- call `get_file_context` before reading a source file
- use `search_text` or `search_symbols` before broad grep or raw file scans
- use `get_file_content` when exact docs/config text matters
- run `analyze_file_impact` after small edits and `index_folder` after larger multi-file work

## Need deeper reference?

The README is intentionally action-first. For deeper reference material, use the wiki:

- [SymForge Wiki Home](https://github.com/special-place-administrator/symforge/wiki)
- [Architecture and How It Works](https://github.com/special-place-administrator/symforge/wiki/Architecture-and-How-It-Works)
- [Tool Reference](https://github.com/special-place-administrator/symforge/wiki/Tool-Reference)
- [Runtime Model](https://github.com/special-place-administrator/symforge/wiki/Runtime-Model)
- [Supported Languages and Config Formats](https://github.com/special-place-administrator/symforge/wiki/Supported-Languages-and-Config-Formats)
- [Benchmarks and Token Savings](https://github.com/special-place-administrator/symforge/wiki/Benchmarks-and-Token-Savings)

> [!IMPORTANT]
> **NOTICE — One prompt for any AI agent after SymForge install**
>
> If your AI agent still falls back to built-in file reads, grep, or text-based edits after SymForge is installed, give it the following prompt once. It is written to work whether the machine has one client or several.
>
> ```text
> SymForge is already installed on this machine. Your job is to detect which supported AI clients are actually installed on this computer or in this workspace, configure SymForge for the ones you find, and update only the relevant instruction files. Do not assume every client is installed. Do not split this into multiple partial tasks. Complete the whole setup flow for every detected client.
>
> Work in this order:
> 1. Detect installed clients and existing config files before editing anything.
> 2. Prefer running `symforge init` for any client that supports it.
> 3. Repair or add the SymForge MCP entry in that client's MCP config only if needed.
> 4. Update the corresponding global or workspace instruction file so the agent treats SymForge as the primary code navigation and structural editing layer.
> 5. Validate the edited config, reload the client if needed, and confirm that SymForge is connected or ready.
>
> Requirements:
> - First inspect what exists. Only modify files for clients you actually detect.
> - Preserve existing user instructions and existing MCP servers. Merge missing SymForge settings; do not duplicate entries or overwrite unrelated content.
> - Use the SymForge binary path for the current OS: `~/.symforge/bin/symforge` on macOS/Linux or `C:\Users\<user>\.symforge\bin\symforge.exe` on Windows. Use no arguments unless that client already requires extra fields.
> - For JSON-based MCP clients, keep the client's existing schema. Native VS Code MCP uses a top-level `servers` object in `mcp.json`. Many extension-managed clients use a top-level `mcpServers` object. Reuse the schema already present in that client's config file instead of forcing one client's format into another.
> - Preserve client-specific keys such as `disabled`, `alwaysAllow`, `autoApprove`, `timeout`, `type`, `env`, or trust fields if that client supports them. Do not invent unsupported keys.
> - If a SymForge entry already exists, update it in place. Do not create duplicate `symforge` entries.
> - After editing any JSON config, validate that it is still valid JSON and confirm the client can start the SymForge MCP server.
> - If a client reports MCP schema errors that mention another tool or server, do not blame SymForge by default. Check the other configured MCP servers in that client, because many strict clients reject the whole MCP set when any one server advertises invalid JSON Schema.
>
> Detect and update whichever of these are actually present:
> - Claude Code: `~/.claude.json`, `~/.claude/settings.json`, `~/.claude/CLAUDE.md`
> - Codex: `~/.codex/config.toml`, `~/.codex/AGENTS.md`
> - Gemini CLI: `~/.gemini/settings.json`, `~/.gemini/GEMINI.md`
> - Native VS Code MCP: workspace or user `mcp.json` and the nearest project instruction file such as `AGENTS.md`
> - Cline: its MCP JSON settings file plus its rules file such as `~/Documents/Cline/Rules/mandatory.md`
> - Kilo Code: `.kilocode/mcp.json` and `.kilocode/rules/symforge.md`
> - Roo Code and similar extensions: their existing MCP JSON file if present, otherwise the nearest MCP config file the extension already uses, plus the nearest project instruction file the client already consumes
> - If a client is not installed or you cannot find its real config path from existing files, do not guess and do not create random paths for it
>
> Client-specific setup notes:
> - Claude Code, Codex, and Gemini CLI are home-scoped clients. If their home directories already exist, prefer `symforge init` and then verify the written config instead of hand-authoring everything from scratch.
> - Kilo Code is workspace-local. Run `symforge init --client kilo-code` from the target project directory when possible. That workspace should end up with `.kilocode/mcp.json`, `.kilocode/rules/symforge.md`, and `.symforge/`.
> - Native VS Code MCP usually uses user- or workspace-level `mcp.json`. Keep its top-level `servers` shape intact.
> - Cline, Roo Code, Kilo Code, and similar VS Code extensions may use extension-managed JSON files instead of native VS Code `mcp.json`. Detect the real file first and preserve its root key and surrounding schema.
> - For VS Code-family extensions, update both sides when appropriate: the MCP JSON file that launches SymForge and the instruction/rules file the extension already consumes.
>
> Add or merge a SymForge rule block with this behavior:
> - When SymForge is available, prefer it over built-in file, search, and edit tools for source code work.
> - Use SymForge first for: `search_symbols`, `search_text`, `get_file_context`, `get_repo_map`, `get_symbol`, `get_symbol_context`, `find_references`, `find_dependents`, `inspect_match`, `what_changed`, `diff_symbols`, `explore`, `ask`, `conventions`, `edit_plan`, `context_inventory`, `investigation_suggest`.
> - Prefer SymForge edit tools: `replace_symbol_body`, `edit_within_symbol`, `insert_symbol`, `delete_symbol`, `batch_edit`, `batch_rename`, `batch_insert`.
> - Do not default to built-in tools such as `read_file`, `search_files`, `list_files`, `write_to_file`, `replace_in_file`, grep, or broad raw file reads for normal source-code work.
> - Raw reads are still acceptable for non-code files where exact wording matters, such as docs and config files.
> - If SymForge reports that the project is empty, missing, stale, loading, degraded, or otherwise unavailable, do not give up on SymForge. Run `health`, then run `index_folder` on the workspace root if needed, then retry the original SymForge operation.
> - Only fall back to built-in code tools after SymForge recovery was attempted and still failed for a non-indexing reason.
> - After small edits, run `analyze_file_impact` on changed files.
> - After larger multi-file jobs, major refactors, or sprint-sized tasks, run `index_folder` on the workspace root so the index is fresh.
> - Before finishing a large task, do a final `health` check and reindex if needed.
>
> Your output must include:
> - which clients you detected
> - which files you changed
> - which files you intentionally left untouched because the client was not installed or no real config file was found
> - the SymForge rule block you added or updated
> - confirmation that each edited MCP config still parses and points to the SymForge binary
> ```

## Operational notes

- `symforge daemon` is optional if you want a shared index across multiple terminal sessions.
- Index snapshots persist at `.symforge/index.bin` for fast restarts.
- Use `get_file_content` for literal document and config reads.
- Use `validate_file_syntax` when a config file may be malformed.

## Environment variables

| Variable | Default | Effect |
|----------|---------|--------|
| `SYMFORGE_HOME` | `~/.symforge` | Home directory for the binary and daemon metadata |
| `SYMFORGE_AUTO_INDEX` | `true` | Enables project discovery and startup indexing |
| `SYMFORGE_HOOK_VERBOSE` | unset | Set to `1` for stderr hook diagnostics |
| `SYMFORGE_CB_THRESHOLD` | `0.20` | Parse-failure circuit-breaker threshold |
| `SYMFORGE_RECONCILE_INTERVAL` | `30` | Watcher reconciliation interval in seconds; set to `0` to disable periodic reconciliation sweeps |
| `SYMFORGE_SIDECAR_BIND` | `127.0.0.1` | Sidecar bind host for local in-process mode |
| `SYMFORGE_DAEMON_BIND` | loopback bind host | Overrides the daemon bind host used for the shared local daemon |

> [!NOTE]
> **Run This In Your Terminal**
>
> These commands establish a recommended baseline for normal SymForge behavior.
> They are useful if you want your shell or user profile to hold explicit SymForge defaults instead of relying on implicit defaults.

> [!TIP]
> **Recommended baseline**
>
> - keep startup indexing enabled with `SYMFORGE_AUTO_INDEX=true`
> - keep hook diagnostics off by default by leaving `SYMFORGE_HOOK_VERBOSE` unset
> - keep the standard watcher reconciliation interval with `SYMFORGE_RECONCILE_INTERVAL=30`
> - keep loopback bind hosts for `SYMFORGE_SIDECAR_BIND` and `SYMFORGE_DAEMON_BIND`
> - keep the standard circuit-breaker threshold with `SYMFORGE_CB_THRESHOLD=0.20`

> [!WARNING]
> **Copy carefully**
>
> The examples below set a recommended baseline, not a debug profile.
> The persistent examples are written to be idempotent: re-running them updates the same variables instead of appending duplicate config blocks.

### PowerShell

Current terminal session only:

```powershell
$env:SYMFORGE_HOME = "$HOME\\.symforge"
$env:SYMFORGE_AUTO_INDEX = "true"
Remove-Item Env:SYMFORGE_HOOK_VERBOSE -ErrorAction SilentlyContinue
$env:SYMFORGE_CB_THRESHOLD = "0.20"
$env:SYMFORGE_RECONCILE_INTERVAL = "30"
$env:SYMFORGE_SIDECAR_BIND = "127.0.0.1"
$env:SYMFORGE_DAEMON_BIND = "127.0.0.1"
```

Persist for the current Windows user:

```powershell
[Environment]::SetEnvironmentVariable("SYMFORGE_HOME", "$HOME\\.symforge", "User")
[Environment]::SetEnvironmentVariable("SYMFORGE_AUTO_INDEX", "true", "User")
[Environment]::SetEnvironmentVariable("SYMFORGE_HOOK_VERBOSE", $null, "User")
[Environment]::SetEnvironmentVariable("SYMFORGE_CB_THRESHOLD", "0.20", "User")
[Environment]::SetEnvironmentVariable("SYMFORGE_RECONCILE_INTERVAL", "30", "User")
[Environment]::SetEnvironmentVariable("SYMFORGE_SIDECAR_BIND", "127.0.0.1", "User")
[Environment]::SetEnvironmentVariable("SYMFORGE_DAEMON_BIND", "127.0.0.1", "User")
```

These calls overwrite the same user-level variable names, so re-running them is already idempotent.

### CMD

Current terminal session only:

```bat
set SYMFORGE_HOME=%USERPROFILE%\.symforge
set SYMFORGE_AUTO_INDEX=true
set SYMFORGE_HOOK_VERBOSE=
set SYMFORGE_CB_THRESHOLD=0.20
set SYMFORGE_RECONCILE_INTERVAL=30
set SYMFORGE_SIDECAR_BIND=127.0.0.1
set SYMFORGE_DAEMON_BIND=127.0.0.1
```

Persist for the current Windows user:

```bat
setx SYMFORGE_HOME "%USERPROFILE%\.symforge"
setx SYMFORGE_AUTO_INDEX "true"
setx SYMFORGE_HOOK_VERBOSE ""
setx SYMFORGE_CB_THRESHOLD "0.20"
setx SYMFORGE_RECONCILE_INTERVAL "30"
setx SYMFORGE_SIDECAR_BIND "127.0.0.1"
setx SYMFORGE_DAEMON_BIND "127.0.0.1"
```

`setx` updates the same variable names instead of creating duplicates. Open a new terminal after running it.

### Linux Terminal

Current shell session only:

```bash
export SYMFORGE_HOME="$HOME/.symforge"
export SYMFORGE_AUTO_INDEX=true
unset SYMFORGE_HOOK_VERBOSE
export SYMFORGE_CB_THRESHOLD=0.20
export SYMFORGE_RECONCILE_INTERVAL=30
export SYMFORGE_SIDECAR_BIND=127.0.0.1
export SYMFORGE_DAEMON_BIND=127.0.0.1
```

Persist for future shells:

```bash
export SYMFORGE_RC_FILE="$HOME/.bashrc"
python3 - <<'PY'
from pathlib import Path
import os, re

path = Path(os.environ["SYMFORGE_RC_FILE"]).expanduser()
start = "# >>> SymForge env >>>"
end = "# <<< SymForge env <<<"
block = """# >>> SymForge env >>>
export SYMFORGE_HOME="$HOME/.symforge"
export SYMFORGE_AUTO_INDEX=true
export SYMFORGE_CB_THRESHOLD=0.20
export SYMFORGE_RECONCILE_INTERVAL=30
export SYMFORGE_SIDECAR_BIND=127.0.0.1
export SYMFORGE_DAEMON_BIND=127.0.0.1
# <<< SymForge env <<<"""

text = path.read_text() if path.exists() else ""
pattern = re.compile(re.escape(start) + r".*?" + re.escape(end), re.S)
if pattern.search(text):
    text = pattern.sub(block, text)
else:
    if text and not text.endswith("\n"):
        text += "\n"
    text += block + "\n"
path.write_text(text)
PY
unset SYMFORGE_RC_FILE

source ~/.bashrc
```

If you use `zsh`, put the same lines in `~/.zshrc` instead.

### macOS Terminal

Current shell session only:

```bash
export SYMFORGE_HOME="$HOME/.symforge"
export SYMFORGE_AUTO_INDEX=true
unset SYMFORGE_HOOK_VERBOSE
export SYMFORGE_CB_THRESHOLD=0.20
export SYMFORGE_RECONCILE_INTERVAL=30
export SYMFORGE_SIDECAR_BIND=127.0.0.1
export SYMFORGE_DAEMON_BIND=127.0.0.1
```

Persist for future shells:

```bash
export SYMFORGE_RC_FILE="$HOME/.zshrc"
python3 - <<'PY'
from pathlib import Path
import os, re

path = Path(os.environ["SYMFORGE_RC_FILE"]).expanduser()
start = "# >>> SymForge env >>>"
end = "# <<< SymForge env <<<"
block = """# >>> SymForge env >>>
export SYMFORGE_HOME="$HOME/.symforge"
export SYMFORGE_AUTO_INDEX=true
export SYMFORGE_CB_THRESHOLD=0.20
export SYMFORGE_RECONCILE_INTERVAL=30
export SYMFORGE_SIDECAR_BIND=127.0.0.1
export SYMFORGE_DAEMON_BIND=127.0.0.1
# <<< SymForge env <<<"""

text = path.read_text() if path.exists() else ""
pattern = re.compile(re.escape(start) + r".*?" + re.escape(end), re.S)
if pattern.search(text):
    text = pattern.sub(block, text)
else:
    if text and not text.endswith("\n"):
        text += "\n"
    text += block + "\n"
path.write_text(text)
PY
unset SYMFORGE_RC_FILE

source ~/.zshrc
```

If you use `bash` on macOS, put the same lines in `~/.bash_profile` or `~/.bashrc`.

## Build from source

```bash
cargo build --release
cargo test
```

The Cargo package name is `symforge`.

## License

SymForge is licensed under [PolyForm Noncommercial License 1.0.0](./LICENSE). The official license text is also available from the [PolyForm Project](https://polyformproject.org/licenses/noncommercial/1.0.0/).

You may inspect, study, and use the source code for noncommercial purposes, but commercial use is prohibited unless separately licensed.
