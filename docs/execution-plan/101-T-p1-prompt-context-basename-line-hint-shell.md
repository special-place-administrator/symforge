---
doc_type: task
task_id: 101
title: P1 prompt_context basename:line hint shell
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 100-T-p1-prompt-context-basename-line-hint-contract-research.md
next_task: 102-T-p1-prompt-context-extensionless-line-hint-contract-research.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 101: P1 Prompt Context Basename:Line Hint Shell

## Objective

- let prompt-context consume a unique basename-derived `file.rs:line` hint for combined file+symbol prompts while preserving exact-path `path:line` and explicit `line N`

## Why This Exists

- task 100 narrows the next prompt-context ergonomic improvement to basename-derived `:line` when basename resolution is already unique
- prompt-context already has the necessary basename-resolution guardrails, so this is the smallest safe follow-up

## Read Before Work

- [100-R-p1-prompt-context-basename-line-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/100-R-p1-prompt-context-basename-line-hint-contract-research.md)
- [100-T-p1-prompt-context-basename-line-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/100-T-p1-prompt-context-basename-line-hint-contract-research.md)
- [99-T-p1-prompt-context-path-line-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/99-T-p1-prompt-context-path-line-hint-shell.md)

## Expected Touch Points

- `src/sidecar/handlers.rs`
- `tests/sidecar_integration.rs`

## Deliverable

- a prompt-context shell that accepts unique basename-derived `file.rs:line` hints for combined file+symbol prompts and feeds them into exact-selector symbol context

## Done When

- unique basename `file.rs:line` prompts resolve through `symbol_line`
- ambiguous basenames do not activate basename-derived `:line`
- existing exact-path `path:line` and explicit `line N` support stay intact
- focused tests cover the new basename-derived path and its ambiguity guardrail

## Completion Notes

- extended prompt-context line-hint extraction so unique basename-derived `file.rs:line` hints feed `symbol_line` for the combined file+symbol exact-selector path
- preserved the existing exact-path `path:line`, explicit `line N`, and ambiguity-fallback behavior
- added focused handler tests for basename-derived success and ambiguous-basename fallback
- added sidecar endpoint coverage for the new basename-derived route

## Carry Forward To Next Task

Next task:

- `102-T-p1-prompt-context-extensionless-line-hint-contract-research.md`

Carry forward:

- keep basename-derived `:line` gated by unique file resolution
- preserve the current exact-selector fallback when no usable line hint exists
- avoid broadening this slice into generic filename parsing
- if the next slice expands to extensionless aliases, require the alias to map uniquely onto the same resolved file hint

Open points:

- OPEN: whether prompt-context should also accept unique extensionless aliases like `db:2`
