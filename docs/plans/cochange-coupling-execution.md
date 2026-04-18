---
type: plan
title: Co-Change Coupling — Execution Plan
created: 2026-04-18
status: in-progress
idea: docs/ideas/cochange-rerank.md
---

# Co-Change Coupling — Execution Plan

Execution plan for the idea in `docs/ideas/cochange-rerank.md`.

## Ground truth from current codebase

Verified against the tree at `main` (commit `29eb9e1` or later):

- **`RankCtx.target_path: Option<&'a str>`** already exists (`src/live_index/rank_signals.rs:46`) — the explicit anchor plumbing from `changed_with=` *already flows* through `RankCtx`. We don't need to add the field.
- **`SearchFilesInput.changed_with`** field already exists (`src/protocol/tools.rs:381`).
- **`CoChangeSignal::score`** returns `0.0` unconditionally (`src/live_index/rank_signals.rs:173-175`) — the placeholder the feature note described.
- **Deps already in `Cargo.toml`:** `git2 = "0.20"` with `vendored-libgit2`, `rusqlite = "0.32"` with `bundled`. **No new native deps required.**
- **Tree-sitter grammars already linked** for every language we parse live — historical blob parsing is possible without adding deps.
- **Existing integration point:** `capture_search_files_view` at `src/live_index/query.rs:1379-1623` — single function, clean boundary for the rerank pass.

## Tentacle 1 — Storage + Best-Effort Symbol Identity

**Deliverable:** a populated `coupling` SQLite table on cold index, incremental HEAD-delta updates on watcher events, queryable via a `CouplingStore` API. No rerank integration yet.

### Step 1.1 — Scaffolding + schema (DONE 2026-04-18)

**Files to create:**
- `src/live_index/coupling/mod.rs` — public surface: `CouplingStore`, `AnchorKey`, `Granularity`, `CouplingRow`.
- `src/live_index/coupling/schema.rs` — schema DDL string + migration version constant.
- `src/live_index/coupling/store.rs` — `CouplingStore` struct wrapping `rusqlite::Connection` with `open`, `query`, `upsert`.

**Files to edit:**
- `src/live_index/mod.rs` — add `pub mod coupling;`.

**Schema v1:**
```sql
CREATE TABLE IF NOT EXISTS coupling_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS coupling (
  anchor_key     TEXT NOT NULL,   -- "file:rel/path" | "symbol:rel/path#name#kind"
  partner_key    TEXT NOT NULL,
  shared_commits INTEGER NOT NULL,
  weighted_score REAL NOT NULL,   -- temporal-decayed + size-weighted
  last_commit_ts INTEGER NOT NULL,
  PRIMARY KEY (anchor_key, partner_key)
);

CREATE INDEX IF NOT EXISTS idx_coupling_anchor_score
  ON coupling (anchor_key, weighted_score DESC);
```

- `coupling_meta` holds `schema_version`, `last_indexed_head_oid`, `cold_build_completed_at` — the provenance needed for HEAD-move invalidation without a full rebuild.
- Composite PK preserves directionality: we store both `(a→b)` and `(b→a)` rows so a single-anchor lookup is one index scan.
- `idx_coupling_anchor_score` supports the hot query: `SELECT partner_key FROM coupling WHERE anchor_key = ? ORDER BY weighted_score DESC LIMIT ?`.

**Acceptance for Step 1.1:**
- [ ] `cargo check` passes.
- [ ] `cargo test --all-targets -- --test-threads=1` passes, including new tests:
  - `open_creates_fresh_schema_on_empty_db`
  - `open_reuses_existing_schema_when_versions_match`
  - `upsert_then_query_roundtrip`
  - `query_returns_ordered_by_weighted_score_desc`
  - `query_limits_respected`
- [ ] `CouplingStore::open(path)` creates the DB file and writes `schema_version=1` to `coupling_meta`.
- [ ] No changes to ranker behavior. Existing goldens unchanged.

### Step 1.2 — Cold-build walker (libgit2) (DONE 2026-04-18)

**Files to create:**
- `src/live_index/coupling/walker.rs` — bounded revwalk, diff extraction, pair generation.

