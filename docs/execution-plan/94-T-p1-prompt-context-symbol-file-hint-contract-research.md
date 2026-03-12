---
doc_type: task
task_id: 94
title: P1 prompt_context symbol and file hint contract research
status: in_progress
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 93-T-p1-get-symbol-context-exact-selector-shell.md
next_task: 95-T-p1-prompt-context-symbol-file-hint-shell.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 94: P1 Prompt Context Symbol And File Hint Contract Research

## Objective

- define the smallest prompt-context follow-up contract that lets file hints and symbol hints compose through the new exact-selector symbol-context path

## Why This Exists

- prompt-context still chooses file outline first and falls back to name-only symbol context, so it does not benefit from the exact-selector work that now exists in `get_symbol_context`
- if a prompt contains both a file hint and a symbol hint, the current heuristic throws away the symbol hint entirely
- this is the smallest remaining internal consumer that can reuse the new selector lane without introducing stable symbol ids

## Read Before Work

- [91-T-p1-get-context-bundle-exact-selector-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/91-T-p1-get-context-bundle-exact-selector-shell.md)
- [92-R-p1-get-symbol-context-exact-selector-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/92-R-p1-get-symbol-context-exact-selector-contract-research.md)
- [93-T-p1-get-symbol-context-exact-selector-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/93-T-p1-get-symbol-context-exact-selector-shell.md)

## Expected Touch Points

- `docs/execution-plan/94-T-p1-prompt-context-symbol-file-hint-contract-research.md`
- `docs/execution-plan/94-R-p1-prompt-context-symbol-file-hint-contract-research.md`
- `docs/execution-plan/95-T-p1-prompt-context-symbol-file-hint-shell.md`

## Deliverable

- a small research task that fixes the first prompt-context composition contract for file hints plus symbol hints and authors the next execution slice

## Done When

- the precedence between file hints, symbol hints, and repo-map requests is explicit
- the relationship to the new exact-selector symbol-context flow is clear
- the next implementation slice is recoverable from disk

## Completion Notes

- pending

## Carry Forward To Next Task

Next task:

- `95-T-p1-prompt-context-symbol-file-hint-shell.md`

Carry forward:

- preserve current prompt-context outputs when only a file hint or only a repo-map intent is present
- keep this research separate from stable symbol-id work
- prefer reusing `symbol_context_text` rather than inventing a parallel prompt-only symbol renderer

Open points:

- OPEN: whether a file hint plus symbol hint should prefer exact symbol context immediately or still show outline when the symbol hint is weak
