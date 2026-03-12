---
doc_type: task
task_id: 134
title: P1 get_file_content ordinary read resource parity contract research
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 133-T-p1-get-file-content-ordinary-read-shell.md
next_task: 135-T-p1-get-file-content-ordinary-read-resource-parity-shell.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 134: P1 Get File Content Ordinary Read Resource Parity Contract Research

## Objective

- choose the smallest resource-surface parity slice after ordinary-read `get_file_content` gained `show_line_numbers` and `header`

## Why This Exists

- task 133 moved the tool surface forward, but the file-content resource template still only exposes `path`, `start_line`, and `end_line`
- the source plan explicitly wants tools, resources, and prompts, not tools alone

## Read Before Work

- [133-T-p1-get-file-content-ordinary-read-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/133-T-p1-get-file-content-ordinary-read-shell.md)
- [02-P-workstreams-and-tool-surface.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/02-P-workstreams-and-tool-surface.md)

## Deliverable

- a short research note selecting the next file-content resource parity contract and authoring the follow-on shell slice

## Done When

- the minimum next resource inputs are explicit
- backward-compatibility behavior is explicit
- the follow-on shell task is authored

## Completion Notes

- chose ordinary-read resource parity first: add `show_line_numbers` and `header` to the existing file-content resource template
- kept the slice scoped to full-file and explicit-range resource reads
- deferred contextual, symbolic, and chunked resource parity to later tasks

## Carry Forward To Next Task

Next task:

- `135-T-p1-get-file-content-ordinary-read-resource-parity-shell.md`

Carry forward:

- preserve the default resource output when the new flags are absent
- keep the first parity slice scoped to ordinary reads only