**Behavior:**
- Walk last N=500 commits from HEAD (N configurable via env `SYMFORGE_COUPLING_COLD_N`, default 500).
- For each commit, compute the changed-path set via libgit2 `Diff`; skip merges unless they touch paths not present in either parent.
- Emit `(path_a, path_b, commit_ts, file_count_in_commit)` tuples for every unordered pair; commit-size weighting = `1 / log2(file_count + 1)`.
- Aggregate into in-memory hashmap `(anchor_key, partner_key) → Acc{shared, weighted, last_ts}` then bulk-insert.
- File-level only in this step. Symbol-level lands in Step 1.3.

**Acceptance:**
- [ ] Cold-build over SymForge's own git history completes in <30 s on a warm FS cache.
- [ ] Pair count for a synthetic repo (fixture) matches hand-computed expectation.
- [ ] Merge-commit path dedup verified by fixture.

### Step 1.3 — Best-effort symbol identity + symbol-level rows (DONE 2026-04-18)

Shipped:
- `WalkerConfig.include_symbols` flag, default false (opt-in).
- New symbol-aware path `cold_build_with_symbols` using `git2` revwalk + per-hunk `Patch` extraction.
- Blob parsing via `crate::parsing::parse_source` (now `pub(crate)`), enclosing-symbol mapping via `crate::domain::find_enclosing_symbol` with 1-based-to-0-based line conversion.
- `AnchorKey::symbol(file, name, kind)` form populated for every changed symbol in every commit.
- Config languages (`Json`, `Toml`, `Yaml`, `Markdown`, `Env`) filtered before parse to honour the `unreachable!` invariant; they still contribute at file level.
- Unsupported extensions, binary blobs, and UTF-8 failures degrade gracefully to file-level only.
- 7 new tests under `walker::tests::cold_build_with_symbols_*` covering: Rust-function pair emission, unknown-extension fallback, config-language fallback, unmodified-file exclusion, extension parser, language filter.

Deferred to Step 1.3.1 (not blocking Step 1.4):
- Old-blob parsing for pure-deletion commits (currently only new-blob symbols are credited).
- Rename detection via libgit2 similarity metrics.
- Intra-file symbol-pair filtering if telemetry shows it's noise (currently emitted by design).

**Landed implementation (see `src/live_index/coupling/walker.rs::cold_build_with_symbols`):**
- Added `WalkerConfig.include_symbols: bool` flag (default false). When true, `cold_build` dispatches to the symbol-aware path.
- Symbol-aware path uses `git2::Repository` directly so per-hunk line ranges are available via `git2::Patch::hunk`. Diff context is set to 0 lines to avoid attributing unchanged neighbours to the commit.
- For each changed file with a tree-sitter-supported language, parses the **new-side blob** via `crate::parsing::parse_source` (made `pub(crate)`) and maps hunk line ranges to enclosing symbols via `crate::domain::find_enclosing_symbol` (with 1-based → 0-based line conversion).
- File-level and symbol-level pairs are emitted **independently** with size-weighted contributions based on each set's own cardinality. Single-file commits still emit intra-file symbol pairs; multi-file commits without parseable symbols still emit file-level pairs.
- Config-language extensions (`Json | Toml | Yaml | Markdown | Env`) are filtered before `parse_source` to honour its `unreachable!` invariant; they still contribute at file level.

**Deferred to follow-up work:**
- Old-blob parsing for pure-deletion commits (currently only new-blob symbols are credited).
- Rename detection via libgit2 similarity metrics / half-weight fallback.
- Intra-file symbol-pair filtering if telemetry shows it's noise (currently emitted by design — legitimate coupling signal).

**Acceptance (achieved):**
- [x] Historical blob parse does not panic on config languages (filtered) or malformed source (`parse_source` returns Err → file-level fallback).
- [x] 7 tests under `walker::tests::cold_build_with_symbols_*` covering: multi-file Rust-function pair emission, single-file intra-file pair emission (regression), unknown-extension fallback, config-language fallback, unmodified-file exclusion.

### Step 1.4 — Incremental HEAD-move delta update (DONE 2026-04-18, rev 2)

