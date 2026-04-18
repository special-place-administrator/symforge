# Review Prompt — Co-Change Coupling, Tentacle 1 Steps 1.1–1.3

Copy the prompt below into the reviewing agent's context.

---

## Context: what SymForge is

SymForge is a Rust-based Model Context Protocol (MCP) server at `C:\AI_STUFF\PROGRAMMING\symforge`, version 7.4.6. It serves a live symbol-aware code index to agent CLIs (Claude Code, Codex, Gemini CLI). Its hot path is symbol navigation (`search_files`, `search_symbols`, `find_references`, `get_symbol_context`) backed by tree-sitter parsing and an in-memory + SQLite-backed persistence layer.

The existing ranker (`src/live_index/rank_signals.rs`) already supports a pluggable `RankSignal` trait with two active signals: `PathMatchSignal` (populated) and `CoChangeSignal` (currently hard-wired to return `0.0`). There is already a separate co-change computation path via shell-out to `git log` in `src/live_index/git_temporal.rs` that serves the `changed_with=` branch of `search_files` — but it does not flow through the ranker's weighted-sum fusion.

## What we're building and why

**Feature:** "Co-Change Coupling" — a persistent, symbol-aware coupling graph built from bounded git history. The goal is to make `search_files` and its sibling tools implicitly surface files and symbols that ride together in git history, without the caller having to pass `changed_with=` explicitly.

**User goals (stated verbatim):**
- Reliability
- Enhancement (meaningful over current SymForge capability)
- Token saving (one agent call instead of two)
- Superior to whatever's on the market (the user clarified "perfection" to mean this)

**Market gap we're targeting:** sub-100ms symbol-level coupling in an agent loop, with implicit query-derived anchoring, as a first-class MCP primitive. CodeScene has symbol-level coupling but it's offline static analysis. Sourcegraph/JetBrains/Cursor do not expose this shape. See `docs/ideas/cochange-rerank.md` for the full positioning.

**Source of the idea:** Obsidian vault note `wiki/concepts/SymForge Co-Change Signal Fusion.md` authored by the user earlier today. The note proposed a scoped v1 (explicit `anchor_path` only, SQLite-backed, new `rank_by` param). We reshaped the scope to be more ambitious and drop the parts that didn't serve the stated goals — see `docs/ideas/cochange-rerank.md` and `docs/plans/cochange-coupling-execution.md` for the rationale trail.

## Architecture — 4 tentacles, current position

1. **Tentacle 1** — Storage + best-effort symbol identity ← **current tentacle, Steps 1.1–1.3 done**
2. Tentacle 2 — Signal-computation refinement (weight tuning, multi-factor signals) — not yet started
3. Tentacle 3 — Ranker fusion + full integration (`search_files` / `search_symbols` / `find_references` / `get_symbol_context` rerank, `changed_with=` migration) — not yet started
4. Tentacle 4 — New `mcp__symforge__coupling` MCP tool + ADR + golden rebaseline + benchmarks — not yet started

The feature is intentionally additive and **not yet wired into the ranker** at this stage. `CoChangeSignal::score` still returns `0.0`. Existing rank-signal golden tests are unchanged. Every piece delivered so far is inert until Tentacle 3.

## What landed in Steps 1.1–1.3 (the scope you are reviewing)

### Step 1.1 — Storage scaffolding + schema
- `src/live_index/coupling/mod.rs` — module root. Defines `AnchorKey` (a namespaced string key: `"file:<rel-path>"` or `"symbol:<rel-path>#<name>#<kind>"`) with Windows backslash normalisation, and `Granularity` enum.
- `src/live_index/coupling/schema.rs` — SQLite DDL:
  - `coupling_meta (key TEXT PK, value TEXT)` — schema version, last-indexed HEAD oid, cold-build timestamp.
  - `coupling (anchor_key, partner_key, shared_commits, weighted_score, last_commit_ts)` with composite primary key and `idx_coupling_anchor_score (anchor_key, weighted_score DESC)` index.
  - `CURRENT_SCHEMA_VERSION = 1`.
