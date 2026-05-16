# Prompt: Author /goal-Ready Tasks For Call-Time Capability Resolution

Use this document as the prompt for GPT-5.5 Pro. Its job is to create the task files that future agents can run with `/goal path/to/file.md`. It should not implement the feature itself.

## What Was Inspected

No literal local `/goal` skill file was found in this workspace or in `C:\Users\poslj\.codex` / `C:\Users\poslj\.agents`.

What does exist:

- `C:\Users\poslj\.codex\config.toml` has `[features] goals = true`.
- The active goal system stores a raw long-form objective; no separate task-file schema was found.
- The strongest executable local task format is the GSD PLAN schema at `.codex/get-shit-done/templates/phase-prompt.md`.
- GSD validates plan structure with `.codex/get-shit-done/bin/gsd-tools.cjs verify plan-structure <file>`.

Therefore, create files that are both:

1. Self-contained enough for `/goal path/to/file.md`.
2. Structurally compatible with the GSD PLAN format, so another agent can execute them deterministically.

## Your Assignment

Create a task prompt pack for implementing **Call-Time Capability Resolution + Derived Store Policy** in SymForge.

Do not change production code. Do not implement the feature. Only create planning/task files.

Recommended output directory:

```text
docs/plans/2026-05-16-call-time-capability-resolution/
```

Create:

- `README.md` - overview, execution order, dependency graph, and how to run each task with `/goal`.
- `call_time_capability_resolution_task01_contract_and_docs.md`
- `call_time_capability_resolution_task02_capability_evidence_foundation.md`
- `call_time_capability_resolution_task03_frecency_call_time_resolution.md`
- `call_time_capability_resolution_task04_cochange_lazy_prepare.md`
- `call_time_capability_resolution_task05_worktree_and_debug_explain.md`
- `call_time_capability_resolution_task06_health_visibility_and_integration.md`

You may split or merge if repo inspection proves a better boundary, but keep each file small enough for one agent to finish in one focused session.

## Source Material To Read First

Read these repo files before authoring the tasks:

- `AGENTS.md`
- `README.md`
- `docs/ideas/2026-05-16-capability-router-scoped-index-ideation-brief.md`
- `docs/plans/2026-05-15-symforge-post-h-roadmap.md`
- `.codex/get-shit-done/templates/phase-prompt.md`
- `.codex/get-shit-done/workflows/execute-plan.md`

Also inspect relevant implementation areas before naming task file ownership:

- `src/protocol/tools.rs`
- `src/live_index/frecency.rs`
- `src/live_index/persist.rs`
- `src/live_index/coupling/lifecycle.rs`
- `src/worktree.rs`
- `src/protocol/edit_hooks.rs`
- `src/protocol/ranking.rs` or the current ranking-response equivalent, if present
- tests touching `search_files`, frecency, coupling, worktree edits, ranking debug output, and health output

Use SymForge/code-intelligence tooling first where available. Fall back to raw reads only for exact source or docs.

## Product Direction To Encode

The task pack must implement the GPT-5.5 Pro recommendation:

- Do **not** build a multi-process router or multi-tenant SymForge swarm yet.
- Do **not** add a broad generic `scope` parameter in the first slice.
- Implement call-time capability resolution inside the existing in-process SymForge server.
- Environment variables should be policy overrides or defaults, not silent prerequisites for advertised tool behavior.
- If a tool call requests a capability, SymForge must do one of four things:
  - apply it,
  - prepare it and say so,
  - explain why it is unavailable,
  - explain that policy disabled it.

Core capabilities:

- Frecency: collect lightweight bumps by default where safe; use frecency ranking only when `rank_by="frecency"` or policy default requests it. Env/config can disable or change persistence policy.
- Co-change: do not build the co-change store eagerly on daemon start by default. On `rank_by="path+cochange"`, lazily prepare or report fallback state with evidence.
- Worktree-aware edits: `working_directory` should be a call-time opt-in with strict validation. Env/config should only disable policy if necessary.
- Debug ranking: expose call-time explain/debug output, such as `debug_ranking=true` or `explain=["ranking"]`. Env/config can default it on, but should not be the only way.
- Health/capabilities: make capability status visible in `health` or an equivalent capability-status surface.

## Requirements

Use these IDs in every task file frontmatter. Each task must reference at least one requirement.

- `CCR-1`: Requested capabilities are honored at call time or return explicit unavailable/disabled evidence.
- `CCR-2`: Env vars are policy/default overrides, not silent feature gates for normal requested behavior.
- `CCR-3`: Frecency has safe default bump collection and deterministic `rank_by="frecency"` behavior.
- `CCR-4`: Co-change ranking uses lazy bounded preparation or clear fallback evidence on first use.
- `CCR-5`: Worktree routing works from `working_directory` without requiring `SYMFORGE_WORKTREE_AWARE=1`, unless policy disables it.
- `CCR-6`: Ranking debug information is available via call-time request without requiring `SYMFORGE_DEBUG_RANKING=1`.
- `CCR-7`: Health/capability visibility reports enabled, disabled, unavailable, preparing, ready, stale, and fallback states where relevant.
- `CCR-8`: Documentation explains env vars as operational policy knobs, including disable/default-on/persistence semantics.
- `CCR-9`: Tests prove call-time behavior for requested capabilities with env vars unset.
- `CCR-10`: The design preserves local-first, in-process read-path performance and avoids startup-heavy derived-store work.

## Required Task File Format

Each task file must be directly runnable as:

```text
/goal docs/plans/2026-05-16-call-time-capability-resolution/<task-file>.md
```

Each file must use this structure:

