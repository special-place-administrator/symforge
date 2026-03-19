# RTK-Style Adoption Rollout Plan

> **For agentic workers:** REQUIRED: Execute this as a staged rollout. Do not merge multiple PR scopes unless a later PR is blocked on an earlier PR’s omission. Keep each PR independently testable and revertable.

**Goal:** Convert the RTK-style adoption strategy into a methodical execution sequence with exact PR boundaries, file ownership, verification, and rollback rules.

**Primary Plan:** `docs/superpowers/plans/2026-03-19-rtk-style-adoption-integration.md`

**Success Condition:** After rollout, newly installed and active SymForge clients should default to SymForge-backed code inspection flows for high-confidence source reads/searches, while remaining explicit and conservative for docs/config and non-semantic shell tasks.

---

## Rollout Principles

1. Keep each PR behaviorally narrow.
2. Do not mix product-positioning changes with routing logic unless they are tightly coupled.
3. Preserve fail-open behavior at every stage.
4. Add tests in the same PR as the behavioral change.
5. Every PR must leave the system in a valid state even if the next PR never ships.
6. Do not advance to the next PR until the previous PR passes both automated checks and a short manual canary check.
7. Guidance-generation changes must lag behind runtime behavior changes unless the guidance is purely descriptive.

---

## Stage Gates

### Gate A: After PR 2

Before starting PR 3, manually validate on a real client session that:

- source-code `Read` is being steered toward SymForge-backed output
- source-code `Grep` is being steered toward SymForge-backed output
- markdown/config reads are not over-routed
- fail-open JSON remains valid and understandable

If this gate fails, fix PR 2 or revert it. Do not move to sidecar expansion.

### Gate B: After PR 3

Before starting PR 4, validate:

- direct sidecar routes and daemon session routes agree
- no ambiguity regressions were introduced in prompt-context flows
- latency remains acceptable for hook-triggered routes

### Gate C: Before PR 5

Before shipping stronger init guidance:

- canonical workflow/tool choices must be stable
- no major naming or endpoint churn should remain
- manual canary runs should show the routing behavior users are about to be instructed to rely on

### Gate D: Final Milestone Gate

Before declaring the rollout complete:

- `cargo check --workspace`
- `cargo test --workspace`
- manual clean-install verification on at least one real client flow
- manual comparison against the baseline problem statement: first-contact source reads/searches should now prefer SymForge-backed paths

---

## PR Sequence

### PR 1: Workflow Ownership and Boundary Declaration

**Purpose:** Lock the product boundary before changing routing behavior.

**Why first:** This reduces thrash. Without a hard ownership line, later hook and sidecar work can drift into “generic shell summarizer” territory.

**Files**
- `README.md`
- `src/cli/hook.rs`

**Changes**
- Document the list of SymForge-owned workflows:
  - source-code read
  - source-code search
  - repo-start overview
  - symbol lookup
  - prompt-context enrichment
  - post-edit impact
  - changed-symbol/code-review flows
- Make the boundary explicit:
  - shell remains primary for process control, package management, test execution, logs, system inspection
  - docs/config raw reads remain legitimate when literal contents are the goal
- Add a brief “RTK-style adoption mechanics yes, generic shell summarizer no” positioning note
- Add comments or small internal constants in `src/cli/hook.rs` that define the workflow categories to be used by later PRs

**Tests**
- `cargo test hook::tests::test_pre_tool_suggestion_read_source_suggests_get_file_context -- --test-threads=1`
- `cargo test hook::tests::test_pre_tool_suggestion_read_markdown_suggests_symforge -- --test-threads=1`
- `cargo test hook::tests::test_is_non_source_path_allows_config_files -- --test-threads=1`

**Rollback Boundary**
- Safe to revert independently.
- Reverting this PR should remove only product-positioning and early workflow scaffolding, not any transport or routing behavior.

**Entry Criteria for PR 2**
- ownership language is settled enough that hook routing can target concrete workflows
- no unresolved disagreement remains about what SymForge should leave to shell/process tooling

---

### PR 2: Hook Workflow Classifier and Stronger High-Confidence Routing

**Purpose:** Upgrade hooks from suggestion-heavy nudges to workflow-aware routing for obvious source-code cases.

**Why second:** This is the highest-value adoption change and should land before any new sidecar API surface.

**Files**
- `src/cli/hook.rs`
- `tests/hook_enrichment_integration.rs`

**Changes**
- Introduce a workflow classifier inside `src/cli/hook.rs`
  - likely categories:
    - `SourceRead`
    - `SourceSearch`
    - `RepoStart`
    - `PromptContext`
    - `PostEditImpact`
    - `PassThrough`
- Tighten high-confidence routing:
  - source `Read` requests prefer outline/file-context routes
  - source `Grep` requests prefer `search_text`
  - post-edit/write continues to route into impact
- Preserve fail-open behavior:
  - if confidence is low, return passthrough/fail-open JSON
  - include rationale and one recommended next SymForge action
- Avoid over-routing:
  - markdown/config exact reads should remain conservative
  - non-source and non-indexed content should not be forcibly semanticized

