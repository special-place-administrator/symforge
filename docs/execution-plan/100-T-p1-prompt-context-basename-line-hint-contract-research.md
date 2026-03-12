---
doc_type: task
task_id: 100
title: P1 prompt_context basename:line hint contract research
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 99-T-p1-prompt-context-path-line-hint-shell.md
next_task: 101-T-p1-prompt-context-basename-line-hint-shell.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 100: P1 Prompt Context Basename:Line Hint Contract Research

## Objective

- define the smallest prompt-context follow-up contract that lets unique basename-derived `file.rs:line` prompts feed the combined file+symbol exact-selector path

## Why This Exists

- task 99 adds exact-path `path:line`, but many prompts still use unique basenames like `db.rs:2`
- prompt-context already has basename resolution with ambiguity protection, so this is the smallest next ergonomic bridge
- this slice can improve prompt-driven disambiguation without weakening the current file-hint guardrails

## Read Before Work

- [98-R-p1-prompt-context-path-line-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/98-R-p1-prompt-context-path-line-hint-contract-research.md)
- [99-T-p1-prompt-context-path-line-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/99-T-p1-prompt-context-path-line-hint-shell.md)
- [95-T-p1-prompt-context-symbol-file-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/95-T-p1-prompt-context-symbol-file-hint-shell.md)

## Expected Touch Points

- `docs/execution-plan/100-T-p1-prompt-context-basename-line-hint-contract-research.md`
- `docs/execution-plan/100-R-p1-prompt-context-basename-line-hint-contract-research.md`
- `docs/execution-plan/101-T-p1-prompt-context-basename-line-hint-shell.md`

## Deliverable

- a small research task that defines the first unique-basename `file.rs:line` contract and authors the next execution slice

## Done When

- the accepted basename-derived `:line` shape is explicit
- the relationship to existing basename resolution and `symbol_line` flows is clear
- the next implementation slice is recoverable from disk

## Completion Notes

- added [100-R-p1-prompt-context-basename-line-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/100-R-p1-prompt-context-basename-line-hint-contract-research.md)
- narrowed basename-derived `:line` parsing to cases where the basename already resolves uniquely to a file hint
- preserved the current exact-path `path:line`, explicit `line N`, and ambiguity-fallback behavior
- authored the follow-on execution slice as `101-T-p1-prompt-context-basename-line-hint-shell.md`

## Carry Forward To Next Task

Next task:

- `101-T-p1-prompt-context-basename-line-hint-shell.md`

Carry forward:

- only activate basename-derived `:line` after unique basename resolution
- preserve exact-path `path:line` and explicit `line N` behavior
- keep this slice separate from broader filename parsing

Open points:

- route the first shell through existing basename-resolution guardrails instead of broadening filename parsing