- `src/live_index/coupling/store.rs` — `CouplingStore` wraps `Arc<Mutex<rusqlite::Connection>>`. Public API: `open`, `open_in_memory`, `schema_version`, `upsert`, `bulk_upsert` (additive transactional), `replace_all_rows` (atomic rebuild: DELETE + INSERT in one transaction, preserves `coupling_meta`), `query` (ordered by `weighted_score DESC` with a `LIMIT`), `last_head`, `set_last_head`, `cold_built_at`, `set_cold_built_at`. Edges are stored in **both directions** for O(1) single-anchor lookups.
- `src/live_index/mod.rs` — added `pub mod coupling;`.

### Step 1.2 — File-level cold-build walker
- `src/live_index/coupling/walker.rs` — `cold_build(store, repo_root, cfg)`.
- Walks git history via the existing `crate::git::GitRepo::log_with_stats`, which wraps `git2::Revwalk`. Graceful early-return for empty repos (no HEAD).
- `WalkerConfig`: `max_commits` (default 500), `window_days` (default ~3 years), `half_life_days` (default 30), `max_files_per_commit` (default 200), `reference_ts` (injected for testing).
- Weighting: per-pair per-commit contribution = `size_weight(file_count) * time_decay(reference_ts, commit_ts, half_life)`.
  - `size_weight = 1 / log2(file_count + 1)` — small focused commits dominate sprawling refactors.
  - `time_decay = exp(-(ref - t) * ln2 / H)` with defensive clamping to 1.0 for future / equal timestamps. Mathematically time-shift invariant (see the reasoning section).
- Aggregator: `HashMap<(anchor_str, partner_str), Acc { shared_commits, weighted_score, last_commit_ts }>`, flushed via `replace_all_rows` in one transaction (DELETE existing + INSERT new). On a no-HEAD repo, still calls `replace_all_rows(&[])` so stale rows are purged.
- Records HEAD oid to `coupling_meta.last_indexed_head_oid` after a successful build.

### Step 1.3 — Best-effort symbol identity + symbol-level rows
- New `WalkerConfig.include_symbols: bool` flag, default false (opt-in, preserves Step 1.2 behaviour for existing callers).
- When true, `cold_build` dispatches to `cold_build_with_symbols`, which uses `git2::Repository` directly (not `log_with_stats`) so per-hunk line-ranges are available via `git2::Patch`.
- For each commit:
  - File-level anchors are emitted exactly like Step 1.2.
  - Symbol-level anchors: for each changed file with a supported tree-sitter language, the **new-side blob** is parsed via `crate::parsing::parse_source` (which was `fn parse_source` — made `pub(crate)` to enable this). Enclosing symbols for each hunk's new-side line range are resolved via `crate::domain::find_enclosing_symbol` with 1-based-to-0-based line conversion (git hunks use 1-based, `SymbolRecord.line_range` uses 0-based tree-sitter row numbers).
  - Pair emission: separate `emit_pairs` calls for file-level anchors and symbol-level anchors. No file-to-symbol cross-pairs (semantically noisy).
- Degraded behaviour — file still contributes at file level when any of: unknown extension, config language (`Json | Toml | Yaml | Markdown | Env` — all `unreachable!` in `parse_source`), binary blob, invalid UTF-8, or parse failure.
- Intra-file symbol pairs (e.g., `symbol:a.rs#alpha#fn` ↔ `symbol:a.rs#unused#fn` when both change in the same commit) **are emitted by design**. Filter in a follow-up if telemetry shows it's noise.

## Reasoning behind non-obvious choices

