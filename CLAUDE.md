# CLAUDE.md — SymForge

## Git & GitHub CLI

**CRITICAL: The `GITHUB_TOKEN` env var is a limited fine-grained PAT injected by Claude Code. It CANNOT create PRs, trigger workflows, or manage releases.**

Prefix ALL `gh` commands with `GITHUB_TOKEN=` to use the keyring token (has `repo` + `workflow` scopes):

```bash
GITHUB_TOKEN= gh pr create ...
GITHUB_TOKEN= gh workflow run ...
GITHUB_TOKEN= gh run list ...
GITHUB_TOKEN= gh run view ...
```

## Deployment Workflow

**Never manually create tags or specify tag numbers in workflow_dispatch. Release-please handles versioning.**

### Standard release flow:

```
1. Feature branch work complete, tests pass
   cargo test && cargo fmt -- --check

2. Create PR to main
   GITHUB_TOKEN= gh pr create --title "feat: description" --body "..." --base main

3. Merge PR (from UI or CLI)
   GITHUB_TOKEN= gh pr merge <number> --merge

4. Push to main triggers Release workflow automatically:
   a. release-please creates release PR (e.g. "chore(main): release 0.19.0")
   b. Auto-merge merges it using RELEASE_PLEASE_TOKEN (GitHub Actions secret)
   c. Second workflow run: release-please creates tag + GitHub release
   d. Build matrix: windows, linux, macos-arm64, macos-x64
   e. npm publish to registry

5. Monitor
   GITHUB_TOKEN= gh run list -L 5
   GITHUB_TOKEN= gh run view <run-id> --log-failed
```

### workflow_dispatch with tag input:
- ONLY for rebuilding an EXISTING release (tag must already exist)
- Do NOT use this to create new releases

### CI failures:
- `cargo fmt` differences between local and CI are common
- Fix: `cargo fmt && git add -A && git commit -m "style: fix rustfmt formatting" && git push`
- Always run `cargo fmt -- --check` before pushing

## Build & Test

```bash
cargo test --all-targets -- --test-threads=1   # match CI config
cargo fmt -- --check                            # match CI check
cargo check                                     # quick compilation check
```

## Architecture

Rust MCP server providing symbol-aware code navigation and editing tools. Currently 24 tools exposed via MCP `tools/list`, with backward-compat aliases for removed tools in `src/daemon.rs`.

Key source files:
- `src/protocol/tools.rs` — Tool handlers, input structs, tests
- `src/protocol/format.rs` — Output formatters
- `src/daemon.rs` — Daemon proxy with backward-compat aliases
- `src/cli/init.rs` — Tool name list for client init
- `src/live_index/query.rs` — Index query functions
- `src/protocol/resources.rs` — MCP resource handlers

## Tool Consolidation Pattern

When merging tools A into B:
1. Add new params to B's input struct (with `#[serde(default)]`)
2. Add mode branch in B's handler
3. Remove `#[tool]` attribute from A (keep the method for internal use)
4. Add backward-compat alias in `src/daemon.rs` `execute_tool_call`
5. Remove A from `SYMFORGE_TOOL_NAMES` in `src/cli/init.rs`
6. Update cross-reference descriptions in other tools
7. Update tests: add new field initializers, add mode-specific tests

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

## Codex Integration

For Codex-specific integration guidance and limitations, see [docs/codex-integration-ceiling.md](docs/codex-integration-ceiling.md).
