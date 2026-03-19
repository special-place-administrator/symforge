# RTK-Style Adoption Integration Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give SymForge RTK-style path-of-least-resistance adoption mechanics without collapsing SymForge into a shell-output summarizer. SymForge should remain the semantic code-intelligence layer, but it should intercept common shell-native workflows early enough that LLMs stop bypassing it.

**Architecture:** Add an adoption layer on top of the existing MCP/index core. Expand hook interception, add workflow-specific sidecar adapters for high-frequency shell intents, tighten init-generated guidance, and surface stronger fallback/next-step routing. Keep the semantic source of truth in the indexed SymForge engine; do not build a generic shell proxy product.

**Tech Stack:** Rust CLI hooks, daemon/session sidecar endpoints, existing formatter/tooling stack, existing MCP tools/resources/prompts

**Related Docs:**
- `docs/superpowers/specs/2026-03-17-llm-tool-preference-strategy.md`
- `docs/superpowers/specs/2026-03-19-symbol-edit-boundaries-design.md`

---

## Problem Statement

RTK’s core advantage is not deeper code understanding. Its advantage is **adoption by interception**:

1. the user or model issues a familiar shell action
2. the integration layer rewrites or routes it automatically
3. the model receives a compact, structured result without first deciding to use a special tool

SymForge currently wins on semantic depth, but still loses too many first-contact decisions because:

- it depends on the model choosing MCP tools explicitly
- hook enrichment is suggestive more often than authoritative
- the highest-frequency shell workflows are not fully “owned”
- exact raw reads and command results still feel like the fastest path for the model

This plan closes that gap while preserving SymForge’s product identity.

---

## Non-Goals