1. **Bidirectional edge storage** — we write both `(a, b)` and `(b, a)`. Doubles row count, but the hot query `"top partners of anchor A"` becomes a single index scan (`WHERE anchor_key = ?`) rather than a `UNION` over two predicates. Storage is cheap; query-path simplicity was the winner.
2. **SQLite not yet shared with frecency** — the `FrecencyStore` (in `src/live_index/frecency.rs`) already uses a per-workspace SQLite file. The plan allows for sharing the backend later. We did not fold the coupling tables into the frecency DB yet because frecency's wiring to the workspace lifecycle is already live and additive risk is lower if we keep them separate until Tentacle 3.
3. **In-memory accumulator + `replace_all_rows`, not per-commit insert** — minimises SQLite round-trips during cold build. `replace_all_rows` wraps `DELETE FROM coupling` + bulk INSERT in one transaction so cold-build is atomic-rebuild semantics (no stale rows survive across reruns or HEAD loss). `bulk_upsert` is retained for future incremental HEAD-delta updates (Step 1.4).
4. **Time-shift invariant decay math** — `exp` is multiplicatively composable across reference-time shifts. This lets Tentacle 1.4's incremental HEAD-delta updates rescale existing `weighted_score` values cheaply: `new_score = old_score * exp(-(t_ref_new - t_ref_old) * ln2 / H) + new_contribution`. Cold-build uses `reference_ts = now` (captured once per build).
5. **Best-effort symbol identity, not semantic identity** — we key symbols by `(file, name, kind)` from the **current** blob. We do NOT track symbol renames or moves across commits. This is called out in `docs/ideas/cochange-rerank.md` as a deliberate trade — the full semantic-identity tracker is a multi-week project. The user explicitly signed off on "best-effort first; full tracker deferred".
6. **Config languages filtered before `parse_source`** — the function has `unreachable!` arms for `Json | Toml | Yaml | Markdown | Env`. Rather than risk a panic on an unexpected future caller, we added `language_supports_parsing` as a cheap gate. Config-extension files still contribute at file level.
7. **Only new-blob parsing in 1.3** — pure-deletion commits (where a symbol is removed and nothing replaces it) aren't credited at symbol level yet. Listed in the plan as deferred to Step 1.3.1, not blocking 1.4. Rationale: the 80% case (modifications + additions) is correct; old-blob parsing doubles the parsing cost and deserves its own benchmarking round.
8. **Env-flag gating deferred** — `CoChangeSignal::score` still returns `0.0` and nothing in the tools layer consumes the new store. There's no runtime gate needed yet because the feature is dead code from the user's perspective until Tentacle 3 wires it up. When it does, the gate will follow the pattern established by `FrecencyBumpHook` (env var like `SYMFORGE_FRECENCY=1`).

## Files affected

New files:
- `src/live_index/coupling/mod.rs` (~100 LOC including tests)
- `src/live_index/coupling/schema.rs` (~30 LOC)
- `src/live_index/coupling/store.rs` (~260 LOC including tests)
- `src/live_index/coupling/walker.rs` (~900 LOC including tests)
- `docs/ideas/cochange-rerank.md` — refined idea doc, 4-tentacle scope
- `docs/plans/cochange-coupling-execution.md` — execution plan, marks 1.1/1.2/1.3 as DONE
- `docs/plans/cochange-coupling-review-prompt.md` — this file

Edited files:
- `src/live_index/mod.rs` — added `pub mod coupling;`
- `src/parsing/mod.rs` — changed `fn parse_source` to `pub(crate) fn parse_source`

Untouched intentionally:
- `src/live_index/rank_signals.rs` — `CoChangeSignal::score` still returns 0.0; the `RankCtx.target_path` field is still populated by the existing `changed_with=` path; no goldens altered.
- `src/live_index/query.rs::capture_search_files_view` — no rerank integration yet.
- `src/protocol/tools.rs::search_files` — `changed_with=` branch unchanged (migration is Tentacle 3).
- `src/live_index/git_temporal.rs` — the existing shell-out-based co-change path is left alone; migration is Tentacle 3.
- `src/live_index/frecency.rs` — untouched; coupling is orthogonal for now.

## How to run verification

```
cargo check
cargo test --all-targets -- --test-threads=1
```

