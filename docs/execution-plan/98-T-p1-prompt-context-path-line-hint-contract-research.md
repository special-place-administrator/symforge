---
doc_type: task
task_id: 98
title: P1 prompt_context path:line hint contract research
status: in_progress
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 97-T-p1-prompt-context-symbol-line-hint-shell.md
next_task: 99-T-p1-prompt-context-path-line-hint-shell.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 98: P1 Prompt Context Path:Line Hint Contract Research

## Objective

- define the smallest prompt-context follow-up contract that lets common `path:line` phrasing feed the combined file+symbol exact-selector path

## Why This Exists

- task 97 adds explicit `line N` support, but many prompts naturally encode the same information as `src/file.rs:42`
- prompt-context already trusts the exact file hint when it appears verbatim, so this is the smallest next ergonomic bridge
- this slice can improve prompt-driven disambiguation without opening global number parsing

## Read Before Work

- [96-R-p1-prompt-context-symbol-line-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/96-R-p1-prompt-context-symbol-line-hint-contract-research.md)
- [97-T-p1-prompt-context-symbol-line-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/97-T-p1-prompt-context-symbol-line-hint-shell.md)
- [95-T-p1-prompt-context-symbol-file-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/95-T-p1-prompt-context-symbol-file-hint-shell.md)

## Expected Touch Points

- `docs/execution-plan/98-T-p1-prompt-context-path-line-hint-contract-research.md`
- `docs/execution-plan/98-R-p1-prompt-context-path-line-hint-contract-research.md`
- `docs/execution-plan/99-T-p1-prompt-context-path-line-hint-shell.md`

## Deliverable

- a small research task that defines the first prompt-context `path:line` contract and authors the next execution slice

## Done When

- the accepted `path:line` shape is explicit
- the relationship to the existing file-hint and `symbol_line` flows is clear
- the next implementation slice is recoverable from disk

## Completion Notes

- pending

## Carry Forward To Next Task

Next task:

- `99-T-p1-prompt-context-path-line-hint-shell.md`

Carry forward:

- anchor colon-based parsing to the resolved file hint
- preserve existing `line N` behavior
- keep this slice separate from broader prompt grammar work

Open points:

- OPEN: whether the first shell should support only exact-path matches or also basename-derived `file.rs:42`
