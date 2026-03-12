---
doc_type: task
task_id: 85
title: P1 search_symbols scope filter shell
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 84-T-p1-search-symbols-scope-contract-research.md
next_task: 86-T-p1-search-symbols-noise-defaults-research.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 85: P1 Search Symbols Scope Filter Shell

## Objective

- add the first public `search_symbols` scope shell with path, language, and limit filters

## Why This Exists

- the internal query substrate already supports scoped symbol search, but the public MCP tool still exposes only `query` and `kind`
- task 84 fixed the smallest stable contract, so implementation can stay additive and bounded

## Read Before Work

- [84-R-p1-search-symbols-scope-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/84-R-p1-search-symbols-scope-contract-research.md)
- [84-T-p1-search-symbols-scope-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/84-T-p1-search-symbols-scope-contract-research.md)
- [66-T-phase1-shared-query-option-struct-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/66-T-phase1-shared-query-option-struct-shell.md)
- [68-T-phase1-explicit-current-tool-option-defaults-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/68-T-phase1-explicit-current-tool-option-defaults-shell.md)

## Expected Touch Points

- `src/protocol/tools.rs`
- `src/live_index/search.rs`
- `src/protocol/format.rs`

## Deliverable

- a first scoped `search_symbols` shell that accepts path, language, and bounded limit filters without changing the current output format

## Done When

- `search_symbols` accepts `path_prefix`, `language`, and `limit`
- path-prefix and language filters narrow symbol hits deterministically
- `limit` is caller-controlled, defaults to 50, and stays bounded
- current `kind` filtering behavior is preserved
- focused tests cover path, language, limit, and kind interactions

## Completion Notes

- extended the public `search_symbols` input with `path_prefix`, `language`, and `limit`
- kept the current semantic defaults explicit by preserving code-lane and noise-permissive behavior in the first scoped shell
- added a symbol-search language filter to `SymbolSearchOptions` and reused the existing path-prefix normalization and canonical language parsing path in the tool layer
- made `limit` caller-controlled while preserving the current default of 50 and capping the first public shell at 100
- preserved the current tiered formatter output and `kind` filtering behavior
- verification run for this task:
  - `cargo test test_search_module_symbol_search_with_options_respects_path_language_and_limit -- --nocapture`
  - `cargo test test_search_symbols_tool_respects_scope_language_limit_and_kind -- --nocapture`
  - `cargo test search_symbols -- --nocapture`
  - `cargo test`

## Carry Forward To Next Task

Next task:

- `86-T-p1-search-symbols-noise-defaults-research.md`

Carry forward:

- keep generated/test suppression out of this shell
- preserve current tier ordering and formatter output
- keep exact-symbol identity work separate

Open points:

- OPEN: whether the next scoped-symbol slice should add noise suppression or ranking refinements first
