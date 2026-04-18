# Follow-up: `replace_symbol_body` single-line inline JSDoc

Surfaced during code review of `fix/review-bugs-v7.5` (finding M1).

## The case

When a doc comment and the symbol signature live on the same source line — e.g., `/** @deprecated */ export function legacy() { ... }` — Unit 4's splice-range fix does not protect the doc. Walk-through:

1. `new_body` has no doc → `new_body_supplies_docs = false`.
2. `effective = sym.byte_range.0 as usize` — offset of `export`.
3. `raw_line_start = rposition('\n' in content[..effective]) + 1` — walks back past `/** @deprecated */` to the prior line boundary.
4. `line_start = raw_line_start`.
5. Splice `(line_start, sym.byte_range.1)` overwrites the whole line, doc included.

The attached-doc and orphan-doc cases are covered by Unit 4's tests because both put the doc on its own line. The one-line-inline case falls through.

## Why we didn't fix it in Unit 4

- Rare in idiomatic code across every language SymForge indexes.
- A correct fix needs to split the current source line at `sym.byte_range.0` when a doc marker is present to the left, which is grammar-specific (doc markers are language-dependent).
- Adding that complexity for an edge case the original bug report did not hit would have widened Unit 4 past what the plan promised.

## Suggested approach

1. After computing `raw_line_start`, scan the slice `content[raw_line_start..sym.byte_range.0]` for a trailing doc-comment close (`*/`, `*/`, `///` at EOL, etc.).
2. If one is found, advance `line_start` to the position immediately after it (preserving any intervening whitespace — strip or keep based on style preference).
3. Add a fixture test per language: Rust `///`, TypeScript `/** */`, Python `#`, Java `/** */`.

## Severity

Minor. User-visible only when the caller runs `replace_symbol_body` on a symbol whose doc and signature share a source line, AND provides a `new_body` without its own doc.
