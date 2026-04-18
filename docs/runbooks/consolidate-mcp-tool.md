# Runbook — Consolidate MCP tool A into tool B

## Use when

You have two MCP tools A and B where A's responsibilities are a proper subset (or a mode) of B's, and you want to retire A's public surface while keeping its call sites working through a backward-compat alias.

Examples already in the codebase:

- `trace_symbol` → `get_symbol_context` (alias routed in `src/daemon.rs:1562-1580`, input adapter `TraceSymbolInput` still deserialized; no `#[tool]` attribute on the handler).

If A and B are not a subset/superset, split them differently — consolidation is the wrong tool.

## Preconditions

1. B's handler can represent everything A does — either via an existing parameter shape or by adding a mode/discriminator field. Confirm this by reading B's handler, not by inspection of A alone.
2. You have verified the current line ranges below — they drift frequently and are load-bearing for steps 4 and 5:
   - `src/daemon.rs::execute_tool_call` — the routing match (`src/daemon.rs:1529-1648`).
   - `src/cli/init.rs::SYMFORGE_TOOL_NAMES` — the client allowlist (`src/cli/init.rs:262-294`).
   - `tests/conformance.rs::EXPECTED_TOOLS` — the registered-handler expectation (`tests/conformance.rs:15-46`).
   - `src/protocol/tools.rs::mod tests` — test module entry (`src/protocol/tools.rs:7065`).
3. Working tree is clean on a branch dedicated to this consolidation — each step below is a candidate for its own commit so rollback is granular.
4. You have read the project's consolidation contract in `CLAUDE.md` ("Tool Consolidation Pattern"). This runbook is the executable form of that contract; it does not override it.

## Steps

Each step ends with a verification command. Do not start step N+1 until step N's verification passes.

### Step 1 — Extend B's input struct with A's parameters

Add every parameter A accepted as an optional field on B's input struct, annotated `#[serde(default)]` (plus the project's `lenient_*` deserializers for numeric/boolean shapes). See `GetSymbolInput` at `src/protocol/tools.rs:233-255` as the canonical shape — note `#[serde(default, deserialize_with = "lenient_u32")]` on optional numerics and `#[serde(default)]` on strings that were previously required.

If A introduces a "mode" that B did not have, add a mode/discriminator field rather than reusing existing fields. Overloading existing fields will bite you in step 7.

**Verify:** `cargo check`

### Step 2 — Add the mode branch inside B's handler

Inside B's `#[tool]`-annotated method (see the `get_symbol` handler at `src/protocol/tools.rs:2475-2479` for the canonical attribute + signature), branch on the new field and implement A's behaviour using B's surrounding infrastructure (index guard, formatter, error paths). Do not duplicate logic that A already implements — call A's method if it still exists, or move the body into a private helper both call.