**Design — commit-scoped ledger for exact bounded-window maintenance.**

Previous draft used "detect bounded-window risk and fall back to cold-build" — reviewer rejected as weakening correctness. This rev implements the full ledger design: the aggregate `coupling` table stays the hot-query surface, but every contribution is tracked in a per-commit ledger so delta can both **add** incoming commits and **subtract** outgoing ones (commits that fell out of the bounded window) at the correct reference time.

**Schema additions (still schema_version=1, no migration):**
- `coupling_active_commits (commit_oid PK, commit_ts)` — the set of commits currently inside the bounded window.
- `coupling_commit_edges (commit_oid, anchor_key, partner_key, shared_inc, base_weight, commit_ts, PK(oid, anchor, partner))` — per-commit pair contributions with reference-time-neutral `base_weight = size_weight(anchor_count)`.
- `idx_ledger_pair` on `(anchor_key, partner_key)` supports the `MAX(commit_ts)` recompute.
- New meta key: `last_reference_ts` (the reference_ts the aggregate was computed against).

**Unified commit selection:** both cold-build and delta now go through the same `compute_window` helper using `git2` directly. `cfg.reference_ts` is honoured uniformly for both cutoff and decay. The file-only `log_with_stats` path was removed from the coupling module.

**Reference-time-neutral math:**
- Ledger stores `base_weight` only (never decayed).
- Runtime contribution for commit `c` at reference `t`: `base_weight * exp(-(t - commit_ts) * ln2 / H)`.
- Decomposition: `score(t_new) = score(t_old) * exp(-(t_new - t_old) * ln2 / H)`. Rescaling existing aggregate rows is a single SQL UPDATE.

**Cold-build atomic transaction (`CouplingStore::commit_cold_build`):** DELETEs `coupling`, `coupling_active_commits`, `coupling_commit_edges`, inserts the full new state, writes `last_head` / `last_reference_ts` / `cold_built_at`.

**Delta atomic transaction (`CouplingStore::commit_delta`):**
1. Rescale all `coupling.weighted_score` by `exp(-(new_ref - old_ref) * ln2 / H)`.
2. For each outgoing commit, read its ledger edges and subtract `base_weight * exp(-(new_ref - commit_ts) * ln2 / H)` from the matching `coupling` row, decrement `shared_commits`.
3. DELETE outgoing rows from `coupling_commit_edges` and `coupling_active_commits`.
4. INSERT incoming into `coupling_active_commits` + `coupling_commit_edges`; upsert their contributions into `coupling` with additive semantics.
5. DELETE aggregate rows whose `shared_commits <= 0`.
6. For every touched pair, recompute `last_commit_ts = (SELECT MAX(commit_ts) FROM coupling_commit_edges WHERE anchor=? AND partner=?)`.
7. Update `last_head` and `last_reference_ts`.

**`apply_head_delta` flow:**
1. Read `old_head`, `old_reference_ts`, `old_active_commits` from store.
2. Call `compute_window(cfg)` → `new_active_commits`, per-commit ledger edges.
3. `incoming = new - old`, `outgoing = old - new`.
4. If head and reference_ts both match → `NoOp`.
5. Otherwise: single transaction via `commit_delta`. Returns `Applied { incoming_commits, outgoing_commits, rescale_factor }`.

**`DeltaOutcome` simplified:** `NoOp | Applied` — no `FallbackRebuild` variant. The algorithm is always correct; force-push / branch swap / empty-repo are handled by outgoing/incoming computation, not by bailing to cold-build.

**Tests (47 coupling tests, all passing) include:**
- `delta_evicts_commits_falling_out_of_bounded_window_matches_scratch` — the critical regression. `max_commits=2`; three commits force c1 eviction. Delta result matches fresh cold-build to 1e-9 on `weighted_score` and exactly on integer fields.
- `delta_deletes_pair_when_all_contributing_commits_evicted` — pair cleanup.
- `delta_matches_scratch_with_symbols_under_eviction` — symbol-level eviction parity.
- `delta_rescales_weighted_score_when_reference_ts_advances` — one half-life advance halves the score.
- `delta_on_non_ancestral_head_matches_scratch` — simulates force-push / branch swap.
- `delta_from_empty_store_matches_scratch_cold_build` — delta on fresh store = cold-build.
- `delta_repeated_application_matches_scratch` — 5-iteration stress test with `max_commits=3` (commits continually falling out); every intermediate state matches fresh cold-build.

