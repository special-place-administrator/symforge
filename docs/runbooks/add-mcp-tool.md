# Runbook: Add a new MCP tool

Add a new tool to the SymForge MCP surface. The procedure touches five files in lockstep; skipping any of them produces a visibly broken tool (client init drops the name, daemon returns "unknown tool", or the symbol name is unreachable from the router).

## Use when

- Shipping a new read or edit capability that should be callable from Claude Code / Codex / Gemini / Kilo / Claude Desktop via MCP.
- NOT for renaming an existing tool (that is a consolidation with a backward-compat alias — see `consolidate-mcp-tool.md`).
- NOT for changing an existing tool's inputs or output formatting (no new registration is required).

## Preconditions

Before starting, pin these decisions — they determine which files you touch and what the tests should assert.

- [ ] **Tool name** chosen. Snake_case, no `symforge_` prefix (the MCP layer adds `mcp__symforge__`). Check it is not already present in `SYMFORGE_TOOL_NAMES` (`src/cli/init.rs:262-294`).
- [ ] **Input struct fields** defined: name, type, and whether each is required or `#[serde(default)]`. Use `lenient_bool` / `lenient_u32` / `lenient_option_vec` deserializers for numeric and boolean fields that clients may pass as strings (see existing structs in `src/protocol/tools.rs` starting at L229).
- [ ] **Read vs edit classification** decided. Read tools set `annotations(read_only_hint = true, open_world_hint = false)` on `#[tool(...)]`. Edit tools do not; they also participate in post-edit bookkeeping via the `EditHook` trait.
- [ ] **Output shape** decided. If the output is a plain string with fixed sections, inline formatting in the handler. If it has reusable structure (multi-section summaries, token accounting, tables), add a formatter in `src/protocol/format.rs` alongside peers like `format_token_savings` (`src/protocol/format.rs:3330`).
- [ ] **Backward-compat requirement** verified: you are adding, not replacing. If this tool ships alongside the removal of an older tool with overlapping scope, stop and follow `consolidate-mcp-tool.md` instead.

## Steps

Perform all six steps in one working branch. Do not commit between steps — run `cargo check` after each step to fail fast.

### 1. Add the input struct in `src/protocol/tools.rs`

Add the struct to the "Input parameter structs" block (starts at L229). Follow the shape of `GetSymbolInput` (`src/protocol/tools.rs:233-255`):

- `#[derive(Deserialize, Serialize, JsonSchema)]`
- `pub struct NewToolInput { ... }`
- Each optional field uses `#[serde(default, deserialize_with = "lenient_*")]` as appropriate.
- Doc comment on every public field — it surfaces in the JSON schema clients fetch.

Verify: `cargo check`.

### 2. Add the handler on `impl SymForgeServer`

Add the handler inside the `#[tool_router(vis = "pub(crate)")]` impl block at `src/protocol/tools.rs:2465`. Follow the shape of `get_symbol` at L2475-2479:

```rust
#[tool(
    description = "Prefer this over <worse alternative>. <One sentence on what it does>. <One sentence on when NOT to use it>.",
    annotations(read_only_hint = true, open_world_hint = false)  // omit for edit tools
)]
pub(crate) async fn new_tool(&self, params: Parameters<NewToolInput>) -> String {
    if let Some(result) = self.proxy_tool_call("new_tool", &params.0).await {
        return result;
    }
    // implementation
}
```

The `proxy_tool_call` short-circuit is mandatory — without it, a daemon-mode process will execute the tool in the wrong project root. Every handler in the impl block has it; copy the line verbatim.

Verify: `cargo check`.

### 3. Add a formatter in `src/protocol/format.rs` (only if output is non-trivial)

Skip this step if the handler returns a short literal string. Otherwise, add a free function next to peers like `format_token_savings` at `src/protocol/format.rs:3330`. Keep the formatter pure: it takes data, returns `String`. No I/O, no index reads.

Verify: `cargo check`.

### 4. Register the tool name in `SYMFORGE_TOOL_NAMES`

Add `"mcp__symforge__new_tool"` to the `SYMFORGE_TOOL_NAMES` list at `src/cli/init.rs:262-294`. This list drives `claude mcp` allow-lists, Codex `allowed_tools`, Gemini `trustedTools`, and Kilo `allowed_tools` emitted by `symforge init`. **If you skip this step, users who ran `symforge init` before your release will not have permission to call the tool — the MCP surface advertises it, but every client auto-declines.**

Verify: `cargo check`; existing init tests cover the parse shape — run `cargo test init -- --test-threads=1` to confirm none broke.

### 5. Wire daemon routing in `src/daemon.rs::execute_tool_call`

Add a match arm to `execute_tool_call` at `src/daemon.rs:1529-1648`. Follow the shape of the `get_symbol` arm at L1541-1543:

```rust
"new_tool" => Ok(server
    .new_tool(Parameters(decode_params::<NewToolInput>(params)?))
    .await),
```

The tool name in quotes is the daemon-facing name (no `mcp__symforge__` prefix — daemon routing strips the prefix before dispatch). **If you skip this step, the tool works in embedded mode but returns "unknown tool 'new_tool'" when called through the daemon.**

Verify: `cargo check`.

### 6. Add at least one test in `mod tests` inside `src/protocol/tools.rs`

Append a `#[test]` or `#[tokio::test]` to the `mod tests` block at `src/protocol/tools.rs:7065`. Use the existing test helpers in that module (`make_symbol`, `make_symbol_with_bytes`, the `TempDir` fixtures). Assert at minimum:

- The handler returns a non-empty string for a well-formed input.
- It returns a well-formed error string (not a panic) for the one most likely malformed input — missing required field, unknown path, empty query.

Verify: `cargo test --all-targets -- --test-threads=1` (the project mandates `--test-threads=1` per `CLAUDE.md`).

## Verification

After all six steps, run the full project check:

- [ ] `cargo check` — clean build.
- [ ] `cargo test --all-targets -- --test-threads=1` — all tests pass, including your new one.
- [ ] `cargo build --release` — release artifact builds.
- [ ] Boot the binary and list tools: the new name appears in the MCP `tools/list` response.
- [ ] In an `init`-ed client session, the tool name appears in the allow-list for at least one client (e.g. `~/.claude/settings.json` → `permissions.allow`).
- [ ] Optional: invoke the tool once via the daemon path — the "unknown tool" error is the most common missed-step symptom and is cheap to confirm absent.

## Rollback

There is no persistent state to unwind — tool registration lives entirely in source files and regenerated client settings. To revert:

1. `git revert <commit-sha>` — undoes all six edits atomically.
2. Re-run `symforge init` in any client workspace where the tool has already been registered to remove the name from the allow-list. This is idempotent; if the name is no longer in `SYMFORGE_TOOL_NAMES`, `init` strips it from `permissions.allow` on the next run.

If only some of the six steps were committed (e.g. the handler and tool-names entry but not the daemon arm), do NOT leave the partial state in `main`. The daemon will return "unknown tool" to any client that picked up the new name from the allow-list. Either revert or finish the remaining steps.

## Related

- `src/protocol/tools.rs` — handler surface, input structs, `mod tests`.
- `src/protocol/format.rs` — formatters.
- `src/cli/init.rs` — `SYMFORGE_TOOL_NAMES` drives client allow-lists.
- `src/daemon.rs::execute_tool_call` — daemon-path dispatch.
- `CLAUDE.md` (project root) — "Tool Consolidation Pattern" for the related case of merging tools, which preserves the old name as a daemon-only alias.