```markdown
---
phase: 3g-call-time-capability-resolution
plan: NN
type: execute
wave: N
depends_on: []
files_modified: []
autonomous: true
requirements: [CCR-X, CCR-Y]
user_setup: []
must_haves:
  truths:
    - "Observable behavior that must be true after this task."
  artifacts:
    - path: "src/path/to/file.rs"
      provides: "What this file must provide."
      contains: "Important symbol or string pattern."
  key_links:
    - from: "src/source.rs"
      to: "src/target.rs"
      via: "How the implementation connects them."
      pattern: "regex-or-plain-pattern"
---

<objective>
One concise paragraph stating exactly what this task accomplishes.

Purpose: Why this task matters for call-time capability resolution.
Output: The concrete artifacts or behavior produced.
</objective>

<execution_context>
@./.codex/get-shit-done/workflows/execute-plan.md
@./.codex/get-shit-done/templates/summary.md
</execution_context>

<context>
@AGENTS.md
@README.md
@docs/ideas/2026-05-16-capability-router-scoped-index-ideation-brief.md
@docs/plans/2026-05-15-symforge-post-h-roadmap.md
@src/relevant/file.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Action-oriented name</name>
  <files>exact/file.rs, tests/exact_test.rs</files>
  <action>Specific implementation instructions, including what to avoid and why.</action>
  <verify>Exact command or check.</verify>
  <done>Measurable acceptance criteria.</done>
</task>

</tasks>

<verification>
Before declaring this goal complete:
- [ ] Run the task-specific tests.
- [ ] Run `cargo check`.
- [ ] If behavior touches shared Rust paths, run `cargo test --all-targets -- --test-threads=1`.
- [ ] If release-facing behavior changes, run `cargo build --release`.
</verification>

<success_criteria>
- All listed tasks are complete.
- All verification checks pass or any skipped check is explicitly justified.
- The task creates real behavior, not a stub, fake-success path, TODO, or silent fallback.
- Capability behavior is observable through tests, tool response evidence, or health/status output.
</success_criteria>

<output>
After completion, summarize changed files, verification output, and any follow-up requirements.
</output>
```

Do not leave `requirements`, `must_haves.truths`, `must_haves.artifacts`, or `files_modified` empty.

## Scoping Rules

Each task file should contain 1-3 `<task>` blocks.

Do not make one giant plan. Avoid overlapping write sets between same-wave tasks.

Use dependencies deliberately:

- Task 01 can be docs/ADR/product contract only.
- Task 02 should create shared capability evidence/policy types before feature-specific conversions.
- Task 03 should depend on Task 02 if it consumes shared evidence types.
- Task 04 should depend on Task 02 if it consumes shared evidence types.
- Task 05 may depend on Task 02 if shared evidence/policy applies to worktree/debug output.
- Task 06 should depend on all implementation tasks and focus on health/status/docs/integration verification.

Suggested waves:

- Wave 1: Task 01 and, if disjoint enough, Task 02.
- Wave 2: Task 03 and Task 04 in parallel if their write sets do not overlap.
- Wave 3: Task 05.
- Wave 4: Task 06.

If repo inspection shows the files overlap, make them sequential.

## Task Content Expectations

Task 01 should ask the implementing agent to:

- Add or update a decision record, preferably `docs/decisions/0016-call-time-capability-resolution.md` if numbering is available.
- Update README/env-var wording so env vars are described as policy/default overrides.
- Add roadmap entry if appropriate.
- Avoid code behavior changes unless needed for docs tests.

Task 02 should ask the implementing agent to:

- Add a small capability evidence/policy model in the appropriate Rust module.
- Prefer existing project module boundaries.
- Define statuses such as `ready`, `disabled`, `unavailable`, `preparing`, `fallback`, and `stale` if they fit the codebase.
- Add focused unit tests for serialization/response shaping if exposed.

Task 03 should ask the implementing agent to:

- Convert frecency from env-gated advertised behavior to call-time resolution.
- Ensure `rank_by="frecency"` with env vars unset produces deterministic behavior or explicit evidence.
- Keep collection cheap and local-first.
- Test env-unset behavior and policy-disabled behavior.

Task 04 should ask the implementing agent to:

- Convert co-change ranking to lazy bounded prepare/fallback behavior.
- Ensure `rank_by="path+cochange"` with env vars unset does not silently ignore the requested signal.
- Test fallback/preparing/ready evidence.

Task 05 should ask the implementing agent to:

- Let edit tools honor validated `working_directory` at call time without requiring `SYMFORGE_WORKTREE_AWARE=1`, unless policy disables it.
- Add call-time ranking explain/debug output without requiring `SYMFORGE_DEBUG_RANKING=1`.
- Keep the response shape backward compatible unless the task explicitly documents a versioned change.

Task 06 should ask the implementing agent to:

- Surface capability policy/status in health or equivalent status output.
- Add integration tests that prove env vars unset still allow call-time requested behavior or explicit unavailable/disabled evidence.
- Run full Rust verification.
- Update docs to match actual final behavior.

## Validation Of Your Output

After creating the task files, validate the plan files yourself:

```powershell
node .\.codex\get-shit-done\bin\gsd-tools.cjs verify plan-structure docs\plans\2026-05-16-call-time-capability-resolution\call_time_capability_resolution_task01_contract_and_docs.md
node .\.codex\get-shit-done\bin\gsd-tools.cjs verify references docs\plans\2026-05-16-call-time-capability-resolution\call_time_capability_resolution_task01_contract_and_docs.md
```

Run the equivalent checks for every generated task file.

Also run:

```powershell
git diff --check
```

If validation fails, fix the task files before reporting completion.

## Final Response Expected From You

Report:

- Files created.
- The dependency/wave order.
- Any assumptions made because a code path or `/goal` parser was not discoverable.
- The validation commands you ran and their results.

Do not claim the implementation is done. The output of this assignment is the task prompt pack only.
