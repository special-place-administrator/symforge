---
doc_type: task
task_id: 133
title: P1 get_file_content ordinary read shell
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 132-T-p1-get-file-content-ordinary-read-contract-research.md
next_task: 134-T-p1-get-file-content-ordinary-read-resource-parity-contract-research.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 133: P1 Get File Content Ordinary Read Shell

## Objective

- let `get_file_content` optionally render line-numbered and headed ordinary full-file and explicit-range reads

## Why This Exists

- task 132 chooses additive `show_line_numbers` and `header` flags as the smallest remaining shell-replacement upgrade for ordinary reads
- the contextual and chunked modes already have richer formatting, but the ordinary read path still returns raw text only

## Read Before Work

- [132-R-p1-get-file-content-ordinary-read-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/132-R-p1-get-file-content-ordinary-read-contract-research.md)
- [132-T-p1-get-file-content-ordinary-read-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/132-T-p1-get-file-content-ordinary-read-contract-research.md)
- [131-T-p1-get-file-content-around-symbol-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/131-T-p1-get-file-content-around-symbol-shell.md)

## Expected Touch Points

- `src/protocol/tools.rs`
- `src/protocol/format.rs`
- `src/live_index/search.rs`
- `tests/live_index_integration.rs`

## Deliverable

- a `get_file_content` shell that optionally adds line numbers and a stable header to ordinary full-file and explicit-range reads without changing the current contextual or chunked modes

## Done When

- default full-file and explicit-range reads keep the current raw output
- `show_line_numbers` renders numbered ordinary reads
- `header` prepends a stable path or path-plus-range header to ordinary reads
- contextual and chunked modes keep their current locked formats
- focused tests cover the new formatting flags

## Completion Notes

- added additive `show_line_numbers` and `header` support for ordinary full-file and explicit-range `get_file_content` reads
- preserved the default raw full-file and explicit-range contract when the new flags are absent
- kept contextual and chunked modes on their current locked formats by rejecting ordinary-read formatting flags there
- focused tool, formatter, and live-index integration coverage passed, followed by a green `cargo test`

## Carry Forward To Next Task

Next task:

- `134-T-p1-get-file-content-ordinary-read-resource-parity-contract-research.md`

Carry forward:

- keep the default full-file and explicit-range contract unchanged when the new flags are absent
- keep the first slice scoped to ordinary reads only
- treat resource parity for the new ordinary-read flags as the next smallest public-surface gap
