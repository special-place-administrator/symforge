---
type: design
title: Tentacle 3 Phase 3.2 — Coupling Store Query Helpers
created: 2026-04-18
tentacle: T3
phase: 3.2
parent_plan: docs/plans/cochange-coupling-execution.md
adr: docs/decisions/0013-coupling-signal-contract.md
prior_phase: docs/plans/cochange-coupling-3.1-design.md
status: approved-with-followups — reviewer sign-off 2026-04-18
---

# Phase 3.2 — Coupling Store Query Helpers (design)

Strict scope: add the read-side substrate 3.3 will consume when it implements the `search_files` rerank against ADR 0013. **No behaviour change**: no `RankCtx` touches, no `CoChangeSignal::score()` changes, no `capture_search_files_view()` changes, no `SearchFilesHit` population, no `protocol/tools.rs` query-path edits, no golden touches.

## Reuse vs new

| Concern | Current state | 3.2 choice |
|---|---|---|
| Per-anchor top-N query | `CouplingStore::query(anchor, limit)` at `src/live_index/coupling/store.rs:212` — returns all partners ordered by `weighted_score DESC`, no floor | **Keep signature stable.** 45+ existing call sites across walker/store tests would break on a signature change, and a signature break violates the "read-side substrate only" phase cut. Add a new `query_with_floor(anchor, limit, shared_commits_min)` helper instead; `query` stays as the no-floor form |
| Shared-commits floor (ADR rule 1) | Not exposed | New method `query_with_floor`. File-level callers pass 2, symbol-level callers pass 3. The numeric policy lives in the caller; the store just filters |
| Partner cap (ADR rule 2) | Already supported via `limit` param | Keep as-is |
| Pairwise lookup (ADR rule 4 — symbol-gated-by-file) | Not exposed | New `pair_row(anchor, partner) -> Option<CouplingRow>`. Required so the rerank can ask "does this symbol-level pair's file-level pair also exist and pass the file floor?" |
| Chore denylist (ADR rule 3) | Not a store concern | Stays caller-side in 3.3 — store returns rows; caller filters anchor keys |
| Relative ordering (ADR rule 6) | Already satisfied by `weighted_score DESC` + `limit` | Keep as-is |
| Same-process store cache (`cached_store_for`) | Deferred in 3.1 | **Still deferred.** 3.2's only reader is the store's own test suite; each test opens fresh. Defer to 3.3 when the rerank becomes a per-query hot path and latency evidence justifies the cache. Documented in 3.1 design doc |
| `AnchorKey` granularity tagging | Already supported — `"file:..."` vs `"symbol:..."` prefixes | No change |

## Helper surface

Two new methods, no signature changes. Both land on `CouplingStore` in `src/live_index/coupling/store.rs`.

### 1. `query_with_floor(anchor, limit, shared_commits_min)` — new method

```rust
/// Top-N partners for `anchor`, ordered by `weighted_score DESC`, with
/// a minimum `shared_commits` floor applied at the SQL layer.
///
/// Implements ADR 0013 rule 1 (shared-commits floor) and rule 2 (per-anchor
/// partner cap). Callers are responsible for rules 3 (chore denylist —
/// filter anchor-side before calling), 4 (symbol-gated-by-file — compose
/// with `pair_row`), 5 (anchor-confidence gate — applies before rerank
/// enters the store), and 6 (relative ordering — satisfied by the
/// `weighted_score DESC` order already).
///
/// `shared_commits_min = 0` degenerates to the same rows `query` returns.
/// File-level callers pass 2; symbol-level callers pass 3 (per ADR rule 1).
pub fn query_with_floor(
    &self,
    anchor: &AnchorKey,
    limit: u32,
    shared_commits_min: u32,
) -> Result<Vec<CouplingRow>>;
```

SQL: same shape as `query` with an added `AND shared_commits >= ?3` predicate before `ORDER BY`. The existing `idx_coupling_anchor_score (anchor_key, weighted_score DESC)` index remains correct — SQLite applies the `shared_commits` predicate during index scan.