**Acceptance:**
- [x] Cold-build on A, advance HEAD to B, apply delta → matches fresh cold-build at B (the original criterion).
- [x] Cold-build on A, advance HEAD through N commits forcing old commits out of bounded window → still matches fresh cold-build (the new correctness bar).
- [x] reference_ts drift → rescale produces correct scores.
- [x] HEAD-move handler is a sync function; blocking-thread deployment is a caller concern for Tentacle 3.

**Removed / no longer applicable:**
- `additive_upsert` (still on CouplingStore for low-level uses but not called by the delta path).
- `DeltaOutcome::FallbackRebuild` (algorithm no longer needs fallback).
- `replace_all_rows` is no longer called by cold-build (`commit_cold_build` is the single entry).

### Tentacle 1 exit criteria

- `CouplingStore` is populated for the workspace on first index.
- HEAD-move keeps it current without full rebuilds.
- Queryable via the internal API; no rerank integration yet.
- Tests pass, docs cross-reference, changelog entry drafted.

## Tentacle 2 — Signal Calibration (DONE 2026-04-18)

**Output:** contract for Tentacle 3, backed by evidence from three real repos (SymForge, tokio-rs/tokio, google/magika).

- **Harness:** `tests/coupling_calibration.rs` — gated integration test, no runtime surface. Runs `cold_build` on each corpus repo in both file-only and file+symbol modes, then queries the store directly for walker stats, partner-count distribution, score percentiles, hotspot anchors, top pairs, and weak-anchor quartiles.
- **Research doc:** `docs/research/coupling-calibration-2026-04-18.md` — per-repo evidence tables, cross-repo synthesis, noise-pattern analysis. Raw output preserved at `docs/research/fixtures/coupling-calibration-2026-04-18.raw.md`.
- **ADR:** `docs/decisions/0013-coupling-signal-contract.md` — pins the defaults Tentacle 3 implements against:
  - Weak-anchor `shared_commits` floor (2 file / 3 symbol)
  - Per-anchor partner cap (20)
  - Chore-denylist for anchors (lockfiles, CHANGELOG, release manifests, CI workflows)
  - Symbol-level pairs gated by file-level
  - Anchor-confidence gate (rerank no-op below `Basename` path-match tier)
  - Relative (not absolute) score thresholds
  - Preserved `WalkerConfig` defaults (half_life_days=30, max_commits=500, etc.)
  - Failure-mode guidance Tentacle 3 must test.

Key empirical findings driving the contract:
- `shared_commits = 1` dominates every repo (60–97% of rows) → weak-floor rule is high-leverage.
- Score scale varies ~18× across repos (SymForge max 41.6, tokio 2.3, magika 1.4) → no absolute thresholds.
- Top-scoring pairs in every repo are release-chore infrastructure (`Cargo.toml`↔`Cargo.lock`, `package.json`↔`.release-please-manifest.json`) → chore denylist needed.
- Symbol-level noise amplification real: tokio `shared=1` jumps 92% → 97% file→symbol → stricter symbol floor + file-level gate.

## Tentacle 3 — Ranker Fusion + Full Integration Pass

**Implements against `docs/decisions/0013-coupling-signal-contract.md` as the source of truth.** ADR rules 1, 2, 3, 4, 6, 7 are fixed contract. Rule 5 is provisional — must be validated in Phase 3.4 before shipping as default behaviour, or amended/dropped per the ADR's stated outcomes.

### Phase cut (reviewer-confirmed after round-1 findings)

**3.1 — Store lifecycle only.** Per-workspace DB path (distinct from but following the general shape of `src/live_index/frecency.rs::cached_store_for`). Open/init on index load. Cold-build bootstrap on first index of a new workspace. HEAD-move hook — treated as **new lifecycle work**, not a straight frecency copy (frecency currently exposes init/load only at `src/live_index/persist.rs`, no watcher-triggered HEAD-move template). Env gating via `SYMFORGE_COUPLING=1`. Tests: init, no-HEAD, repeated-HEAD, no-store. No ranker changes.

