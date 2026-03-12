---
doc_type: task
task_id: 110
title: P1 prompt_context qualified symbol alias contract research
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 109-T-p1-prompt-context-module-alias-file-hint-shell.md
next_task: 111-T-p1-prompt-context-qualified-symbol-alias-shell.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 110: P1 Prompt Context Qualified Symbol Alias Contract Research

## Objective

- define the smallest follow-up contract that lets prompt-context accept exact qualified symbol aliases like `crate::db::connect` without requiring a separate file hint

## Why This Exists

- task 109 makes exact module aliases behave like file hints, but prompts still often name the full symbol path directly
- fully qualified symbol aliases can be more precise than module-plus-symbol token pairs when the user already knows the full name
- this is the next natural exact-selector bridge before any broader semantic prompt parsing

## Read Before Work

- [106-R-p1-prompt-context-qualified-module-alias-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/106-R-p1-prompt-context-qualified-module-alias-contract-research.md)
- [109-T-p1-prompt-context-module-alias-file-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/109-T-p1-prompt-context-module-alias-file-hint-shell.md)
- [91-T-p1-get-context-bundle-exact-selector-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/91-T-p1-get-context-bundle-exact-selector-shell.md)

## Expected Touch Points

- `docs/execution-plan/110-T-p1-prompt-context-qualified-symbol-alias-contract-research.md`
- `docs/execution-plan/110-R-p1-prompt-context-qualified-symbol-alias-contract-research.md`
- `docs/execution-plan/111-T-p1-prompt-context-qualified-symbol-alias-shell.md`

## Deliverable

- a research task that chooses the first exact qualified-symbol prompt shape and authors the next shell slice

## Done When

- the accepted qualified symbol alias syntax is explicit
- the boundary between exact qualified symbols and fuzzy symbol-path guessing is clear
- the next implementation slice is recoverable from disk

## Completion Notes

- added [110-R-p1-prompt-context-qualified-symbol-alias-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/110-R-p1-prompt-context-qualified-symbol-alias-contract-research.md)
- chose exact fully qualified symbol aliases like `crate::db::connect` as the next safe prompt-context boundary
- authored the follow-on execution slice as `111-T-p1-prompt-context-qualified-symbol-alias-shell.md`

## Carry Forward To Next Task

Next task:

- `111-T-p1-prompt-context-qualified-symbol-alias-shell.md`

Carry forward:

- keep qualified symbol aliases exact and boundary-aware
- preserve the current file-hint and module-hint routes
- avoid broadening this slice into fuzzy path or namespace guessing

Open points:

- whether later slices should cover additional language-specific qualified symbol forms explicitly