**Tests**
- Existing unit coverage in `src/cli/hook.rs`:
  - endpoint mapping tests
  - pre-tool suggestion tests
  - non-source classification tests
- Integration coverage in `tests/hook_enrichment_integration.rs`:
  - `test_read_hook_returns_formatted_outline`
  - `test_read_hook_noop_for_missing_file`
  - `test_grep_hook_annotates_matches`
  - `test_session_start_repo_map`
  - add new cases for:
    - source read routed with stronger confidence
    - markdown/config read intentionally not over-routed
    - fallback rationale visible

**Verification**
- `cargo test hook -- --test-threads=1`
- `cargo test hook_enrichment_integration -- --test-threads=1`

**Rollback Boundary**
- Safe to revert independently if routing becomes too aggressive.
- Revert target is limited to the hook layer only.
- No sidecar API additions should be required yet.

**Entry Criteria for PR 3**
- Gate A passes
- hook routing categories feel stable enough to justify dedicated sidecar adapter names
- no evidence that the desired behavior can be achieved sufficiently with hooks alone

---

### PR 3: Sidecar Workflow Adapters and Session-Scoped Exposure

**Purpose:** Formalize workflow adapters so hook routes land on stable, semantically-backed endpoints instead of ad hoc handler assumptions.

**Why third:** Once hook intent is clarified, the sidecar should expose explicit workflow surfaces that remain thin wrappers over semantic primitives.

**Files**
- `src/sidecar/handlers.rs`
- `src/sidecar/router.rs`
- `src/daemon.rs`
- `tests/sidecar_integration.rs`

**Changes**
- Add or formalize sidecar adapters for:
  - source read / outline
  - search hit expansion
  - prompt-context narrowing
  - post-edit impact summary
  - repo-start quick map
- Ensure adapters are thin delegations into existing semantic operations, not separate search/ranking engines
- Expose new endpoints under session-scoped daemon routes where appropriate
- Align sidecar behavior and daemon-scoped sidecar behavior

**File Ownership Notes**
- `src/sidecar/handlers.rs`: owns rendering decisions and endpoint-specific glue
- `src/sidecar/router.rs`: owns local route declarations only
- `src/daemon.rs`: owns session-scoped exposure and per-session state wiring
- `tests/sidecar_integration.rs`: owns black-box behavior verification

**Tests**
- Existing sidecar integration coverage:
  - outline endpoint
  - repo map endpoint
  - prompt-context endpoint family
  - output JSON validity
- Add:
  - route parity tests between direct sidecar and daemon session sidecar
  - fallback rationale tests for ambiguous/unsupported requests
  - token-savings / compactness assertions where reasonable

**Verification**
- `cargo test sidecar -- --test-threads=1`
- `cargo test daemon -- --test-threads=1`

**Rollback Boundary**
- Safe to revert independently if new adapter endpoints are noisy or unstable.
- Hooks from PR 2 should still fail open even if these routes are removed.

**Entry Criteria for PR 4**
- Gate B passes
- sidecar adapters are stable enough that tool-surface guidance can reference them confidently
- no remaining uncertainty about whether the workflow adapters are thin wrappers vs duplicate logic

---

### PR 4: MCP Tool Surface Steering and Response UX

**Purpose:** Make the MCP layer itself nudge models better even when hooks are absent.

**Why fourth:** This is the “direct MCP use still gets smarter” PR. It is complementary to hooks, not a substitute.

**Files**
- `src/protocol/tools.rs`
- `src/protocol/format.rs`
- `src/protocol/resources.rs` (review, modify only if needed)

**Changes**
- Strengthen descriptions for:
  - `get_file_context`
  - `get_repo_map`
  - `get_symbol`
  - `search_text`
  - `search_symbols`
  - `get_file_content`
- Add:
  - “prefer this over raw file read for code understanding”
  - workflow hints
  - token-efficiency framing
- Add compact next-step hints in responses where helpful
- Improve degraded/fallback messaging

**Tests**
- `src/protocol/tools.rs` tests for description stability and routing affordances
- `src/protocol/format.rs` tests for:
  - next-step hint placement
  - fallback guidance
  - noise control
- Focus on existing tests around:
  - `get_file_context`
  - `search_text`
  - `health`
  - `validate_file_syntax`

**Verification**
- `cargo test protocol::tools -- --test-threads=1`
- `cargo test protocol::format -- --test-threads=1`

**Rollback Boundary**
- Safe to revert independently if the response UX becomes verbose or unstable.
- Does not alter core query semantics.

**Entry Criteria for PR 5**
- Gate C passes
- canonical tool names, workflow rules, and preferred routing language are stable enough to encode into generated client guidance

---

### PR 5: Init and Client Rollout Hardening

**Purpose:** Make new installs and refreshed installs inherit the better behavior automatically.

**Why fifth:** This should ship after behavior and routing are stable, otherwise generated guidance may freeze the wrong rules into user configs.

**Files**
- `src/cli/init.rs`
- `tests/init_integration.rs`

