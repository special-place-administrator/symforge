# 0001. Tool Consolidation Contract and Backward-Compat Aliases

Date: 2026-04-17
Status: Accepted

## Context

SymForge exposes a long-lived public MCP tool surface. The surface is defined
in three places that must stay in sync:

- Tool handlers and input structs on `impl SymForgeServer` in
  [`src/protocol/tools.rs`](../../src/protocol/tools.rs) (the `impl` block
  begins at line 2465). Each exposed tool is a method marked with a `#[tool]`
  attribute on this `impl`.
- Output formatters in
  [`src/protocol/format.rs`](../../src/protocol/format.rs) (file-scoped;
  formatters render handler results into the MCP response payload).
- The canonical name registry `SYMFORGE_TOOL_NAMES` in
  [`src/cli/init.rs:262-294`](../../src/cli/init.rs) — the list `symforge
  init` writes into client configs and that clients advertise as allowed.
- The session-scoped dispatcher `execute_tool_call` in
  [`src/daemon.rs:1529-1648`](../../src/daemon.rs). Every MCP call a live
  client makes is routed here; the match arms are the actual runtime contract
  a tool name must satisfy. This is also where consolidated tools live on as
  backward-compat aliases — for example the `trace_symbol` arm at
  [`src/daemon.rs:1562-1580`](../../src/daemon.rs) translates the legacy
  `trace_symbol` params into a `get_symbol_context` call on the server,
  keeping the old tool name callable after its semantics moved.

Tools get consolidated — two related tools merged into one with a mode
parameter — because the MCP surface area is a cost paid by every client,
every prompt, every cached tool catalog. But a consolidation that changes a
tool name or drops a tool breaks every client session that still references
the old name, and every `symforge init`-generated config that allowlisted it.
The project's ADR index (`docs/decisions/README.md`) names this exact class
of change as requiring a durable decision record — "consolidating or
splitting an MCP tool (creates a backward-compat alias contract that other
agents depend on)" and "changing the daemon proxy / `execute_tool_call`
routing in `src/daemon.rs`" are listed side-by-side as ADR triggers.

The procedure for a correct consolidation is already in use (see the
`trace_symbol` → `get_symbol_context` alias cited above) and documented in
the project root `CLAUDE.md` under "Tool Consolidation Pattern". This ADR
formalizes that procedure as a contract and names the invariants that future
consolidations must respect.

## Decision

Consolidating tool `A` into tool `B` follows the seven-step pattern quoted
verbatim from
[`CLAUDE.md`](../../CLAUDE.md):

> When merging tools A into B:
>
> 1. Add new params to B's input struct (with `#[serde(default)]`)
> 2. Add mode branch in B's handler
> 3. Remove `#[tool]` attribute from A (keep the method for internal use)
> 4. Add backward-compat alias in `src/daemon.rs` `execute_tool_call`
> 5. Remove A from `SYMFORGE_TOOL_NAMES` in `src/cli/init.rs`
> 6. Update cross-reference descriptions in other tools
> 7. Update tests: add new field initializers, add mode-specific tests

Three invariants bind this pattern:

- **Alias precedes removal.** The `execute_tool_call` arm for A (step 4)
  ships strictly before A is removed from `SYMFORGE_TOOL_NAMES` (step 5). A
  consolidation commit that performs step 5 without step 4 is a breaking
  change to live client sessions and MUST be reverted rather than patched
  forward.
- **Aliases are permanent.** Once an alias branch exists in
  `execute_tool_call`, removing it is a breaking change to clients whose
  configs were written by an older `symforge init`. Alias removal is itself
  an ADR-worthy decision — the ADR index flags `execute_tool_call` routing
  changes as triggers — and requires a superseding ADR, not a casual edit.
- **The handler method survives the `#[tool]` removal.** Step 3 removes the
  MCP attribute only; the method on `impl SymForgeServer` stays callable
  so the daemon alias in step 4 can dispatch through it. Deleting the method
  outright turns the alias into `unknown tool` at runtime (the trailing
  `anyhow::bail!("unknown tool '{other}'")` in
  [`src/daemon.rs:1646`](../../src/daemon.rs)).

## Consequences

**Easier**

- Shrinking the MCP surface is a safe refactor: the 7 steps produce a
  consolidation that old clients keep calling and new clients see as a
  single tool with modes.
- Future ADRs and runbooks can reference this contract instead of restating
  it. A companion runbook `docs/runbooks/consolidate-mcp-tool.md` (planned)
  turns the pattern into a step-by-step checklist with per-step verification;
  this ADR is the contract it verifies against.

**Harder**

- `src/daemon.rs::execute_tool_call` grows monotonically: every
  consolidation adds an alias branch that cannot be deleted without
  breaking clients. Review of this function must weigh "is this branch a
  live tool or a permanent alias?" — both are load-bearing.
- `SYMFORGE_TOOL_NAMES` in `src/cli/init.rs:262-294` no longer enumerates
  everything a client can call. The set of *callable* tool names is
  `SYMFORGE_TOOL_NAMES ∪ { alias branches in execute_tool_call }`. Any code
  that assumes `SYMFORGE_TOOL_NAMES` is the complete contract (e.g.
  generated documentation, allowlist audits) must be updated to consult
  both.

**New invariants future code must respect**

1. A tool listed in `SYMFORGE_TOOL_NAMES` MUST have a matching arm in
   `execute_tool_call`. CI's existing daemon tests protect this direction.
2. An alias arm in `execute_tool_call` for a name NOT in
   `SYMFORGE_TOOL_NAMES` is the backward-compat contract; it MUST NOT be
   removed without a superseding ADR.
3. `SYMFORGE_TOOL_NAMES` removal for a given tool MUST land in the same
   release as (or a later release than) its `execute_tool_call` alias —
   never earlier. This is the "alias precedes removal" invariant stated in
   release-timing form.
4. The handler method on `impl SymForgeServer` for a consolidated tool
   MUST remain in `src/protocol/tools.rs` for as long as its
   `execute_tool_call` alias exists, even without the `#[tool]` attribute.
