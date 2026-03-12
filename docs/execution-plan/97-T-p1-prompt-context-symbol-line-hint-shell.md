---
doc_type: task
task_id: 97
title: P1 prompt_context symbol line hint shell
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 96-T-p1-prompt-context-symbol-line-hint-contract-research.md
next_task: 98-T-p1-prompt-context-path-line-hint-contract-research.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 97: P1 Prompt Context Symbol Line Hint Shell

## Objective

- let prompt-context feed an explicit line hint into the combined file+symbol exact-selector path while preserving current single-hint behavior

## Why This Exists

- task 96 narrows the next prompt-context disambiguation step to an explicit line hint instead of broader prompt parsing
- prompt-context already routes combined hints to exact-selector symbol context, so the next small win is to let explicit `line N` text drive `symbol_line`

## Read Before Work

- [96-R-p1-prompt-context-symbol-line-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/96-R-p1-prompt-context-symbol-line-hint-contract-research.md)
- [96-T-p1-prompt-context-symbol-line-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/96-T-p1-prompt-context-symbol-line-hint-contract-research.md)
- [95-T-p1-prompt-context-symbol-file-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/95-T-p1-prompt-context-symbol-file-hint-shell.md)

## Expected Touch Points

- `src/sidecar/handlers.rs`
- `tests/sidecar_integration.rs`

## Deliverable

- a prompt-context shell that accepts an explicit `line N` hint for combined file+symbol prompts and feeds it into exact-selector symbol context

## Done When

- combined file+symbol prompts with `line N` resolve through `symbol_line`
- combined prompts without `line N` keep the current stable ambiguity behavior
- file-only, symbol-only, and repo-map behavior stay unchanged
- focused tests cover the new line-hint path and its fallback behavior

## Completion Notes

- fed explicit `line N` hints into the combined file+symbol exact-selector prompt-context path via `symbol_line`
- preserved existing `file-only`, `symbol-only`, repo-map, and no-line ambiguity behavior
- added focused handler tests to prove `line N` disambiguates while unlabeled numbers do not
- added sidecar endpoint coverage for the new line-hint route

## Carry Forward To Next Task

Next task:

- `98-T-p1-prompt-context-path-line-hint-contract-research.md`

Carry forward:

- keep parsing explicit and narrow
- preserve the current exact-selector fallback when no line hint is provided
- avoid opening broader prompt parsing in this slice
- if the next slice expands line hints further, reuse the resolved file hint instead of parsing arbitrary colon numbers globally

Open points:

- OPEN: whether prompt-context should also accept `path:line` when the prompt already includes an exact file hint
