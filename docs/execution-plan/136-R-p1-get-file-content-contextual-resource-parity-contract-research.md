# Research: P1 Get File Content Contextual Resource Parity Contract

Related plan:

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [02-P-workstreams-and-tool-surface.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/02-P-workstreams-and-tool-surface.md)
- [135-T-p1-get-file-content-ordinary-read-resource-parity-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/135-T-p1-get-file-content-ordinary-read-resource-parity-shell.md)

Goal:

- choose the next smallest file-content resource parity slice after ordinary full/range reads gained resource support

## Current Code Reality

The file-content tool supports:

- ordinary full-file and explicit-range reads
- ordinary-read `show_line_numbers`
- ordinary-read `header`
- `around_line`
- `around_match`
- `around_symbol`
- chunked reads via `chunk_index` plus `max_lines`

The file-content resource template now supports:

- `path`
- `start_line`
- `end_line`
- `show_line_numbers`
- `header`

## Candidate Approaches

### Option 1: expose every remaining file-content mode at once

- would add contextual, symbolic, and chunked parameters together
- too broad for the next recoverable slice

### Option 2: contextual resource parity first

- add `around_line`, `around_match`, and `context_lines`
- reuses the stable numbered-excerpt contract that already exists on the tool
- avoids the extra selector ambiguity rules from `around_symbol`

### Option 3: jump straight to symbolic resource parity

- useful, but it introduces `symbol_line` exact-selector behavior at the same time
- broader than the simpler contextual step

### Option 4: jump straight to chunked resource parity

- useful for large files
- weaker immediate value than contextual follow-up from search results

## Decision: Contextual Resource Parity First

Recommendation:

- extend the file-content resource template with `around_line`, `around_match`, and `context_lines`
- keep the slice limited to the existing contextual read modes
- preserve ordinary-read resource behavior from task 135
- defer `around_symbol`, `symbol_line`, `chunk_index`, and `max_lines` to later slices

## Output Contract

- resource requests using `around_line` or `around_match` should match the current tool output exactly
- default ordinary resource reads keep the current behavior
- symbolic and chunked resource parity remain out of scope for this slice

## Why This Is The Smallest Useful Slice

- it covers the most common follow-up from `search_text` and path-driven navigation
- it keeps resource parity moving without introducing exact-selector ambiguity handling yet
- it stays small enough for focused parser/resource tests

## Recommended Next Implementation Slice

- extend the file-content resource template and URI parser with `around_line`, `around_match`, and `context_lines`
- thread those fields into `GetFileContentInput`
- add focused resource tests for one `around_line` and one `around_match` case

Expected touch points:

- `src/protocol/resources.rs`
- `src/protocol/tools.rs`
- `README.md`

## Carry Forward

- preserve ordinary-read resource behavior from task 135
- keep the slice scoped to contextual reads only
- defer symbolic and chunked resource parity
