---
doc_type: task
task_id: 96
title: P1 prompt_context symbol line hint contract research
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 95-T-p1-prompt-context-symbol-file-hint-shell.md
next_task: 97-T-p1-prompt-context-symbol-line-hint-shell.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 96: P1 Prompt Context Symbol Line Hint Contract Research

## Objective

- define the smallest prompt-context follow-up contract that lets an explicit line hint disambiguate the combined file+symbol exact-selector path

## Why This Exists

- task 95 fixes the first prompt-context composition gap, but ambiguous same-name symbols in a hinted file still require a raw `symbol_line` follow-up outside prompt-context
- prompt-context already has the exact-selector substrate it needs; the missing piece is a narrow line-hint bridge
- this is the smallest next slice that improves prompt-driven disambiguation without reopening broader prompt parsing work

## Read Before Work

- [94-R-p1-prompt-context-symbol-file-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/94-R-p1-prompt-context-symbol-file-hint-contract-research.md)
- [95-T-p1-prompt-context-symbol-file-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/95-T-p1-prompt-context-symbol-file-hint-shell.md)
- [93-T-p1-get-symbol-context-exact-selector-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/93-T-p1-get-symbol-context-exact-selector-shell.md)

## Expected Touch Points

- `docs/execution-plan/96-T-p1-prompt-context-symbol-line-hint-contract-research.md`
- `docs/execution-plan/96-R-p1-prompt-context-symbol-line-hint-contract-research.md`
- `docs/execution-plan/97-T-p1-prompt-context-symbol-line-hint-shell.md`

## Deliverable

- a small research task that defines the first prompt-context line-hint contract and authors the next execution slice

## Done When

- the accepted line-hint shape is explicit
- the relationship to the existing exact-selector `symbol_line` flow is clear
- the next implementation slice is recoverable from disk

## Completion Notes

- added [96-R-p1-prompt-context-symbol-line-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/96-R-p1-prompt-context-symbol-line-hint-contract-research.md)
- narrowed the first prompt-context line disambiguator to explicit `line N` hints only
- kept the fallback contract explicit: if no line hint is present, prompt-context should keep the stable exact-selector ambiguity behavior
- authored the follow-on execution slice as `97-T-p1-prompt-context-symbol-line-hint-shell.md`

## Carry Forward To Next Task

Next task:

- `97-T-p1-prompt-context-symbol-line-hint-shell.md`

Carry forward:

- keep the parser narrow and explicit
- preserve current behavior when no line hint is present
- do not broaden this slice into general natural-language line parsing

Open points:

- route the first shell through the existing combined file+symbol path without changing file-only, symbol-only, or repo-map behavior