**`query` stays stable.** 45+ existing call sites in `src/live_index/coupling/walker.rs` and `store.rs::tests` keep calling `query(anchor, limit)` unchanged. No production consumer hits either method yet — `CoChangeSignal::score()` still returns 0.0, so no rerank path enters the store.

### 2. `pair_row(anchor, partner)` — new method

```rust
/// Look up a single coupling row for the exact pair `(anchor, partner)`.
/// Returns `None` when the pair is absent (either endpoint unknown, or
/// the pair was never co-modified within the bounded window).
///
/// Required by ADR rule 4 — symbol-gated-by-file. A caller holding a
/// symbol-level pair `(sym_a_in_f1, sym_b_in_f2)` looks up the
/// corresponding file-level pair `(file:f1, file:f2)` and only credits
/// the symbol-level pair if the file-level pair exists and meets the
/// file-level floor. Callers apply the floor; the store just answers
/// "does this row exist, and what are its values?".
pub fn pair_row(
    &self,
    anchor: &AnchorKey,
    partner: &AnchorKey,
) -> Result<Option<CouplingRow>>;
```

SQL: `SELECT anchor_key, partner_key, shared_commits, weighted_score, last_commit_ts FROM coupling WHERE anchor_key = ?1 AND partner_key = ?2 LIMIT 1`. Primary key `(anchor_key, partner_key)` makes this O(1) lookup.

**Why `Option<CouplingRow>` and not `Result<bool>`?** Rule 4 needs to know "does the pair pass the file-level floor?", which is not just existence — the floor value depends on granularity. Returning the row lets the caller check `shared_commits >= floor` without a second query. One well-shaped method covers both "existence?" and "passes floor?" cases.

**Why two endpoints, not one?** A future "partner neighbourhood" query would benefit from indexing the reverse direction (`partner_key, anchor_key`), but that's a query pattern for hypothetical callers. 3.2 only needs the exact-pair lookup.

## Why no lifecycle.rs or mod.rs changes

The store caches itself internally via `Arc<Mutex<Connection>>`. 3.3's rerank will either:
(a) Open the store per-query (simple; adds ~microseconds SQLite open cost per `search_files` call), or
(b) Cache via `cached_store_for` if evidence shows option (a) is a bottleneck.

Either choice is 3.3's to make based on latency data. 3.2 must not pre-commit to (b) — doing so either ships dead code or locks 3.3 into one strategy before it has the profile evidence.

The `pub use` in `src/live_index/coupling/mod.rs` already re-exports `CouplingStore`, so new methods are reachable by importers without any module-level change.

## Failure semantics

Every read helper is a pure SQLite read; failure modes are the same as the existing `query` and other readers on `CouplingStore`:

| Case | Behaviour |
|---|---|
| Store file missing / unopenable | Caller responsibility — `CouplingStore::open` returns `Err`; helpers never see this |
| Anchor unknown (no rows) | `query` returns empty `Vec`; `pair_row` returns `Ok(None)` |
| Partner unknown for a known anchor | `pair_row` returns `Ok(None)` |
| `shared_commits_min` larger than any stored value | `query` returns empty `Vec` |
| SQLite read error (corruption, lock timeout) | Propagated as `Err` — matches `query`'s current behaviour. Caller in 3.3 is responsible for drop-and-degrade-to-no-rerank |
| Empty store (zero rows) | Both helpers return Ok with empty result |
| Pair present with `shared_commits < floor` | `query` filters it out; `pair_row` still returns the row (caller enforces the floor) |

**Concurrency:** `Arc<Mutex<Connection>>` serialises every DB touch. Reads never contend with each other (SQLite WAL-or-journal handles concurrent readers via the connection pool; ours is a single connection, so the mutex is the bottleneck). Acceptable for 3.2 — optimisation is a 3.3 concern if profiling demands it.

## Test plan

All tests land in `src/live_index/coupling/store.rs::tests` — colocated with the code they exercise, matching the existing pattern. **No existing test migrations** — `query()` signature is stable, so prior tests stay exactly as written.

### New tests

