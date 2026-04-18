# 0002. Parasitic Hook Integration, Not Tool Replacement

Date: 2026-04-18
Status: Accepted

## Context

SymForge delivers symbol-aware code navigation to coding agents. The project
has an in-principle choice about how that value reaches the model:

- **Replace** the model's built-in tools. Push the agent away from `Read`,
  `Grep`, `Edit` toward SymForge's MCP tools via prompt instructions
  ("prefer SymForge for source code"), CLAUDE.md conventions, or
  tool-description steering.
- **Parasitically enrich** the model's built-in tools. Leave `Read`, `Grep`,
  `Edit` in place; inject structural context into every native tool call
  via hooks so each call returns richer, symbol-aware output than it would
  otherwise.

The v2 rewrite committed to the second option in
[`docs/ROADMAP-v2.md:66-73`](../ROADMAP-v2.md) (architectural decision AD-2):

> Do NOT try to replace the model's Read/Grep/Edit tools. Instead, hook into
> PostToolUse to enrich every native tool call with structural context from
> the LiveIndex.
>
> **Rationale**: Models are trained on their native tools. CLAUDE.md
> instructions to "prefer MCP tools" are fragile — the model drifts. Hooks
> are deterministic: every Read gets an outline injected, every Edit
> triggers re-index + impact analysis. Zero behavior change required from
> the model.

That commitment is now load-bearing. The runtime architecture is organized
around it:

- The HTTP sidecar spawned by
  [`spawn_sidecar`](../../src/sidecar/server.rs) in
  [`src/sidecar/server.rs:28-105`](../../src/sidecar/server.rs) binds an
  ephemeral local port and writes it to the project-relative file
  `sidecar.port` (constant `PORT_FILE` at
  [`src/sidecar/port_file.rs:12`](../../src/sidecar/port_file.rs)). This is
  the liveness signal the enrichment layer uses to know it can call into
  the indexed store.
- The hook CLI
  [`run_hook`](../../src/cli/hook.rs) in
  [`src/cli/hook.rs:209`](../../src/cli/hook.rs) is the entry point Claude
  Code invokes for every matched `PostToolUse` / `PreToolUse` event. It
  reads the stdin payload, routes to a workflow based on the triggering
  tool (`Read` → source-outline, `Edit` → impact, `Grep` → hit-expansion,
  etc.), and prints a `hookSpecificOutput` JSON envelope back to the agent
  harness. The native tool call still happens; the hook returns the
  enrichment.
- The `PreToolUse` branch at
  [`src/cli/hook.rs:225-232`](../../src/cli/hook.rs) short-circuits when
  the sidecar is active (`read_port_file().is_ok()`), suppressing the
  "prefer SymForge" suggestion emitted by
  [`pre_tool_suggestion`](../../src/cli/hook.rs) at
  [`src/cli/hook.rs:450-469`](../../src/cli/hook.rs). This is the AD-2
  stance made concrete: when the agent is already using SymForge, the
  nudge toward MCP tools is counterproductive noise, so the system goes
  quiet and lets the enrichment path do the work instead.

The operational consequence is already documented in user-facing docs:

- [`README.md:23`](../../README.md) — "Smarter PreToolUse hooks — When the
  SymForge sidecar is already running, tool-preference hints are suppressed
  to reduce noise for agents that are actively using SymForge."
- [`README.md:225`](../../README.md) — "PreToolUse hooks auto-suppress when
  the sidecar is active — no redundant 'use SymForge' hints when you're
  already using it."

AD-2's rationale has never been formalized as an ADR. Because every queued
feature — the worktree-awareness redirect
([ADR 0010](./0010-worktree-working-directory.md)), the edit-tool and
ranker extension points
([ADR 0012](./0012-edit-and-ranker-hook-architecture.md)), the planned
frecency-ranking signal — is a direct consequence of it, the invariant
needs to live somewhere future contributors will see before they propose a
"tell the model to prefer SymForge" fix.

## Decision

Every agent-facing improvement in SymForge reaches the model by one of two
paths only:

1. A **new or extended MCP tool** exposed through the handler surface on
   `impl SymForgeServer` in
   [`src/protocol/tools.rs`](../../src/protocol/tools.rs). These tools are
   opt-in for the agent and are invoked explicitly.