**3.2 — Store query helpers + scaffolding.** Optional if needed. Store-read helpers + tests only. **MUST NOT** thread coupling evidence through `RankCtx` — `CoChangeSignal` is already in the default registry at `src/live_index/rank_signals.rs:178` and `rank_signals::combine()` consumes any populated evidence, so any `RankCtx` wiring is immediately a behaviour change. Keep this phase behavior-free by construction: no `RankCtx` fields, no `SearchFilesHit` evidence that existing rankers consume.

**3.3 — First behaviour shift: `search_files` rerank + `changed_with=` migration + provisional rule 5.** Single phase. Three things land together:

1. **`search_files` rerank** implementing ADR rules 1 (floors), 2 (partner cap), 3 (chore denylist), 4 (symbol-gated-by-file), 6 (relative score ordering). Env-gated (`SYMFORGE_COUPLING=1`), default off.
2. **`changed_with=` migration** — the existing heuristic path in `src/protocol/tools.rs::3856` that drives coupling via `git_temporal` + Jaccard (labels it "heuristic coupling" at `tools.rs:1809`) is routed through the new fused path. Eliminates the "two incompatible meanings of co-change inside one tool" contract break.
3. **Provisional rule 5** (anchor-confidence gate) implemented behind a named constant so Phase 3.4 validation can swap it without reshipping. Per ADR §5.

Failure-mode tests required per ADR: empty store, weak/insufficient anchor, no qualifying partners, coupling DB unavailable.