**Changes**
- Upgrade generated guidance blocks for Codex, Claude, Gemini, and Kilo-style clients
- Make source inspection rules more explicit:
  - SymForge is primary for repo-local code inspection
  - raw reads are fallback for exact docs/config or non-indexed files
- Ensure allow-lists include all canonical entry points needed by the new workflow model
- Optionally add or document project-local rule emission strategy

**Tests**
- Existing init integration tests:
  - Codex registration
  - idempotency
  - guidance writing
  - client isolation
- Add:
  - stronger routing language assertions
  - project-level fallback/reference assertions if new docs are emitted

**Verification**
- `cargo test init -- --test-threads=1`
- targeted `codex`/`claude` registration smoke checks if available locally

**Rollback Boundary**
- Safe to revert independently if generated guidance proves too aggressive.
- Does not affect core MCP runtime or sidecar behavior.

**Entry Criteria for PR 6**
- runtime behavior is already stable enough that measurement reflects product reality, not churn
- generated guidance has settled so adoption metrics are not immediately invalidated by another init rewrite

---

### PR 6: Metrics, Eval Harness, and Adoption Proof

**Purpose:** Prove the rollout changed behavior rather than assuming it did.

**Why last:** Measurement design should be considered early, but the observable behavior must exist first.

**Files**
- `src/cli/hook.rs`
- `src/protocol/format.rs`
- possibly `src/sidecar/handlers.rs`
- test/eval files as needed

**Changes**
- Add lightweight counters or observability for:
  - hook interception by workflow
  - semantic route accepted vs fallback
  - token savings by workflow
  - first-contact repo-start behavior
- Surface useful summaries in `health` or debug output
- Build a small acceptance eval set for:
  - source read
  - source search
  - repo start
  - post-edit impact
  - prompt-context narrowing

**Tests**
- targeted unit tests for counters/formatting
- small integration checks for expected workflow categorization

**Verification**
- `cargo test --workspace`
- manual eval run against a clean-installed client setup

**Rollback Boundary**
- Metrics may be reverted independently if noisy.
- Functional routing from prior PRs should continue working unchanged.

---

## Exact File Matrix by PR

| PR | Primary Files | Test Files |
|----|---------------|------------|
| PR 1 | `README.md`, `src/cli/hook.rs` | existing hook unit tests |
| PR 2 | `src/cli/hook.rs` | `tests/hook_enrichment_integration.rs` |
| PR 3 | `src/sidecar/handlers.rs`, `src/sidecar/router.rs`, `src/daemon.rs` | `tests/sidecar_integration.rs` |
| PR 4 | `src/protocol/tools.rs`, `src/protocol/format.rs`, maybe `src/protocol/resources.rs` | protocol tool/format inline tests |
| PR 5 | `src/cli/init.rs` | `tests/init_integration.rs` |
| PR 6 | hook/format/sidecar observability files | targeted tests or eval assets |

---

## Test Ladder

Run this progressively; do not jump straight to full workspace every time.

### PR 1
- `cargo test hook::tests -- --test-threads=1`

### PR 2
- `cargo test hook -- --test-threads=1`
- `cargo test hook_enrichment_integration -- --test-threads=1`

### PR 3
- `cargo test sidecar -- --test-threads=1`
- `cargo test daemon -- --test-threads=1`

### PR 4
- `cargo test protocol::tools -- --test-threads=1`
- `cargo test protocol::format -- --test-threads=1`

### PR 5
- `cargo test init -- --test-threads=1`

### PR 6
- targeted metrics/eval tests
- `cargo test --workspace`

### Milestone Gate
- `cargo fmt`
- `cargo check --workspace`
- `cargo test --workspace`

---

## Rollback Strategy

### Hard Rule

If a PR causes over-interception, confusing fallback behavior, or user-hostile routing, revert the current PR only. Do not immediately patch forward inside the same rollout unless the failure is trivial and fully understood.

### Practical Rollback Boundaries

- **PR 1 rollback:** removes positioning/ownership scaffolding only
- **PR 2 rollback:** returns hook behavior to suggestion-heavy mode
- **PR 3 rollback:** removes workflow adapter endpoints but keeps hook fail-open safety
- **PR 4 rollback:** returns MCP UX to prior descriptions/formatting without touching routing
- **PR 5 rollback:** restores prior init guidance/allow-list generation
- **PR 6 rollback:** removes metrics/eval surfacing only

### Stop Conditions

Pause the rollout if any of the following occurs:

- docs/config reads get over-routed into code-oriented handlers
- fallback behavior becomes opaque
- hook output validity or latency regresses materially
- daemon/session-scoped sidecar behavior diverges from direct sidecar behavior
- tool descriptions become so verbose that they reduce usability

---

## Recommended Immediate Next Step

Start with **PR 1 and PR 2 only**.

Reason:
- they are the smallest slice that can materially change adoption behavior
- they do not yet require expanding the sidecar API surface
- they keep rollback simple if the routing heuristics are too aggressive
 - they establish whether stronger hook routing is enough to change behavior materially before we widen the architecture

Once PR 2 passes Gate A, proceed to PR 3.
