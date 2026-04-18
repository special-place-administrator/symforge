# ADR 0013 — Coupling Signal Contract

**Status:** Accepted (2026-04-18)
**Tentacle:** T2 (calibration). Consumed by T3 (ranker fusion + integration).
**Evidence:** `docs/research/coupling-calibration-2026-04-18.md` + `docs/research/fixtures/coupling-calibration-2026-04-18.raw.md`

## Context

Tentacle 1 shipped a persistent, commit-scoped coupling graph (`coupling`, `coupling_active_commits`, `coupling_commit_edges` tables in the coupling SQLite store) with exact bounded-window maintenance. `CoChangeSignal::score()` in `src/live_index/rank_signals.rs` still returns `0.0` because Tentacle 3 hasn't wired the signal through `RankCtx` yet.

Before Tentacle 3 implements that wiring, we need a contract that fixes:

- What coupling evidence counts as signal vs noise
- How the ranker gates a query on coupling evidence (when does it rerank, when does it pass through?)
- How per-anchor hotspots are bounded
- What pathological inputs must not break the ranker

Tentacle 2 ran the calibration harness on three real repos (SymForge, tokio, magika) to answer those questions empirically. This ADR pins the resulting defaults and guardrails.

## Decision

Tentacle 3 MUST implement the following contract when consuming coupling evidence from `CouplingStore`.

**Evidence grading.** Rules below are tagged **[CALIBRATED]** when they are directly backed by numbers in `docs/research/coupling-calibration-2026-04-18.md` (coupling-store distributions across the three corpus repos), or **[PROVISIONAL]** when they are heuristic guidance outside what the Tentacle 2 harness measured. Provisional rules must be empirically validated during Tentacle 3 implementation (by adding query-level calibration or by measuring rerank outcomes in integration tests). If a provisional rule doesn't hold up under that validation, this ADR MUST be amended before the rule ships as default behaviour.

### 1. Weak-anchor shared-commits floor **[CALIBRATED]**

Coupling pairs with `shared_commits < N` MUST be excluded from rerank contributions. Different floors per granularity:

- **File-level pairs:** `shared_commits >= 2`.
- **Symbol-level pairs:** `shared_commits >= 3`.

**Rationale:** `shared_commits = 1` is indistinguishable from chance on an active repo. Across the corpus:

| repo | `shared_commits = 1` share (file) | (file+symbol) |
|---|---:|---:|
| SymForge | 69.3% | 59.6% |
| tokio | 91.7% | 97.4% |
| magika | 82.7% | 84.6% |

Applying the floors removes the majority of noise without losing any evidence that actually compounds over multiple commits. The stricter symbol-level floor is justified by the symbol-level amplification visible in tokio's 92% → 97% jump: symbol-level fine granularity produces more single-coincidence pairs per commit.

### 2. Per-anchor partner cap **[CALIBRATED]**

When returning coupling partners for a given anchor, Tentacle 3 MUST cap results at:

- `max_partners_per_anchor = 20`, ordered by `weighted_score DESC`.

**Rationale:** SymForge's calibration showed 52% of file anchors having >100 partners (driven by a small, active repo with mega-files like `src/protocol/tools.rs`). Without a cap, rerank would flood top-K displays with low-signal partners. 20 is consistent with typical agent-facing top-K conventions and keeps the long tail out of every response.

### 3. Anchor chore-denylist **[CALIBRATED]**

Files matching any of the following patterns MUST NOT be used as **anchors** that drive rerank. They MAY appear as partners of other anchors.

Default denylist (both exact filenames and simple glob patterns):

- `Cargo.lock`
- `package-lock.json`
- `uv.lock`
- `poetry.lock`
- `yarn.lock`
- `pnpm-lock.yaml`
- `CHANGELOG.md` (any directory)
- `.release-please-manifest.json`
- `.github/workflows/*.yml`
- `.github/workflows/*.yaml`

**Rationale:** Across all three repos, the top-scoring pairs are dominated by release-chore and CI-config files that ride together on every version bump or workflow tweak. They have high `shared_commits` (120+ on SymForge) and high `weighted_score`, but they carry no code-navigation signal. Excluding them as drivers — while keeping them available as partners — filters the release-chore pattern without losing any real coupling evidence.

Workspace override: the denylist MUST be configurable via workspace settings (pattern TBD in Tentacle 3 — either settings.toml key or env-var-separated pattern list). Ships with the defaults above.

### 4. Symbol-level pairs gated by file-level **[CALIBRATED]**

A symbol-level pair `(symbol_a_in_file_a, symbol_b_in_file_b)` MUST NOT contribute to rerank unless the corresponding file-level pair `(file_a, file_b)` also passes the file-level floor (rule 1) and the chore denylist (rule 3). When `file_a == file_b` (intra-file symbol pair), the file-level gate is vacuous — intra-file symbol pairs SHOULD still contribute, because they capture cohesion within a file that file-level cannot.

**Rationale:** Symbol-level signal is significantly noisier (tokio's 92% → 97% `shared=1` jump, magika's 0.3% → 32.8% jump in >100-partner share). Gating behind the coarser file-level signal preserves symbol-level precision where the file-level signal is already strong and suppresses it where it would be pure noise.

### 5. Anchor-confidence gate for the rerank itself **[PROVISIONAL — validate in Tentacle 3]**

Proposed rule (NOT evidence-backed by Tentacle 2): rerank should only apply when the query's top path-match result has sufficient confidence to be treated as an anchor.

- If the query's top result's `PathMatchSignal::score()` is below the `Basename` tier (i.e., `Loose` or `Prefix` only), rerank should be a no-op and the baseline path-match ordering should be returned unchanged.

