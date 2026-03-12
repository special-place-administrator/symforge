# Research: P1 Get File Content Around Symbol Contract

Related plan:

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [02-P-workstreams-and-tool-surface.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/02-P-workstreams-and-tool-surface.md)
- [129-T-p1-get-file-content-chunking-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/129-T-p1-get-file-content-chunking-shell.md)

Goal:

- decide the first exact-path `get_file_content` contract for symbol-anchored excerpts without widening into span rendering, chunk continuation helpers, or cross-file symbol lookup

## Current Code Reality

`get_file_content` now supports:

- exact-path full-file reads
- explicit line ranges
- `around_line`
- first-match literal `around_match`
- exact-path `chunk_index` plus `max_lines`

It still lacks a way to ask for context around a symbol when the caller knows the file and symbol identity but not the line number.

## Candidate Approaches

### Option 1: `around_symbol` by name only

- smallest input surface
- too ambiguous for files that contain repeated helper names or generated duplicates

### Option 2: `around_symbol` plus optional `symbol_line`

- aligns with current `search_symbols` exact-selector output
- keeps lookup file-local because `get_file_content` already starts from an exact path
- lets the first slice reject ambiguity deterministically instead of guessing

### Option 3: explicit byte or line span input from callers

- more exact
- pushes symbol-resolution work onto callers and duplicates knowledge the index already has

## Decision: Exact-Path File-Local Symbol Anchor

Recommendation:

- add optional `around_symbol` and `symbol_line` inputs to `get_file_content`
- require exact file path, as with current line, match, and chunked read modes
- resolve symbols only within the captured file’s existing symbol list
- when `symbol_line` is omitted:
  - accept a unique symbol name in that file
  - reject ambiguous repeated names with a stable message asking for `symbol_line`
- anchor the first slice on the symbol start line and reuse `context_lines`

## Output Contract

- return the same numbered excerpt style already used by `around_line`
- preserve the current default around-line context behavior when `context_lines` is omitted
- return a stable symbol-not-found message when the named symbol is absent from the target file
- return a stable ambiguity message when multiple same-name symbols exist in the file and no `symbol_line` is provided
- reject mixing `around_symbol` with line ranges, `around_line`, `around_match`, or chunk selectors

## Why This Is The Smallest Useful Slice

- it lets agents pivot directly from `search_symbols` to `get_file_content` without manual line copying
- it reuses current exact-selector patterns already adopted in `find_references`, `get_context_bundle`, and `get_symbol_context`
- it avoids widening this step into symbol-span rendering or chunk-selection heuristics

## Recommended Next Implementation Slice

- extend `GetFileContentInput` with `around_symbol` and `symbol_line`
- add validation for exclusivity against the current read modes
- add a file-local symbol anchor helper in the formatter path
- reuse numbered around-line rendering once the symbol line is resolved
- add focused tool, formatter, and integration coverage

Expected touch points:

- `src/protocol/tools.rs`
- `src/protocol/format.rs`
- `src/live_index/search.rs`
- `tests/live_index_integration.rs`

## Carry Forward

- keep the first `around_symbol` slice exact-path only
- keep the first renderer anchored to the symbol start line rather than full symbol spans
- defer chunk-containing-symbol selection, end-line anchoring, and cross-file symbol resolution