- Do **not** turn SymForge into a general-purpose shell command wrapper.
- Do **not** replace semantic MCP tools with string-postprocessing of raw command output.
- Do **not** hide degraded or fallback behavior; surface it clearly.
- Do **not** regress direct MCP use for clients that already call SymForge tools correctly.

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/cli/hook.rs` | Modify | Expand interception policy and workflow routing hints |
| `src/cli/init.rs` | Modify | Install stronger client guidance and hook defaults |
| `src/sidecar/router.rs` | Modify | Add or reorganize workflow-oriented endpoints if needed |
| `src/sidecar/handlers.rs` | Modify | Implement shell-workflow adapters backed by SymForge semantics |
| `src/daemon.rs` | Modify | Expose session-scoped routing for new adoption endpoints |
| `src/protocol/tools.rs` | Modify | Improve tool descriptions and routing affordances |
| `src/protocol/format.rs` | Modify | Add next-step/fallback/token-savings messaging where useful |
| `src/protocol/resources.rs` | Review / maybe modify | Add reusable resources for workflow adapters if needed |
| `README.md` | Modify | Clarify product positioning vs raw shell workflows |
| `tests/hook_enrichment_integration.rs` | Modify | Cover interception behavior and fallback messaging |
| `tests/sidecar_integration.rs` | Modify | Cover workflow endpoint behavior end to end |
| `tests/init_integration.rs` | Modify | Verify client guidance and registration outcomes |

---

## Chunk 1: Product Boundary + Workflow Ownership

### Task 1: Define the workflows SymForge should own

**Files:**
- Modify: `README.md`
- Modify: `src/cli/hook.rs`

- [ ] **Step 1: Establish the “owned workflow” list**

Create a concrete list of first-class workflows SymForge should intercept or redirect:

1. source-code read
2. source-code grep/search
3. first-contact repo overview
4. symbol lookup
5. post-edit impact
6. prompt-context enrichment
7. code review / changed-symbol inspection

- [ ] **Step 2: Document the ownership boundary**

State explicitly:

- SymForge owns semantic code-inspection workflows
- shell remains appropriate for process control, package management, running tests, and system inspection
- config/doc raw reads remain valid when exact literal content is the point

- [ ] **Step 3: Record the “do not become RTK” boundary**

Add a short positioning note in `README.md`:

- RTK-style adoption mechanics are in scope
- generic shell summarization as the primary product is out of scope

---

## Chunk 2: Hook Interception Becomes More Assertive

### Task 2: Upgrade hook behavior from suggestion-heavy to workflow-routing-heavy

**Files:**
- Modify: `src/cli/hook.rs`
- Modify: `tests/hook_enrichment_integration.rs`

- [ ] **Step 1: Audit current hook routing**

Review existing handling for:

- `Read`
- `Grep`
- `Edit`
- `Glob`
- `SessionStart`
- `UserPromptSubmit`

Classify each current behavior as:

- hard route
- soft suggestion
- fail-open passthrough

- [ ] **Step 2: Add a workflow classifier**

Introduce an internal classification layer that maps incoming hook/tool events to semantic workflows such as:

- `SourceRead`
- `SourceSearch`
- `RepoStart`
- `PromptContext`
- `PostEditImpact`

The goal is to reason in terms of workflows, not raw client tool names.

- [ ] **Step 3: Tighten high-confidence routes**

For obvious source-code cases:

- `Read` on code files should route to `get_file_context` or a sidecar outline endpoint by default
- `Grep` on code intent should route to `search_text`
- `Edit` completion should continue to trigger impact analysis

Do not just emit “consider using X”; return the stronger guided alternative when confidence is high.

- [ ] **Step 4: Preserve fail-open behavior**

When confidence is low or the file is unsuitable:

- fall back cleanly
- say why the route was not taken
- give exactly one recommended next step

- [ ] **Step 5: Add integration tests**

Cover:

- code read routed to SymForge
- markdown/config read intentionally not over-routed
- grep over source routed to `search_text`
- fallback path emits rationale, not silent passthrough

---

## Chunk 3: Sidecar Adapters for High-Frequency Shell-Native Workflows

### Task 3: Add workflow adapters that feel automatic but remain semantic

**Files:**
- Modify: `src/sidecar/handlers.rs`
- Modify: `src/sidecar/router.rs`
- Modify: `src/daemon.rs`
- Modify: `tests/sidecar_integration.rs`

- [ ] **Step 1: Define adapter endpoints**

Add or formalize sidecar endpoints for:

- source read / outline
- search hit expansion
- prompt-context narrowing
- post-edit impact summary
- repo-start quick map

These should be thin adapters over existing semantic queries, not independent logic silos.

- [ ] **Step 2: Keep adapters backed by MCP-grade primitives**

Each adapter must delegate into existing semantic layers such as:

- `get_file_context`
- `search_text`
- `get_symbol_context`
- `analyze_file_impact`
- `get_repo_map`

Do not duplicate ranking or formatting logic unnecessarily.

- [ ] **Step 3: Add session-scoped daemon exposure**

Ensure the daemon exposes any new sidecar routes under the current session namespace so multi-session behavior stays deterministic.

- [ ] **Step 4: Add e2e tests**

Verify that hook-triggered workflow routes and direct sidecar endpoint calls agree on:

- path resolution
- ambiguity behavior
- token-savings/fallback messaging

---

## Chunk 4: Tool Descriptions and Response UX

### Task 4: Make the MCP surface itself steer better

**Files:**
- Modify: `src/protocol/tools.rs`
- Modify: `src/protocol/format.rs`
- Review: `src/protocol/resources.rs`

- [ ] **Step 1: Strengthen “prefer over raw read” wording**

Update descriptions for:

- `get_file_context`
- `get_repo_map`
- `get_symbol`
- `search_text`
- `search_symbols`
- `get_file_content`

Add:

- explicit “prefer this over raw file reads for code understanding”
- typical workflow hints
- token-efficiency framing where justified

- [ ] **Step 2: Add contextual next-step hints**

For selected outputs, append compact next-step guidance such as:

- after `get_file_context`: “use `get_symbol` for body”
- after `search_text`: “use `inspect_match` for symbol context”
- after `health` on first use: “start with `get_repo_map`”

Keep this sparse; the hints should help, not clutter.

- [ ] **Step 3: Improve degraded/fallback messaging**

When the system falls back or serves reduced context, say:

- what happened
- what signal caused the fallback
- what the best next SymForge action is

- [ ] **Step 4: Add regression tests**

Verify:

- key descriptions contain the new routing language
- response formatting does not become noisy
- fallback guidance is stable and informative

---

## Chunk 5: Init/Client Guidance Tightening

### Task 5: Make installed clients harder to mis-route

**Files:**
- Modify: `src/cli/init.rs`
- Modify: `tests/init_integration.rs`

- [ ] **Step 1: Upgrade generated guidance**

The generated guidance for Codex, Claude, Gemini, and Kilo-compatible clients should say:

- SymForge is primary for repo-local source inspection
- raw reads are fallback for exact docs/config or non-indexed content
- semantic-first workflow rules

- [ ] **Step 2: Add project-level rule emission strategy**

Decide whether `symforge init` should also emit project-local guidance artifacts for clients that prefer repo-local rules.

At minimum, document the strategy and implement the most valuable safe subset.

- [ ] **Step 3: Keep allowed tool exposure frictionless**

Ensure the generated allow-lists continue to include the canonical semantic entry points and edit/impact tools required for the owned workflows.

- [ ] **Step 4: Add tests**

Verify:

- generated guidance contains stronger routing language
- Codex guidance remains full-strength
- existing user content is preserved idempotently

---

## Chunk 6: Measurement and Rollout

### Task 6: Prove adoption improvement rather than assuming it

**Files:**
- Modify: `src/cli/hook.rs`
- Modify: `src/protocol/format.rs`
- Modify: tests where needed

- [ ] **Step 1: Define adoption metrics**

Track at least:

- hook interceptions by workflow
- accepted semantic route vs fallback rate
- token savings by workflow
- first-contact tool choice patterns where observable

- [ ] **Step 2: Add lightweight reporting**

Expose useful summaries through:

- `health`
- hook output
- optional debug logging

- [ ] **Step 3: Add before/after acceptance checks**

Create a small eval set for:

- “read this source file”
- “grep for this symbol”
- “show me what changed”
- “what file should I read first?”

Expected result: the preferred path should be SymForge-backed, not raw shell-backed.

---

## Verification

- [ ] `cargo fmt`
- [ ] `cargo check --workspace`
- [ ] `cargo test hook -- --test-threads=1`
- [ ] `cargo test sidecar -- --test-threads=1`
- [ ] `cargo test init -- --test-threads=1`
- [ ] `cargo test --workspace`

Manual verification:

- [ ] install into a clean Codex config and confirm repo-local source reads are steered through SymForge-backed routes
- [ ] confirm markdown/config reads are not over-aggressively intercepted
- [ ] confirm token-savings and next-step hints are visible but not noisy

---

## Acceptance Criteria

- SymForge owns the high-frequency semantic code-inspection workflows without becoming a generic shell summarizer.
- Hook interception upgrades the default path for code reads/searches instead of merely suggesting alternatives.
- Sidecar workflow adapters are thin semantic wrappers over existing indexed queries, not duplicate engines.
- Client guidance and init output make raw shell inspection a fallback, not a peer default.
- Fallback behavior is explicit, deterministic, and points back into SymForge.
- Adoption improvements are measurable through concrete workflow metrics and tests.