| # | Test | Asserts |
|---|---|---|
| 1 | `query_with_floor_excludes_weak_pairs` | Store has pairs with `shared_commits` = 1, 2, 3; `query_with_floor` with `shared_commits_min = 2` returns only the 2+ pairs |
| 2 | `query_with_floor_keeps_strong_pairs` | Store has pairs with `shared_commits` = 3, 5, 10; `shared_commits_min = 3` returns all three |
| 3 | `query_with_floor_zero_matches_query_behavior` | Store with mixed `shared_commits`; `query_with_floor(anchor, limit, 0)` returns the same rows as `query(anchor, limit)` — property check against the stable method to prove floor=0 is semantically equivalent |
| 4 | `query_with_floor_preserves_weighted_score_ordering` | Store with rows whose `weighted_score` decreases as `shared_commits` decreases; `shared_commits_min = 2` filters the weakest row AND the remaining rows stay in `weighted_score DESC` order |
| 5 | `query_with_floor_above_max_returns_empty` | `shared_commits_min` higher than any stored value → empty Vec |
| 6 | `pair_row_returns_row_when_pair_exists` | Insert `(a, b, shared=5, ws=2.5, ts=T)`; `pair_row(a, b)` returns `Some(row)` with those exact fields |
| 7 | `pair_row_returns_none_for_absent_pair` | Insert `(a, b)`; `pair_row(a, c)` where `c` is unknown returns `Ok(None)` |
| 8 | `pair_row_returns_none_for_unknown_anchor` | Empty store; `pair_row(x, y)` returns `Ok(None)` |
| 9 | `pair_row_is_directional` | Insert only `(a, b)` via `upsert` (not `bulk_upsert`, which is symmetric in production); `pair_row(b, a)` returns `None`. Pins the exact PK contract at `src/live_index/coupling/schema.rs:22` — the schema is keyed by ordered `(anchor_key, partner_key)`, and rule 4 composition depends on the direction being respected |
| 10 | `pair_row_does_not_filter_by_shared_commits` | Insert `(a, b, shared=1)`; `pair_row(a, b)` returns `Some(row)` with `shared_commits = 1` — floor is caller-applied, not store-enforced on `pair_row` |

Test count: 10 new tests in `store.rs::tests`.

## Touched files

| File | Change |
|---|---|
| `src/live_index/coupling/store.rs` | Add `query_with_floor` and `pair_row` methods. `query` signature unchanged. Add 10 new tests. No existing tests modified. |
| `docs/plans/cochange-coupling-3.2-design.md` | This design doc |
| `docs/plans/cochange-coupling-execution.md` | Progress log entry after 3.2 lands |

Explicitly **not** touched:
- `src/live_index/coupling/lifecycle.rs` — no read call yet; rerun only via tick/boot
- `src/live_index/coupling/mod.rs` — re-exports are stable
- `src/live_index/coupling/schema.rs` — index `idx_coupling_anchor_score` already suits the new floor predicate; no DDL change
- `src/live_index/rank_signals.rs`, `src/live_index/query.rs`, `src/protocol/tools.rs`, `tests/rank_signal_behavior.rs` — these are 3.3 territory. If the diff shows any change to them, block the PR.

## Reviewer resolutions (2026-04-18)

1. **`query` signature change vs new method** — RESOLVED: keep `query()` stable, add `query_with_floor()` as a new helper. Rationale: 45+ existing call sites across `walker.rs` tests and `store.rs::tests`; breaking signature forces churn outside the phase boundary, which violates the "read-side substrate only" cut.
2. **`pair_row` directionality** — RESOLVED: keep test 9. The store schema is keyed by ordered `(anchor_key, partner_key)` at `src/live_index/coupling/schema.rs:16-22`, so `pair_row` is an exact PK lookup primitive, not an unordered graph API. ADR rule 4 depends on direction being respected. Test writes the one-way row via `upsert` (not `bulk_upsert`) to prove the contract cleanly.
3. **Deferred cache** — CONFIRMED: no production read path in 3.2; cache reintroduction would still be dead code. Revisit when 3.3 makes per-query reads hot.

## Next

Implement → tests green → advisor review → reviewer review → land.
