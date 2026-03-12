---
doc_type: task
task_id: 84
title: P1 search_symbols scope contract research
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 83-T-phase3-search-text-match-semantics-shell.md
next_task: 85-T-p1-search-symbols-scope-filter-shell.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 84: P1 Search Symbols Scope Contract Research

## Objective

- define the smallest public `search_symbols` scope contract that materially reduces shell fallback without reopening exact-symbol identity work

## Why This Exists

- the code-lane `search_text` shell is now materially stronger after the Phase 3 semantics slice
- backlog P1 explicitly calls for `search_symbols` path/language/limit filters
- this is a smaller next step than starting the broader Phase 4 read-surface work

## Read Before Work

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [04-P-phase-plan.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/04-P-phase-plan.md)
- [66-T-phase1-shared-query-option-struct-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/66-T-phase1-shared-query-option-struct-shell.md)
- [68-T-phase1-explicit-current-tool-option-defaults-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/68-T-phase1-explicit-current-tool-option-defaults-shell.md)
- [83-T-phase3-search-text-match-semantics-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/83-T-phase3-search-text-match-semantics-shell.md)

## Expected Touch Points

- `docs/execution-plan/84-T-p1-search-symbols-scope-contract-research.md`
- likely follow-on research note and implementation task docs

## Deliverable

- a small research task that fixes the first scoped `search_symbols` contract and authors the follow-on implementation slice

## Done When

- the first `search_symbols` scope/filter contract is explicit
- path, language, and cap semantics are clear
- any interaction with generated/test defaults is called out explicitly
- the next implementation task is recoverable from disk

## Completion Notes

- added [84-R-p1-search-symbols-scope-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/84-R-p1-search-symbols-scope-contract-research.md)
- recommendation:
  - add `path_prefix`, `language`, and `limit` first
  - keep the current code-lane and noise-permissive defaults
  - preserve the current `kind` filter and output format
  - defer generated/test suppression to a separate follow-on slice
  - default `limit` to 50 and cap the first shell at 100
- authored the next execution slice as `85-T-p1-search-symbols-scope-filter-shell.md`

## Carry Forward To Next Task

Next task:

- `85-T-p1-search-symbols-scope-filter-shell.md`

Carry forward:

- keep this research separate from exact-symbol identity work in Phase 5
- prefer additive filters over output redesign
- preserve the current `kind` filter unless research finds a concrete conflict

Open points:

- OPEN: whether generated/test suppression for `search_symbols` should land immediately after the first scoped shell or wait for ranking work