**Verify:** `cargo test --all-targets --test-threads=1 -- <B_handler_name>`
(scope to B's tests so the feedback loop is fast; full suite comes at step 7)

### Step 3 — Remove `#[tool]` from A (keep the method)

Delete the `#[tool(description = "...", annotations(...))]` attribute on A's handler. **Keep the method itself, keep the input struct, keep its tests.** The method is now reachable only from the alias in step 4.

In the same commit, update `tests/conformance.rs::EXPECTED_TOOLS` (`tests/conformance.rs:15-46`) to remove A's name. The test `all_expected_tools_are_registered` (`tests/conformance.rs:53-70`) and its sibling `no_unexpected_tools_registered` together enforce that `EXPECTED_TOOLS` equals the set of `#[tool]`-registered handlers, so removing `#[tool]` without updating `EXPECTED_TOOLS` fails the suite immediately.

Do **not** touch `SYMFORGE_TOOL_NAMES` yet. That lags by one release (step 5).

**Verify:** `cargo check && cargo test --test conformance -- --test-threads=1`

### Step 4 — Add the backward-compat alias in `execute_tool_call`

In `src/daemon.rs::execute_tool_call` (`src/daemon.rs:1529-1648`), add a match arm for A's tool name. It must:

1. Deserialize A's original input struct (so clients sending the old shape still work).
2. Translate it into B's input shape — field-by-field, with the mode field set to A's branch from step 2.
3. Call B's handler with the translated parameters.

See the `trace_symbol` alias at `src/daemon.rs:1562-1580` as the canonical pattern: it deserializes `TraceSymbolInput`, constructs a `GetSymbolContextInput` with field mapping (note the explicit `sections.unwrap_or_default()` that preserves A's default), then delegates.

In the same commit, add a test inside `mod tests` at `src/protocol/tools.rs:7065` that exercises the alias through the handler with A's original input shape and asserts the output matches the non-aliased call. The existing `test_trace_symbol_delegates_to_formatter` at `src/protocol/tools.rs:11886` is the canonical pattern to copy.

**Verify:** `cargo test --all-targets -- --test-threads=1 <A_alias_test_name>`

### Step 5 — Remove A from `SYMFORGE_TOOL_NAMES` *(next release, not this one)*

`SYMFORGE_TOOL_NAMES` (`src/cli/init.rs:262-294`) is the allowlist written into client configs by `symforge init`. It controls whether a client auto-approves calls to A's old name without prompting. Removing A from this list while existing client configs still reference it produces an approval prompt storm for users on the old config.

**Defer this step by at least one release cycle** after steps 1-4 ship. The indicator in this repo: `mcp__symforge__trace_symbol` still sits at `src/cli/init.rs:293` despite the alias having shipped, because the consolidation has not yet been through a post-release cycle.

When the lag period has elapsed, delete A's `mcp__symforge__<A>` entry from `SYMFORGE_TOOL_NAMES`. No test enforces this list's content against `EXPECTED_TOOLS`, so the build will pass silently if you remove it — read twice before editing.

**Verify:** `cargo test --test conformance -- --test-threads=1`
(conformance does not check the allowlist, but ensures you did not accidentally remove a registered tool)

### Step 6 — Update cross-reference descriptions in other tools

Search `src/protocol/tools.rs` for the string `"<A>"` inside `#[tool(description = "...")]` blocks (the description strings end up in the MCP schema and the model sees them). Replace references to A with B + the mode parameter. Use `search_text` on the identifier, not just the tool name, to catch "(use A)" style hints embedded in longer descriptions.

Also scan `docs/` for non-stale references (ADRs, runbooks, `README.md`). Do **not** update `docs/*.md` files that the docs tentacle has marked out-of-scope — write those proposals to `.octogent/tentacles/docs/proposed-destructive.md` instead.

**Verify:** `cargo check` (description strings are validated at macro-expansion time)

### Step 7 — Update tests and run the full suite

Sweep all tests that constructed A's input struct inline. Two patterns are common:

1. Tests that now need the B-input shape with A's mode field — migrate them to the B path.
2. Tests specific to A's handler — keep them where they exercise A's method (the method is still callable internally) but add a matching test through the alias path so regressions in `execute_tool_call` are caught.

If B's input struct grew a new field in step 1, every existing test that literal-initialized B's input now fails to compile — add the new field to each initializer. `cargo check` surfaces these immediately; don't suppress them with `..Default::default()` if B's input does not already implement `Default` — inconsistency there causes a separate class of bug.

**Verify:** `cargo test --all-targets -- --test-threads=1`

## Verification (post-run)

All of the following must be true before the consolidation is done:

- [ ] `cargo check` clean.
- [ ] `cargo test --all-targets -- --test-threads=1` passes (serial is mandatory — the index watcher races under parallel tests).
- [ ] `cargo test --test conformance -- --test-threads=1` passes — this is the drift catcher between `EXPECTED_TOOLS` and registered `#[tool]` handlers.
- [ ] `src/daemon.rs::execute_tool_call` has a match arm for A's name that delegates to B.
- [ ] `src/protocol/tools.rs` has no `#[tool]` attribute on A's handler.
- [ ] `tests/conformance.rs::EXPECTED_TOOLS` does not contain A.
- [ ] `src/cli/init.rs::SYMFORGE_TOOL_NAMES` *still contains* A (until the next release cycle — see step 5).
- [ ] A call with A's original JSON payload still produces an equivalent result through the daemon path (covered by the alias test from step 4).

## Rollback

Consolidation makes no persistent state changes; rollback is git-only.

The safe rollback order depends on how far you got:

- **Within steps 1-4 (pre-merge):** `git reset --hard <pre-consolidation-sha>` is fine. No alias has shipped; no client depends on anything.
- **After steps 1-4 have shipped but before step 5:** revert commits in reverse order (step 4 → step 3 → step 2 → step 1). A stays callable through the alias right up until step 4's revert, at which point existing client configs start getting "unknown tool" errors for A — this is the signal to ship the revert promptly.
- **After step 5 has shipped:** restore A to `SYMFORGE_TOOL_NAMES` *first*, then revert the other commits. Client configs that were regenerated between step 5 and rollback will be missing A's entry; `symforge init` on those clients will re-add it.

The critical ordering: **`SYMFORGE_TOOL_NAMES` must never lag behind the alias in the removal direction**. If you need to remove the alias (step 4 revert), `SYMFORGE_TOOL_NAMES` must already have A re-added, not the other way round — otherwise clients auto-approve calls to a tool name the daemon will reject.

## Related

- Project-level contract: `CLAUDE.md` § "Tool Consolidation Pattern".
- Canonical alias example: `src/daemon.rs:1562-1580` (`trace_symbol`).
- Canonical alias test: `src/protocol/tools.rs:11886` (`test_trace_symbol_delegates_to_formatter`).
- Sibling runbook: `docs/runbooks/add-mcp-tool.md` (forward direction — adding a new tool rather than consolidating).
