---
doc_type: task
task_id: 92
title: P1 get_symbol_context exact selector contract research
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 91-T-p1-get-context-bundle-exact-selector-shell.md
next_task: 93-T-p1-get-symbol-context-exact-selector-shell.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 92: P1 Get Symbol Context Exact Selector Contract Research

## Objective

- define the smallest exact-selector follow-up contract that lets `search_symbols` feed `get_symbol_context` without reopening stable symbol-id work

## Why This Exists

- `get_symbol_context` is still a useful compact follow-up surface, but today it is fully global-name-driven
- task 91 fixed the same ambiguity class for `get_context_bundle`, so the next small win is to align the grouped symbol-context summary with the exact-selector lane
- the tool is sidecar-backed, which means the contract needs to be explicit before changing both protocol and sidecar plumbing

## Read Before Work

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [88-R-p1-find-references-exact-selector-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/88-R-p1-find-references-exact-selector-contract-research.md)
- [89-T-p1-find-references-exact-selector-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/89-T-p1-find-references-exact-selector-shell.md)
- [90-R-p1-get-context-bundle-exact-selector-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/90-R-p1-get-context-bundle-exact-selector-contract-research.md)
- [91-T-p1-get-context-bundle-exact-selector-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/91-T-p1-get-context-bundle-exact-selector-shell.md)

## Expected Touch Points

- `docs/execution-plan/92-T-p1-get-symbol-context-exact-selector-contract-research.md`
- `docs/execution-plan/92-R-p1-get-symbol-context-exact-selector-contract-research.md`
- `docs/execution-plan/93-T-p1-get-symbol-context-exact-selector-shell.md`

## Deliverable

- a small research task that fixes the first exact-selector follow-up contract for `get_symbol_context` and authors the next execution slice

## Done When

- the first exact-selector contract for `get_symbol_context` is explicit
- the relationship between current `file` filtering and exact symbol selection is clear
- the next implementation slice is recoverable from disk

## Completion Notes

- added [92-R-p1-get-symbol-context-exact-selector-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/92-R-p1-get-symbol-context-exact-selector-contract-research.md)
- recommendation:
  - preserve current name-only behavior unless an exact selector is provided
  - add `path`, `symbol_kind`, and `symbol_line` so `search_symbols` output can chain directly into `get_symbol_context`
  - keep current `file` semantics as an output filter, not the symbol selector
  - reuse shared exact-selector query helpers from `find_references` / `get_context_bundle`
  - keep successful grouped output unchanged and use stable error strings for missing or ambiguous selectors
- authored the follow-on execution slice as `93-T-p1-get-symbol-context-exact-selector-shell.md`

## Carry Forward To Next Task

Next task:

- `93-T-p1-get-symbol-context-exact-selector-shell.md`

Carry forward:

- preserve the current compact grouped output shape and cap
- keep this research separate from stable symbol-id substrate work
- prefer reusing shared exact-selector query helpers over inventing sidecar-only matching rules

Resolved point:

- route the first shell through the sidecar handler but back it with shared exact-selector query helpers instead of sidecar-local matching logic
