---
doc_type: task
task_id: 87
title: P1 search_symbols noise defaults shell
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 86-T-p1-search-symbols-noise-defaults-research.md
next_task: 88-T-p1-find-references-exact-selector-contract-research.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 87: P1 Search Symbols Noise Defaults Shell

## Objective

- make current-code `search_symbols` hide generated/test symbol noise by default while preserving explicit opt-in access

## Why This Exists

- task 85 added public scoping for `search_symbols`, but intentionally kept permissive noise defaults
- task 86 fixed the smallest safe follow-on contract: align symbol search with current `search_text` defaults and expose the two needed escape hatches

## Read Before Work

- [86-R-p1-search-symbols-noise-defaults-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/86-R-p1-search-symbols-noise-defaults-research.md)
- [86-T-p1-search-symbols-noise-defaults-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/86-T-p1-search-symbols-noise-defaults-research.md)
- [67-R-phase1-dual-lane-option-defaults-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/67-R-phase1-dual-lane-option-defaults-research.md)
- [68-T-phase1-explicit-current-tool-option-defaults-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/68-T-phase1-explicit-current-tool-option-defaults-shell.md)
- [85-T-p1-search-symbols-scope-filter-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/85-T-p1-search-symbols-scope-filter-shell.md)

## Expected Touch Points

- `src/protocol/tools.rs`
- `src/live_index/search.rs`

## Deliverable

- a `search_symbols` shell that hides generated/test hits by default, keeps vendor visible, and exposes explicit generated/test opt-in knobs

## Done When

- `search_symbols` defaults suppress generated and test hits in current code-lane searches
- callers can opt back into generated hits with `include_generated=true`
- callers can opt back into test hits with `include_tests=true`
- vendor visibility stays unchanged
- current scope, language, limit, and kind filters still compose correctly
- focused tests cover the new defaults and explicit opt-ins

## Completion Notes

- changed current-code `search_symbols` defaults to hide generated and test hits while keeping vendor visibility unchanged
- extended the public `search_symbols` input with `include_generated` and `include_tests` as explicit opt-in overrides
- aligned the tool-layer symbol-search adapter with the current `search_text` generated/test contract
- added focused unit and tool tests for default suppression and explicit generated/test inclusion
- verification run for this task:
  - `cargo test search_symbols -- --nocapture`
  - `cargo test`

## Carry Forward To Next Task

Next task:

- `88-T-p1-find-references-exact-selector-contract-research.md`

Carry forward:

- keep formatter output unchanged
- keep ranking changes out of this slice
- keep exact-symbol identity work separate

Open points:

- OPEN: whether a later slice should expose `include_vendor` or tighten vendor defaults
