---
title: RTK match_output Investigation
type: audit
status: complete
date: 2026-05-16
roadmap_unit: "Wave 3a / Unit 3a.2"
---

# RTK match_output Investigation

## Scope

Roadmap Unit 3a.2 asks whether `match_output`, from the RTK Tier 2 note,
refers to a real SymForge symbol. This is research-only. No source code was
patched.

Required output:

- clear yes/no on whether `match_output` exists as a real symbol
- if yes, propose the short-circuit shape
- if no, mark T2.6 N/A and state the downstream implication for T2.2

## Finding

`match_output` is not a real SymForge symbol in the current checkout.

T2.6 should be marked N/A as a named implementation unit. The RTK idea is still
understandable as a generic formatter optimization, but there is no concrete
`match_output` hook, helper, or pipeline stage to patch.

Downstream implication: T2.2 SQLite analytics should be treated as standalone
if the roadmap keeps it. It should not depend on T2.6, because T2.6 did not
find a real symbol or implementation seam. The roadmap has contradictory
wording here: Unit 3a.2 says "mark T2.2 as standalone" when `match_output` is
not real, while Unit 3e.2 says T2.2 collapses if `match_output` is not real.
This investigation resolves only the evidence question: `match_output` is not
real. The owner should decide whether T2.2 remains valuable on its independent
telemetry merits.

## Evidence Used

- SymForge sidecar was reindexed to `C:\AI_STUFF\PROGRAMMING\symforge` and
  reported 379 files / 12,489 symbols.
- `mcp__symforge__search_symbols query="match_output"` returned no source symbol
  matches. After reindexing, the only matches were roadmap headings in
  `docs/plans/2026-05-15-symforge-post-h-roadmap.md`.
- `mcp__symforge__search_text` for `match_output`, `MatchOutput`,
  `match-output`, `matches_output`, `output_match`, and `match output` found no
  implementation target. The only source match was an unrelated local expression
  in `src/protocol/format.rs:2041`:
  `match output[..budget].rfind('\n')`.
- Raw cross-check:
  `rg -n -S "\bmatch_output\b|\bMatchOutput\b|\bmatches_output\b|\boutput_match\b|\bmatch-output\b" src tests npm scripts Cargo.toml Cargo.lock`
  returned no matches.
- Raw broader cross-check:
  `rg -n -S "match_output|MatchOutput|match-output|matches_output|output_match|match output" src tests docs -g '!target'`
  found roadmap references, one unrelated `match output[i]` in
  `docs/superpowers/plans/2026-03-20-reviewer-feedback-remediation.md`, and the
  unrelated `src/protocol/format.rs:2041` expression.
- ReminDB cross-reference of `wiki/concepts/RTK Techniques for SymForge.md`
  shows the RTK note describes a concept, not a symbol: short-circuiting
  obviously successful or empty output before expensive line-by-line filtering.
  The SymForge examples named there are zero-match searches and healthy index
  responses.

## Closest Real Implementation Surfaces

The RTK idea maps to existing formatter code, but not to a shared
`match_output` abstraction.

- `src/protocol/format.rs:350-401` - `search_symbols_result_view` already
  returns immediately for empty `result.hits`, producing
  `No symbols matching ...`.
- `src/protocol/format.rs:454-735` - `search_text_result_view` already returns
  immediately for invalid query cases and no-match cases, producing
  `No matches for ...` or structural-search-specific guidance.
- `src/protocol/format.rs:1449-1510` - `search_files_result_view` uses a
  `SearchFilesView::NotFound` arm to return
  `No indexed source files matching ...`.
- `src/protocol/format.rs:1179-1234` -
  `health_report_compact_from_published_state` already renders a short
  status-line style health response.
- `src/protocol/format.rs:1236-1368` - `health_report_from_stats` builds the
  full health report and appends parse-resilience sections only when relevant.
- `src/protocol/tools.rs:4634-4727` - `health` still appends token savings,
  tool-call counts, hook adoption, git temporal status, worktree misuse, and
  optional frecency diagnostics after the base health formatter.
- `src/protocol/tools.rs:4734-4788` - `health_compact` is the existing explicit
  compact path for healthy-index style output.

These sites show that trivial-response behavior is currently implemented as
local branch logic in each formatter or tool handler, not as a central output
matching pipeline.

## Short-Circuit Shape If Revisited Later

Because no `match_output` symbol exists, Unit 3e.4 should not proceed as
written. If the product still wants this class of optimization later, treat it
as a new design item rather than a patch to T2.6.

Recommended future shape:

1. Keep the existing local early returns for search formatter no-match paths.
   They are simple, deterministic, and already covered by formatter tests.
2. Prefer `health_compact` for terse health output rather than adding a
   heuristic to collapse full `health` output after it is assembled.
3. If a generic trivial-response layer is introduced, make it explicit and typed
   around formatter inputs such as `SearchFilesView::NotFound`, empty
   `SymbolSearchResult`, empty `TextSearchResult`, or `PublishedIndexStatus`.
   Do not regex-match rendered strings after the fact.
4. Do not add SQLite analytics only to support this missing hook. Analytics can
   still be valuable as an independent observability feature, but that should be
   justified separately.

## Verification

- Existence decision: no real `match_output` symbol exists.
- T2.6 status: N/A as currently written.
- T2.2 implication: standalone only if retained; it has no dependency on T2.6.
- Source changes: none.
