---
doc_type: task
task_id: 99
title: P1 prompt_context path:line hint shell
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 98-T-p1-prompt-context-path-line-hint-contract-research.md
next_task: 100-T-p1-prompt-context-basename-line-hint-contract-research.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 99: P1 Prompt Context Path:Line Hint Shell

## Objective

- let prompt-context consume a resolved `path:line` hint for combined file+symbol prompts while preserving the explicit `line N` path from task 97

## Why This Exists

- task 98 narrows the next prompt-context ergonomic improvement to path-anchored `path:line`
- prompt-context already resolves exact file hints, so this is the smallest safe way to support a common coding prompt shape

## Read Before Work

- [98-R-p1-prompt-context-path-line-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/98-R-p1-prompt-context-path-line-hint-contract-research.md)
- [98-T-p1-prompt-context-path-line-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/98-T-p1-prompt-context-path-line-hint-contract-research.md)
- [97-T-p1-prompt-context-symbol-line-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/97-T-p1-prompt-context-symbol-line-hint-shell.md)

## Expected Touch Points

- `src/sidecar/handlers.rs`
- `tests/sidecar_integration.rs`

## Deliverable

- a prompt-context shell that accepts a resolved `path:line` hint for combined file+symbol prompts and feeds it into exact-selector symbol context

## Done When

- combined prompts with an exact `path:line` hint resolve through `symbol_line`
- existing explicit `line N` support stays intact
- unrelated colon numbers do not get treated as line hints
- focused tests cover the new `path:line` path and its fallback behavior

## Completion Notes

- extended prompt-context line-hint extraction so `<resolved-path>:<line>` feeds `symbol_line` for the combined file+symbol exact-selector path
- preserved the existing explicit `line N` flow and the ambiguity fallback when no usable line hint exists
- added focused handler tests for `path:line` success and unrelated colon-number rejection
- added sidecar endpoint coverage for the `path:line` route

## Carry Forward To Next Task

Next task:

- `100-T-p1-prompt-context-basename-line-hint-contract-research.md`

Carry forward:

- keep colon parsing anchored to the resolved file hint
- preserve the current exact-selector fallback when no usable line hint exists
- avoid broadening this slice into full prompt grammar parsing
- if the next slice expands to basename-derived hints, require the basename to already resolve uniquely before treating `file.rs:line` as actionable

Open points:

- OPEN: whether prompt-context should also accept unique basename-derived `file.rs:42` hints
