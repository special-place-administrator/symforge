---
doc_type: task
task_id: 135
title: P1 get_file_content ordinary read resource parity shell
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 134-T-p1-get-file-content-ordinary-read-resource-parity-contract-research.md
next_task: 136-T-p1-get-file-content-contextual-resource-parity-contract-research.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 135: P1 Get File Content Ordinary Read Resource Parity Shell

## Objective

- let the file-content resource template opt into ordinary-read `show_line_numbers` and `header`

## Why This Exists

- task 133 added ordinary-read formatting flags to the tool surface
- task 134 chose ordinary-read resource parity as the next smallest public-surface gap

## Read Before Work

- [134-R-p1-get-file-content-ordinary-read-resource-parity-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/134-R-p1-get-file-content-ordinary-read-resource-parity-contract-research.md)
- [134-T-p1-get-file-content-ordinary-read-resource-parity-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/134-T-p1-get-file-content-ordinary-read-resource-parity-contract-research.md)
- [133-T-p1-get-file-content-ordinary-read-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/133-T-p1-get-file-content-ordinary-read-shell.md)

## Expected Touch Points

- `src/protocol/resources.rs`
- `src/protocol/tools.rs`
- `README.md`

## Deliverable

- a file-content resource template that can request ordinary-read line numbers and headers while preserving the current default resource behavior

## Done When

- the `tokenizor://file/content` resource template accepts `show_line_numbers` and `header`
- the new flags forward into `GetFileContentInput`
- resource requests without the new flags keep the current output
- focused tests cover the new resource parity behavior

## Completion Notes

- extended the file-content resource template with ordinary-read `show_line_numbers` and `header`
- threaded the new resource query flags through the file-content resource parser into `GetFileContentInput`
- preserved the default resource output when the new flags are absent
- focused resource tests passed, followed by a green `cargo test`

## Carry Forward To Next Task

Next task:

- `136-T-p1-get-file-content-contextual-resource-parity-contract-research.md`

Carry forward:

- keep the first resource parity slice scoped to ordinary reads only
- do not broaden this slice into contextual or chunked resource parity
- take contextual file-content resource parity as the next smallest gap
