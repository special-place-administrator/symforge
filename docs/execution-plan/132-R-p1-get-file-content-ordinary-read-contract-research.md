# Research: P1 Get File Content Ordinary Read Contract

Related plan:

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [02-P-workstreams-and-tool-surface.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/02-P-workstreams-and-tool-surface.md)
- [131-T-p1-get-file-content-around-symbol-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/131-T-p1-get-file-content-around-symbol-shell.md)

Goal:

- decide the smallest stable contract for line-numbered and optionally headed ordinary `get_file_content` reads without disturbing the contextual and chunked modes already shipped

## Current Code Reality

`get_file_content` now supports:

- exact-path full-file reads
- explicit line ranges
- `around_line`
- `around_match`
- `around_symbol`
- exact-path chunked reads

The contextual and chunked modes already emit numbered excerpts or a stable header. The ordinary full-file and explicit-range modes still emit raw text only.

## Candidate Approaches

### Option 1: always add headers and line numbers to every ordinary read

- simplest implementation
- breaks the long-standing public contract for full-file and explicit-range reads

### Option 2: optional `show_line_numbers` and `header` flags for ordinary reads

- smallest additive surface
- preserves backward compatibility by default
- lets agents opt into a richer shell-replacement format only when needed

### Option 3: separate read modes or a second tool

- explicit
- too heavy for a formatting-only upgrade

## Decision: Additive Ordinary-Read Flags

Recommendation:

- add optional `show_line_numbers` and `header` inputs to `get_file_content`
- apply them only to ordinary full-file and explicit-range reads in the first slice
- keep the default behavior unchanged when both flags are absent or false
- leave `around_line`, `around_match`, `around_symbol`, and chunked reads on their current locked formats for now

## Output Contract

- full-file and explicit-range reads keep the current raw output by default
- `show_line_numbers=true` renders numbered lines for those ordinary reads
- `header=true` prepends a small stable header with the file path and, for explicit ranges, the covered line span
- `header` and `show_line_numbers` can be used together
- the first slice should not reinterpret or restyle contextual or chunked modes

## Why This Is The Smallest Useful Slice

- it closes one of the last direct shell-replacement gaps without reopening the already-set contextual read contracts
- it preserves backward compatibility for existing callers
- it isolates formatting work from the broader non-code text-lane backlog

## Recommended Next Implementation Slice

- extend `GetFileContentInput` and `ContentContext` with `show_line_numbers` and `header`
- teach the ordinary read renderer to format numbered full/range output and an optional header
- keep the contextual and chunked renderers unchanged
- add focused tool, formatter, and integration coverage

Expected touch points:

- `src/protocol/tools.rs`
- `src/protocol/format.rs`
- `src/live_index/search.rs`
- `tests/live_index_integration.rs`

## Carry Forward

- preserve the current default full-file and explicit-range output when the new flags are absent
- keep the first slice scoped to ordinary reads only
- defer non-code text-lane work, richer chunk headers, and context-mode formatting unification
