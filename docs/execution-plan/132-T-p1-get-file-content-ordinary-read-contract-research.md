---
doc_type: task
task_id: 132
title: P1 get_file_content ordinary read contract research
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 131-T-p1-get-file-content-around-symbol-shell.md
next_task: 133-T-p1-get-file-content-ordinary-read-shell.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 132: P1 Get File Content Ordinary Read Contract Research

## Objective

- choose the smallest contract for line-numbered and optionally headed ordinary `get_file_content` reads

## Why This Exists

- the source plan still calls out `show_line_numbers` and `header` as remaining upgrades for direct file reads
- the current contextual and chunked modes already have richer formatting, but ordinary full/range reads still return raw text only

## Read Before Work

- [02-P-workstreams-and-tool-surface.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/02-P-workstreams-and-tool-surface.md)
- [131-T-p1-get-file-content-around-symbol-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/131-T-p1-get-file-content-around-symbol-shell.md)

## Deliverable

- a short research note selecting the first ordinary-read formatting contract and the next shell slice

## Done When

- the contract defines the minimum new inputs
- backward-compatibility behavior is explicit
- the first shell slice is authored

## Completion Notes

- chose additive `show_line_numbers` and `header` flags for ordinary full/range reads
- kept the current default raw output unchanged when those flags are absent
- deferred contextual/chunked formatting unification and broader text-lane work

## Carry Forward To Next Task

Next task:

- `133-T-p1-get-file-content-ordinary-read-shell.md`

Carry forward:

- preserve the default full-file and explicit-range contract when the new flags are not used
- keep the first slice scoped to ordinary reads only
