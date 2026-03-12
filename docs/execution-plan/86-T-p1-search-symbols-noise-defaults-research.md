---
doc_type: task
task_id: 86
title: P1 search_symbols noise defaults research
status: in_progress
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 85-T-p1-search-symbols-scope-filter-shell.md
next_task: 
created: 2026-03-12
updated: 2026-03-12
---
# Task 86: P1 Search Symbols Noise Defaults Research

## Objective

- determine the smallest safe follow-on contract for generated/test suppression in `search_symbols`

## Why This Exists

- task 85 added scoped public filters while explicitly preserving the current noise-permissive defaults
- backlog P1 still calls for generated/test suppression defaults, but that change affects result visibility rather than just scoping
- this deserves a small research slice before another public behavior change

## Read Before Work

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [67-R-phase1-dual-lane-option-defaults-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/67-R-phase1-dual-lane-option-defaults-research.md)
- [84-R-p1-search-symbols-scope-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/84-R-p1-search-symbols-scope-contract-research.md)
- [85-T-p1-search-symbols-scope-filter-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/85-T-p1-search-symbols-scope-filter-shell.md)

## Expected Touch Points

- `docs/execution-plan/86-T-p1-search-symbols-noise-defaults-research.md`
- likely a follow-on research note and implementation task docs

## Deliverable

- a small research task that fixes the next `search_symbols` noise-default contract and authors the follow-on execution slice

## Done When

- the next generated/test suppression contract is explicit
- default behavior versus explicit opt-in is clear
- the follow-on implementation task is recoverable from disk

## Completion Notes

- pending

## Carry Forward To Next Task

Next task:

- `TBD`

Carry forward:

- keep this research separate from exact-symbol identity work
- avoid mixing noise-default changes with ranking changes in the same slice
- preserve the new scope filter contract from task 85

Open points:

- OPEN: whether the next shell should change defaults only or also expose explicit `include_generated` / `include_tests` knobs
