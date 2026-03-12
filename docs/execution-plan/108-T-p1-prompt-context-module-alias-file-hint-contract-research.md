---
doc_type: task
task_id: 108
title: P1 prompt_context module alias file hint contract research
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 107-T-p1-prompt-context-qualified-module-alias-shell.md
next_task: 109-T-p1-prompt-context-module-alias-file-hint-shell.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 108: P1 Prompt Context Module Alias File Hint Contract Research

## Objective

- define the smallest follow-up contract that lets exact qualified module aliases like `crate::db` act as prompt-context file hints even without `:line`

## Why This Exists

- task 107 covers exact qualified module aliases with `:line`, but prompts often name a module and symbol without an explicit line
- exact module aliases can already identify one file deterministically when they match the indexed module path
- this is the next small ergonomic bridge before any broader module-guessing discussion

## Read Before Work

- [106-R-p1-prompt-context-qualified-module-alias-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/106-R-p1-prompt-context-qualified-module-alias-contract-research.md)
- [107-T-p1-prompt-context-qualified-module-alias-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/107-T-p1-prompt-context-qualified-module-alias-shell.md)
- [95-T-p1-prompt-context-symbol-file-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/95-T-p1-prompt-context-symbol-file-hint-shell.md)

## Expected Touch Points

- `docs/execution-plan/108-T-p1-prompt-context-module-alias-file-hint-contract-research.md`
- `docs/execution-plan/108-R-p1-prompt-context-module-alias-file-hint-contract-research.md`
- `docs/execution-plan/109-T-p1-prompt-context-module-alias-file-hint-shell.md`

## Deliverable

- a research task that decides whether exact qualified module aliases should activate file hints without `:line` and authors the next shell slice

## Done When

- the no-line module-alias prompt shape is explicit
- the exact-boundary rule that separates `crate::db` from `crate::dbx` is clear
- the next implementation slice is recoverable from disk

## Completion Notes

- added [108-R-p1-prompt-context-module-alias-file-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/108-R-p1-prompt-context-module-alias-file-hint-contract-research.md)
- chose exact qualified module aliases as no-line file hints when they match a full module-path boundary
- authored the follow-on execution slice as `109-T-p1-prompt-context-module-alias-file-hint-shell.md`

## Carry Forward To Next Task

Next task:

- `109-T-p1-prompt-context-module-alias-file-hint-shell.md`

Carry forward:

- keep module aliases exact and boundary-aware
- preserve current `:line` parsing and fallback behavior
- avoid broadening this slice into fuzzy module guessing

Open points:

- whether a later slice should extend no-line module hints beyond explicitly qualified aliases
