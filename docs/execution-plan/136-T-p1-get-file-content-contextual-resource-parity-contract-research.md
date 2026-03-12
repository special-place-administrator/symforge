---
doc_type: task
task_id: 136
title: P1 get_file_content contextual resource parity contract research
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 135-T-p1-get-file-content-ordinary-read-resource-parity-shell.md
next_task: 137-T-p1-get-file-content-contextual-resource-parity-shell.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 136: P1 Get File Content Contextual Resource Parity Contract Research

## Objective

- choose the next smallest file-content resource parity slice after ordinary full/range reads

## Why This Exists

- task 135 closed the ordinary-read resource gap
- the file-content resource template still trails the tool on contextual, symbolic, and chunked modes

## Read Before Work

- [135-T-p1-get-file-content-ordinary-read-resource-parity-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/135-T-p1-get-file-content-ordinary-read-resource-parity-shell.md)
- [02-P-workstreams-and-tool-surface.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/02-P-workstreams-and-tool-surface.md)

## Deliverable

- a short research note selecting the next file-content resource parity contract and authoring the follow-on shell slice

## Done When

- the next minimum resource inputs are explicit
- backward-compatibility behavior is explicit
- the follow-on shell task is authored

## Completion Notes

- chose contextual resource parity next: `around_line`, `around_match`, and `context_lines`
- kept symbolic and chunked resource parity out of scope for the next slice
- authored the follow-on shell task for the contextual resource path

## Carry Forward To Next Task

Next task:

- `137-T-p1-get-file-content-contextual-resource-parity-shell.md`

Carry forward:

- preserve ordinary-read resource behavior from task 135
- keep the next slice scoped to contextual reads only
