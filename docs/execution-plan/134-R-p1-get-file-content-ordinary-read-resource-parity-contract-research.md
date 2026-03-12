# Research: P1 Get File Content Ordinary Read Resource Parity Contract

Related plan:

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [02-P-workstreams-and-tool-surface.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/02-P-workstreams-and-tool-surface.md)
- [133-T-p1-get-file-content-ordinary-read-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/133-T-p1-get-file-content-ordinary-read-shell.md)

Goal:

- choose the smallest resource-surface parity slice after ordinary-read `get_file_content` gained additive `show_line_numbers` and `header`

## Current Code Reality

`get_file_content` tools now support:

- exact-path full-file reads
- explicit line ranges
- ordinary-read `show_line_numbers`
- ordinary-read `header`
- `around_line`
- `around_match`
- `around_symbol`
- chunked reads via `chunk_index` plus `max_lines`

The file-content resource template still only exposes:

- `path`
- `start_line`
- `end_line`

## Candidate Approaches

### Option 1: full file-content resource parity in one slice

- would expose ordinary, contextual, symbolic, and chunked modes together
- too broad for the next recoverable step

### Option 2: ordinary-read resource parity only

- adds `show_line_numbers` and `header` to the existing file-content resource template
- keeps the template shape familiar
- closes the gap created by task 133 without reopening the larger contextual/chunked resource surface

### Option 3: defer resource parity entirely

- leaves the tools/resources mismatch visible on a recently changed public surface
- weakens recoverability for clients that prefer resources over tools for direct reads

## Decision: Ordinary-Read Resource Parity First

Recommendation:

- extend the `tokenizor://file/content` resource template with optional `show_line_numbers` and `header`
- keep the slice limited to ordinary full-file and explicit-range reads
- preserve the current default resource output when the new flags are absent
- defer `around_line`, `around_match`, `around_symbol`, and chunked resource parity to later slices

## Output Contract

- resource requests without the new flags keep the current raw full/range behavior
- `show_line_numbers=true` yields numbered ordinary reads
- `header=true` yields the same stable path or path-plus-range header as the tool
- unsupported contextual or chunked resource modes remain out of scope for this slice

## Why This Is The Smallest Useful Slice

- it keeps the queue aligned with the latest tool capability instead of letting the resource surface drift further
- it limits changes to the existing file-content resource template instead of inventing a second URI family
- it stays small enough to verify through resource-side tests without pulling in the broader non-code text lane

## Recommended Next Implementation Slice

- extend the file-content resource request/template parser with optional `show_line_numbers` and `header`
- pass the new flags through to `GetFileContentInput`
- add focused resource coverage that proves the resource output matches the tool behavior for ordinary reads

Expected touch points:

- `src/protocol/resources.rs`
- `src/protocol/tools.rs`
- `README.md`

## Carry Forward

- preserve the default resource contract when the new flags are absent
- keep the slice scoped to ordinary reads only
- defer contextual and chunked resource parity to later tasks