**3.4 — Query-level validation for ADR rule 5.** Build a query-level calibration harness (analog of Tentacle 2's store-level harness but measuring rerank outcomes at varying path-match confidence tiers). Run on the same corpus (SymForge + tokio + magika). Outcome required: confirm `Basename`-tier threshold and promote rule 5 to [CALIBRATED], find a different threshold and amend the ADR, or show the gate is unnecessary and drop it. ADR amendment committed before Phase 3.5.

**3.5a — `search_files` contract cleanup.** Fix the `SearchFilesInput.rank_by` doc drift (already done pre-3.1 as the reviewer called out). Labels, golden adjustments from 3.3 migration fallout, `SearchFilesHit.coupling_score` / `shared_commits` populated correctly, any remaining doc/comment drift. Separate from 3.5b because this is finishing work on the already-integrated surface.

**3.5b — Extension to new consumers.** `search_symbols`, `find_references`, `get_symbol_context`. **Not just wiring** — these are new consumers:
- `search_symbols` uses its own sorter at `src/live_index/search.rs::757`, not `rank_signals::combine()`. Needs rerank integration design, not just `RankCtx` threading.
- `find_references` and `get_symbol_context` don't consume rank signals at all today. Coupling integration there is a new ordering contract, not an extension.

This is where the phase's risk concentrates. Each surface needs its own design round + goldens + tests.

**3.6 — Ship pass.** `proposed-destructive.md` (required by the rank-signal-tentacle contract before ordering-shifting default-on). Golden rebaseline, perf/latency re-validation, ADR finalization (including whatever rule 5 became in 3.4), env flag flipped to default-on. No separate 3.7 soft-launch phase — the destructive review is the launch gate.

### Default-on timing

Flip in 3.6. Covered by destructive review + goldens + perf validation + ADR amendments from 3.4.

### Pre-3.1 cleanups (LOW findings, addressed upfront)

- `SearchFilesInput.rank_by` docstring no longer claims coupling fusion already exists — updated 2026-04-18 to point forward at Tentacle 3 / ADR 0013.

## Tentacle 4 — `mcp__symforge__coupling` Tool + Observability

(Details deferred. New tool registration, ADR, proposed-destructive.md, golden rebaseline, benchmark pass.)

## Progress log

- **2026-04-18 (session 1)** — Steps 1.1–1.3 landed. Three review rounds resolved: single-file-commit symbol coupling fix, atomic rebuild semantics, no-HEAD purge.
- **2026-04-18 (session 1, rev 2)** — Step 1.4 rewritten to the exact bounded-window ledger design after reviewer rejection of the additive/fallback approach. New tables `coupling_active_commits` + `coupling_commit_edges`, new meta key `last_reference_ts`, new atomic `commit_cold_build` / `commit_delta` transactions on the store, unified `git2`-based `compute_window` helper. Reference-time rescaling, commit-scoped subtraction of evictions, and pair cleanup. `DeltaOutcome` simplified to `NoOp | Applied` (no fallback). Follow-up round fixed a no-HEAD meta-clear bug so a restored-same-HEAD sequence can no longer false-positive into `NoOp`.
- **Full lib suite 1571 tests passing, 50 coupling tests**, no duplicate-attribute warnings.
- **Tentacle 1 complete** — storage, cold-build (file + symbol), best-effort identity, and exact-window incremental HEAD-delta all shipped. `CoChangeSignal::score` still inert (0.0).
- **Tentacle 2 complete (2026-04-18)** — calibration harness, per-repo evidence on SymForge + tokio + magika, research doc + ADR 0013 pinning the contract Tentacle 3 consumes. Rule 5 explicitly [PROVISIONAL] — Tentacle 3 must validate in Phase 3.4.
- **Tentacle 3 Phase 3.1 complete (2026-04-18)** — store lifecycle landed per `docs/plans/cochange-coupling-3.1-design.md`. Per-workspace DB at `.symforge/coupling.db`, `SYMFORGE_COUPLING=1` env gate, per-workspace `HashMap<PathBuf, Arc<AtomicBool>>` in-flight guard covering both boot-init and the watcher's 30 s reconcile tick, synchronous `run_init` helper branching on `cold_built_at`, cheap `git::head_sha` pre-check before `apply_head_delta`, `std::thread::Builder::spawn` with explicit guard-release on spawn failure, silent-drop error policy. 12 new tests in `src/live_index/coupling/lifecycle.rs` (including `run_init_reports_err_on_unwriteable_dir` for store-open failure coverage). Landed alongside the earlier `SearchFilesInput.rank_by` docstring fix at `src/protocol/tools.rs:400`. 1583 lib tests passing overall; `cargo build --release` clean. No ranker behaviour change — `CoChangeSignal::score` still returns 0.0.
- **Tentacle 3 Phase 3.2 complete (2026-04-18)** — store query helpers landed per `docs/plans/cochange-coupling-3.2-design.md`. Two new methods on `CouplingStore`: `query_with_floor(anchor, limit, shared_commits_min)` implements ADR 0013 rule 1 at the SQL layer (single `AND shared_commits >= ?2` predicate on the existing `idx_coupling_anchor_score` index), and `pair_row(anchor, partner) -> Option<CouplingRow>` provides the exact PK lookup required for ADR rule 4 (symbol-gated-by-file composition). `query()` signature stable — now delegates to `query_with_floor(..., 0)` so all 45+ existing call sites in `walker.rs`/`store.rs::tests` stay unchanged. 10 new tests in `src/live_index/coupling/store.rs::tests` covering floor exclusion/retention, floor=0 ↔ `query()` equivalence, ordering preservation under filter, empty-result cases, pair existence, directional PK contract (inserted one-way via `upsert` so `pair_row(b, a)` returns None), and pair_row's no-floor semantics. Reviewer APPROVE-WITH-FOLLOWUPS (MEDIUM: keep `query` stable — applied; LOW: keep directional test — applied; LOW: cache still deferred — applied). 1593 lib tests passing overall (+10 delta, exact match); `cargo build --release` clean. No ranker behaviour change — `CoChangeSignal::score` still returns 0.0, no production consumer hits either helper yet.
- **Next — Tentacle 3 Phase 3.3** (first user-visible rerank: wire helpers into RankCtx / `CoChangeSignal::score`, compose rules 1–6, flip the gate).
- **Deferred** — Step 1.4 legacy followups from Tentacle 1: Step 1.3.1 old-blob parsing, rename detection.