2. An **enrichment of an existing native tool call** delivered through the
   hook/sidecar path: `run_hook`
   ([`src/cli/hook.rs:209`](../../src/cli/hook.rs)) reads the `PostToolUse`
   or `PreToolUse` payload for `Read`, `Grep`, `Edit`, `Write`, and
   friends; the sidecar (spawned by `spawn_sidecar` at
   [`src/sidecar/server.rs:28-105`](../../src/sidecar/server.rs)) answers
   queries from the LiveIndex; the hook prints a `hookSpecificOutput`
   envelope that the agent harness appends to the tool result.

The third option — **telling the model to prefer SymForge** — is not a
valid design for new capability. Prompt-level steering is:

- Fragile. The model drifts across turns, context windows, and task types;
  any capability whose correctness depends on the agent obeying a
  preference instruction is intermittent.
- Duplicative. Native tools remain first-class citizens in the agent's
  training distribution, so two tools covering the same job is a
  persistent ambiguity the agent re-resolves on every call.
- Agent-local. A hint delivered via CLAUDE.md or tool descriptions lands
  only in harnesses that read those channels; hook enrichment lands in
  every harness that honors the standard `PostToolUse` protocol.

The already-shipped `PreToolUse` suggestion at
[`src/cli/hook.rs:450-469`](../../src/cli/hook.rs) is a transitional
affordance, not a design precedent: it fires only when the sidecar is
*not* running, exists to bootstrap agents that don't yet know SymForge is
available, and is explicitly suppressed once the enrichment path is live
([`src/cli/hook.rs:225-232`](../../src/cli/hook.rs)). Future features MUST
NOT propagate the "tell the model what to do" pattern into the
sidecar-active path.

## Consequences

**Easier**

- Capability growth does not require agent retraining or per-harness
  prompt work. A new `PostToolUse(Edit)` enrichment lands in every
  compatible agent on the next tool call.
- Removing an MCP tool does not strand capability. If the value is
  mirrored as hook enrichment, the native tool path continues to deliver
  it.
- The AD-2 stance gives a clean rejection test for proposed features:
  "which path — new MCP tool, or enriched native tool?" Any answer other
  than those two routes must be rewritten before review.

**Harder**

- The sidecar and hook layer are now on the critical path for *perceived*
  SymForge value, not just the MCP tool surface. Hook response time
  constraints (Claude Code's 600s hook timeout and the
  in-index <100ms target stated in
  [`docs/ROADMAP-v2.md:522-525`](../ROADMAP-v2.md)) apply to every
  enrichment handler, not only to explicit MCP calls.
- Every new enrichment point is a new public contract. The hook input
  shape (`HookInput`, `HookToolInput`), the workflow routing enum
  (`HookWorkflow`), and the `hookSpecificOutput` JSON envelope in
  [`src/cli/hook.rs`](../../src/cli/hook.rs) MUST remain backward-compatible
  with live agent harnesses — changes there are on the same footing as
  the MCP tool-alias contract formalized in
  [ADR 0001](./0001-tool-consolidation-contract.md).
- The `sidecar_active` short-circuit at
  [`src/cli/hook.rs:225-232`](../../src/cli/hook.rs) is the project's
  stance encoded in runtime behavior. Proposals to make `PreToolUse` emit
  tool-preference hints when the sidecar IS active must carry a
  superseding ADR.

**New invariants future code must respect**

1. A proposed SymForge feature that routes value to the model via "prompt
   the agent to prefer an MCP tool" is out of scope for AD-2. It MUST be
   rewritten as either a new MCP tool (with the handler-surface changes
   governed by [ADR 0001](./0001-tool-consolidation-contract.md)) or a
   new hook enrichment handler in
   [`src/sidecar/handlers.rs`](../../src/sidecar/handlers.rs) wired
   through [`src/cli/hook.rs`](../../src/cli/hook.rs).
2. New `PostToolUse` enrichment handlers MUST be the default delivery
   path for capability that makes *existing* agent workflows smarter
   (impact after edit, outline after read, symbol expansion after grep).
   Adding a new MCP tool to cover the same workflow is acceptable only
   when the capability needs agent-initiated invocation — e.g. exploring
   a symbol's references on demand rather than on every read.
3. Any change that re-enables "prefer SymForge" style prompting on the
   `sidecar_active` path (removing or inverting the suppression in
   [`src/cli/hook.rs:225-232`](../../src/cli/hook.rs)) is a reversal of
   this ADR and requires a superseding ADR, not an inline edit.
