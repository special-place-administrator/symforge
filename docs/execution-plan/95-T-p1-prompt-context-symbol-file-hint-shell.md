---
doc_type: task
task_id: 95
title: P1 prompt_context symbol and file hint shell
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 94-T-p1-prompt-context-symbol-file-hint-contract-research.md
next_task: 96-T-p1-prompt-context-symbol-line-hint-contract-research.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 95: P1 Prompt Context Symbol And File Hint Shell

## Objective

- make prompt-context compose file hints and symbol hints through the exact-selector symbol-context flow while preserving existing single-hint behavior

## Why This Exists

- task 94 fixes the smallest safe contract: combined hints should use the new exact-selector path instead of dropping the symbol hint
- prompt-context is now the smallest remaining consumer that can reuse the exact-selector lane with minimal surface churn

## Read Before Work

- [94-R-p1-prompt-context-symbol-file-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/94-R-p1-prompt-context-symbol-file-hint-contract-research.md)
- [94-T-p1-prompt-context-symbol-file-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/94-T-p1-prompt-context-symbol-file-hint-contract-research.md)
- [93-T-p1-get-symbol-context-exact-selector-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/93-T-p1-get-symbol-context-exact-selector-shell.md)

## Expected Touch Points

- `src/sidecar/handlers.rs`
- `tests/sidecar_integration.rs`

## Deliverable

- a prompt-context shell that uses exact-selector symbol context when a prompt contains both a file hint and a symbol hint, while keeping existing file-only and symbol-only behavior

## Done When

- file hint only still returns outline
- symbol hint only still returns symbol context
- file hint plus symbol hint routes to exact-selector symbol context
- ambiguity in the hinted file returns the stable exact-selector message
- focused tests cover the combined-hint path and preserved single-hint behavior

## Completion Notes

- routed prompt-context through exact-selector symbol context when both a file hint and a symbol hint are present
- preserved the existing `file-only` outline path and `symbol-only` name-only symbol-context path
- added focused handler tests for single-hint preservation, combined-hint exact selection, and exact-selector ambiguity
- added sidecar endpoint coverage to prove `/prompt-context` preserves the combined-hint exact-selector behavior

## Carry Forward To Next Task

Next task:

- `96-T-p1-prompt-context-symbol-line-hint-contract-research.md`

Carry forward:

- keep this slice separate from broader prompt parsing work
- preserve token-budget behavior
- avoid adding a new prompt-only symbol renderer
- if prompt-context is going to help with ambiguity next, prefer feeding the existing `symbol_line` selector instead of inventing a prompt-only disambiguation lane

Open points:

- OPEN: whether prompt-context should accept only explicit `line N` hints first, or also support looser line-reference phrasing later
