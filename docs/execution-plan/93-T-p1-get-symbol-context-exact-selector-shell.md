---
doc_type: task
task_id: 93
title: P1 get_symbol_context exact selector shell
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 92-T-p1-get-symbol-context-exact-selector-contract-research.md
next_task: 94-T-p1-prompt-context-symbol-file-hint-contract-research.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 93: P1 Get Symbol Context Exact Selector Shell

## Objective

- add the first exact-selector `get_symbol_context` shell that chains from current `search_symbols` output while preserving the current compact grouped output

## Why This Exists

- task 92 fixes the smallest safe contract: preserve current `file` filtering, but add exact-selector inputs for symbol selection
- `get_symbol_context` still pulls from global-name references and needs the same exact-selector follow-up that `find_references` and `get_context_bundle` now have

## Read Before Work

- [92-R-p1-get-symbol-context-exact-selector-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/92-R-p1-get-symbol-context-exact-selector-contract-research.md)
- [92-T-p1-get-symbol-context-exact-selector-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/92-T-p1-get-symbol-context-exact-selector-contract-research.md)
- [89-T-p1-find-references-exact-selector-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/89-T-p1-find-references-exact-selector-shell.md)
- [91-T-p1-get-context-bundle-exact-selector-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/91-T-p1-get-context-bundle-exact-selector-shell.md)

## Expected Touch Points

- `src/protocol/tools.rs`
- `src/sidecar/handlers.rs`
- likely `src/live_index/query.rs`

## Deliverable

- a first exact-selector `get_symbol_context` shell that accepts `path`, `symbol_kind`, and `symbol_line`, preserves current grouped output, and materially reduces same-name noise

## Done When

- current name-only `get_symbol_context` behavior remains intact when no exact selector is supplied
- exact-selector mode accepts `path`, `symbol_kind`, and `symbol_line`
- ambiguous selectors fail deterministically with a stable message
- current `file` filtering still composes with exact-selector mode
- focused tests cover exact-selector success, ambiguity, and backward compatibility

## Completion Notes

- extended `get_symbol_context` with optional exact-selector inputs: `path`, `symbol_kind`, and `symbol_line`
- preserved current name-only behavior when `path` is omitted
- routed the sidecar-backed symbol-context path through the shared exact-selector query helper in `src/live_index/query.rs`
- kept the current compact grouped output and 10-match cap while returning stable user-facing error strings for missing or ambiguous selectors
- preserved `file` as an output filter and added focused coverage to prove it still composes with exact-selector mode
- verification run for this task:
  - `cargo test symbol_context -- --nocapture`
  - `cargo test`

## Carry Forward To Next Task

Next task:

- `94-T-p1-prompt-context-symbol-file-hint-contract-research.md`

Carry forward:

- keep this slice separate from stable symbol-id work
- preserve the current compact grouped output and cap
- keep sidecar token-budget behavior unchanged

Resolved point:

- the next follow-on should target prompt-context file+symbol hint fusion, which is the smallest remaining consumer of the old name-only symbol-context path