**Why provisional:** the Tentacle 2 harness measures coupling-store distributions only. It never inspects queries, path-match scores, or rerank outcomes. The `Basename`-tier threshold is a heuristic, not a calibrated value.

**Tentacle 3 validation requirement:** before this rule ships as default behaviour, Tentacle 3 MUST add query-level calibration that measures rerank precision across a query set at varying path-match confidence tiers, or document an equivalent empirical check. Outcomes to choose among:
1. Confirm `Basename`-tier threshold and promote to **[CALIBRATED]** in an amended ADR.
2. Find a different threshold (e.g., `StrongPath` only, or a continuous confidence score) and amend the ADR with the measured value.
3. Show the gate is unnecessary (rerank doesn't degrade weak-query responses) and drop it.

Until Tentacle 3 produces that evidence, implement the gate as the conservative default but mark the threshold as subject to change via a named constant in code so the validation can swap it without reshipping the ranker.

**Rationale for the direction (not the threshold):** a weak top result is a poor pivot for "what rides with this?" The coupling graph is anchored off the top result; an unreliable anchor produces unreliable partners. The check is cheap. If Tentacle 3 validation shows the risk is real, this gate stays; if not, it goes.

### 6. Score threshold is RELATIVE, not absolute **[CALIBRATED]**

Tentacle 3 MUST NOT gate rerank on absolute `weighted_score` thresholds. Defaults that work on SymForge (max score 41.6) fail on tokio (max score 2.3) and magika (max 1.4) — an 18× spread driven by commit cadence and half-life interaction.

Acceptable relative gates (Tentacle 3 picks one):
- Top-K ordering within the current anchor's partner set (preferred — simplest, matches rule 2).
- Percentile of the store's current distribution (e.g., include only partners above this anchor's own `p75`).

**Rationale:** The cross-repo score-scale spread is structural, not tunable.

### 7. Default constants preserved from Tentacle 1 **[CALIBRATED — but only for "run-to-completion under sensible output", not parameter-sweep tuned]**

These defaults in `WalkerConfig` were used as inputs to the Tentacle 2 harness and produced the validated output. They were NOT parameter-swept (no A/B across different values). They MUST NOT be changed without rerunning the harness across the full corpus and updating this ADR:

- `max_commits = 500` — bounds the cold-build cost
- `window_days = 1095` (~3 years) — outer cutoff; practical floor is `half_life_days`
- `half_life_days = 30` — active-repo-tuned; justified by SymForge's 90-day compressed history
- `max_files_per_commit = 200` — effectively a no-op on the corpus (0–1 skipped per repo)

**Half-life tradeoff explicitly recorded:** 30 days produces near-zero scores for commits > 6 months old. This is correct for agent-facing recent-coupling signal but under-counts long-term coupling on slow repos. If Tentacle 3's usage data shows slow-repo customers needing longer tails, revisit the half-life calibration separately; do not adjust by gut feel.

## Failure-mode guidance

Tentacle 3's rerank path MUST handle these cases without panic and without degrading baseline path-match behaviour:

1. **Empty coupling store** (fresh workspace, never cold-built, or newly-purged no-HEAD repo) — rerank is a no-op; path-match ordering returned unchanged.
2. **Coupling store populated but current query has no top result above `Basename`** — rerank is a no-op (rule 5).
3. **Top-anchor is itself in the chore denylist** — rerank is a no-op for this query; path-match ordering returned.
4. **Anchor has no partners passing the floor** — no partners to promote; path-match ordering returned.
5. **Anchor's partners all fall below the chore denylist as partners** — still return them (denylist is anchor-side only). This preserves the "partner visibility" case where, e.g., `Cargo.toml` is the user's query and `Cargo.lock` is a legitimate partner.
6. **Coupling SQLite file locked or missing** — rerank MUST be treated as a no-op (degrade gracefully); the live index remains functional.

Tentacle 3 MUST add test coverage for at least 1, 2, 4, and 6.

## Consequences

**Positive:**
- Deterministic rerank behaviour backed by empirical noise analysis.
- Absolute-threshold trap avoided — the contract is portable across repos with different commit cadences.
- Release-chore noise filtered by design, not by weight-tuning.

**Negative / explicit tradeoffs:**
- Chore-denylist is an opinionated choice. Workspaces where `Cargo.toml` ↔ `Cargo.lock` is meaningful will need to customise. Default ships with the denylist.
- The `shared_commits >= 2` floor drops legitimate first-coincidence evidence. Acceptable: a first coincidence carries too little signal to promote a file over exact path-match.
- Symbol-level-gated-by-file-level means a genuinely valid intra-file symbol pair (e.g., `fn a` and `fn b` in a solo.rs commit) still contributes because the file-gate is vacuous for intra-file pairs — preserves Step 1.3's intra-file pair delivery.

**Revisit triggers:**
- Tentacle 3 acceptance testing shows rerank misbehaves on a real workflow not covered by the corpus → rerun calibration with that workflow's repo added.
- Half-life tuning comes up as a real concern once Tentacle 3 is live.
- Chore denylist produces complaints that exceed bug-fix volume → move from static default to telemetry-driven.

## References

- Calibration harness: `tests/coupling_calibration.rs`
- Research doc: `docs/research/coupling-calibration-2026-04-18.md`
- Raw output: `docs/research/fixtures/coupling-calibration-2026-04-18.raw.md`
- Tentacle 1 execution plan: `docs/plans/cochange-coupling-execution.md`
- Parent idea note: `docs/ideas/cochange-rerank.md`
- Companion ADR (rank-signal extension architecture): `docs/decisions/0012-edit-and-ranker-hook-architecture.md`