Expected: `cargo check` clean, `cargo test` green. Before this work: 1521 lib tests. After: **1571 lib tests + every integration suite**. All 50 new tests are under `live_index::coupling::{store,walker,tests}::*`, with no duplicate-attribute warnings. Tentacle 1 Steps 1.1–1.4 are all shipped; Step 1.4 implements the commit-scoped ledger design (exact bounded-window maintenance via `coupling_active_commits` + `coupling_commit_edges` with reference-time rescaling and commit-scoped subtraction on eviction).

## What to focus on

Primary review axes:
1. **Correctness of the weighting math** — particularly `time_decay`, `size_weight`, and the aggregator logic. Is the "time-shift invariant" claim actually true as composed? Edge cases on commits with `commit_ts >= reference_ts` (clock skew, rebases with future authored dates)?
2. **1-based vs 0-based line conversion** — `cold_build_with_symbols` converts git hunk `new_start` (1-based) to `find_enclosing_symbol`'s 0-based expectation via `saturating_sub(1)`. Is the range `start_0..=end_0` correct, especially the `end_0 = new_start + new_lines - 1 - 1` computation? Off-by-one risk.
3. **Concurrency / correctness of `Arc<Mutex<Connection>>`** — is the `.expect("coupling mutex poisoned")` pattern acceptable here, or should we return `Result` on poison? (Frecency store uses the same pattern; we matched it for consistency.)
4. **SQL correctness** — `bulk_upsert`'s `ON CONFLICT` semantics with `excluded.*`. Composite primary key scan cost. Does the `idx_coupling_anchor_score` index actually cover the hot query (SQLite query-planner behaviour)?
5. **Git diff edge cases** — merge commits (currently diffed against first parent only; no special handling), empty diffs, binary files (skipped), submodule changes, path-rename events (default libgit2 diff shows them as delete+add, so we still emit a pair for old_path ↔ new_path — intentional, since it's a legitimate coupling event).
6. **Memory bounds during cold-build** — `HashMap` grows with the unique-pair count. On a repo with 500 commits × 20 files/commit × 2 directions that's O(200k) entries peak. Acceptable? Would a per-commit flush change the arithmetic?
7. **Test coverage gaps** — tree-sitter parse failures, UTF-8 failures, binary blobs. I added tests for unknown extension and config languages; explicit tests for binary blob handling and UTF-8 failures don't exist yet.
8. **Intra-file symbol pair design decision** — is emitting `(foo.rs#alpha#fn, foo.rs#beta#fn)` from a two-function same-file change the right default, or should the walker filter these out? I argued "legitimate coupling signal"; legitimate counter-argument is "noisy within the same file, prefer inter-file coupling only".

Out of scope for this review:
- Tentacle 3's ranker fusion design (the rerank contract itself). It's in `docs/ideas/cochange-rerank.md` but no code for it yet.
- Tool layer / MCP surface (Tentacle 4).
- Whether to keep the `changed_with=` branch alive after migration (deferred to Tentacle 3).

## Known follow-ups (intentionally deferred)

- Step 1.3.1 — old-blob parsing to attribute pure-deletion commits at symbol level.
- Step 1.4 — incremental HEAD-move delta update (next step in this tentacle).
- Rename detection via `DiffOptions::find_similar` — currently unused; pairs still emit correctly for rename events via the delete+add default, but we miss the rename edge.
- Optional intra-file symbol-pair filtering, gated by a config flag.
- Old-blob + new-blob union for symbol attribution (one line change maps to exactly one enclosing symbol today via new-side; robustness to edit patterns could improve).

---

## Files to read before reviewing

Read in this order to build context efficiently:
1. `docs/ideas/cochange-rerank.md` — the what and why (one-pager, 5 minutes)
2. `docs/plans/cochange-coupling-execution.md` — sequencing and goal-backward acceptance criteria
3. `src/live_index/coupling/mod.rs` — types
4. `src/live_index/coupling/schema.rs` — DDL
5. `src/live_index/coupling/store.rs` — persistence API + tests
6. `src/live_index/coupling/walker.rs` — walker logic + tests (largest file)

Please return your findings as a structured review with severity tagged per item (blocking / major / minor / nit).
