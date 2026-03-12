---
doc_type: task
task_id: 131
title: P1 get_file_content around_symbol shell
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 130-T-p1-get-file-content-around-symbol-contract-research.md
next_task: 132-T-p1-get-file-content-ordinary-read-contract-research.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 131: P1 Get File Content Around Symbol Shell

## Objective

- let `get_file_content` return a numbered excerpt anchored to a symbol in an exact target file

## Why This Exists

- task 130 chooses exact-path `around_symbol` plus optional `symbol_line` as the first stable symbol-anchored read contract
- agents should be able to move from `search_symbols` into `get_file_content` without manually copying line numbers

## Read Before Work

- [130-R-p1-get-file-content-around-symbol-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/130-R-p1-get-file-content-around-symbol-contract-research.md)
- [130-T-p1-get-file-content-around-symbol-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/130-T-p1-get-file-content-around-symbol-contract-research.md)
- [129-T-p1-get-file-content-chunking-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/129-T-p1-get-file-content-chunking-shell.md)

## Expected Touch Points

- `src/protocol/tools.rs`
- `src/protocol/format.rs`
- `src/live_index/search.rs`
- `tests/live_index_integration.rs`

## Deliverable

- a `get_file_content` shell that supports exact-path `around_symbol` excerpts while preserving the existing full-file, range, line, match, and chunk modes

## Done When

- unique same-file `around_symbol` requests resolve and render numbered excerpts
- ambiguous same-name symbols require `symbol_line` deterministically
- missing symbols return a stable not-found message
- mixing `around_symbol` with the other selector families is rejected deterministically
- focused tests cover the new symbol-anchored read mode

## Completion Notes

- added exact-path `around_symbol` plus optional `symbol_line` support to `get_file_content`
- file-local symbol reads now reuse numbered excerpt rendering and deterministic ambiguity messaging
- exact symbol selectors match stored symbol lines while the excerpt anchor is converted to the corresponding user-visible file line
- focused unit, tool, and integration coverage passed, followed by a green `cargo test`

## Carry Forward To Next Task

Next task:

- `132-T-p1-get-file-content-ordinary-read-contract-research.md`

Carry forward:

- keep the first `around_symbol` slice exact-path only
- keep the first renderer anchored to symbol start lines
- avoid broadening this slice into full symbol-span rendering or chunk-containing-symbol heuristics
