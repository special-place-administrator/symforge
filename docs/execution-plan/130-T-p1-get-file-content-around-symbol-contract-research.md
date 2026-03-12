---
doc_type: task
task_id: 130
title: P1 get_file_content around_symbol contract research
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 129-T-p1-get-file-content-chunking-shell.md
next_task: 131-T-p1-get-file-content-around-symbol-shell.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 130: P1 Get File Content Around Symbol Contract Research

## Objective

- decide the smallest stable `get_file_content` contract for symbol-anchored excerpts

## Why This Exists

- the source plan calls out `around_symbol` as a next-step improvement after line, match, and chunked reads
- agents already get exact-path and symbol-line data from `search_symbols`, so `get_file_content` should be able to consume that directly

## Read Before Work

- [02-P-workstreams-and-tool-surface.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/02-P-workstreams-and-tool-surface.md)
- [129-T-p1-get-file-content-chunking-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/129-T-p1-get-file-content-chunking-shell.md)

## Deliverable

- a short research note choosing the first `around_symbol` contract and the next shell slice

## Done When

- the contract defines the minimum new inputs
- ambiguity handling is decided
- the renderer target is clear enough for a shell implementation
- the next shell task is authored

## Completion Notes

- chose exact-path `around_symbol` plus optional `symbol_line` as the first contract
- kept the first slice file-local and anchored to symbol start lines with current numbered excerpt rendering
- deferred full symbol-span rendering, chunk-containing-symbol selection, and cross-file lookup

## Carry Forward To Next Task

Next task:

- `131-T-p1-get-file-content-around-symbol-shell.md`

Carry forward:

- preserve current full-file, range, `around_line`, `around_match`, and chunked read behavior
- require deterministic ambiguity handling when repeated symbol names exist in one file
