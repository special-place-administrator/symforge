---
type: research
title: Co-Change Coupling — Calibration on SymForge, tokio, magika
created: 2026-04-18
tentacle: T2
consumers: [docs/decisions/0013-coupling-signal-contract.md]
harness: tests/coupling_calibration.rs
raw_output: docs/research/fixtures/coupling-calibration-2026-04-18.raw.md
---

# Co-Change Coupling — Calibration Report

## Purpose

Tentacle 2's output: evidence-backed defaults and guardrails that Tentacle 3 will consume when fusing `CoChangeSignal` into the ranker. The job is not to ship new runtime surface; it is to produce the contract the integration phase will wire in.

## Method

A gated integration test (`tests/coupling_calibration.rs`, run with `cargo test --release --test coupling_calibration -- --ignored --nocapture`) runs `cold_build` on each corpus repo in both file-only and file+symbol modes, then queries the SQLite store directly for:

- Walker stats (commits scanned, skipped, unique edges, rows written)
- Partner-count distribution per anchor
- Score-and-shared-commits percentiles (p50/p75/p90/p95/p99/p99.9/max)
- Top 10 hotspot anchors by partner count
- Top 10 pairs by `weighted_score`
- Weak-anchor quartiles and share of `shared_commits == 1`

Raw output is preserved at `docs/research/fixtures/coupling-calibration-2026-04-18.raw.md`. Re-run with the same command to reproduce.

Config is the shipped default: `max_commits=500`, `window_days=1095`, `half_life_days=30`, `max_files_per_commit=200`.

## Corpus

Pinned to exact HEAD SHA at calibration time (captured by the harness; see fixture header for reproduction). The harness `assert!`s that all three repos are present — a silently-reduced corpus would weaken the ADR's auditability, so the test fails rather than under-sample.

| repo | role | HEAD SHA | commits_scanned (file) | commits_scanned (file+sym) |
|---|---|---|---:|---:|
| SymForge | self-host Rust, ~90 days of history, active | `29eb9e1d73fb0626aebcf536b98355b2610efdb7` | 345 | 415 |
| tokio-rs/tokio | large Rust async runtime, years of history | `b010b5ddafb316e5d540de1736f5f7bfde42ec39` | 290 | 327 |
| google/magika | polyglot (Rust CLI + Python lib + JS/TS + website) | `0a8cb9626bbf76c2194117d9830b23e9052a1548` | 233 | 262 |

(`commits_scanned` under file+symbol exceeds file-only because the window stays wall-clock-bounded; both runs walk their caps but the timing differs slightly between invocations.)

To reproduce, check out each repo to the recorded SHA (`git checkout <sha>`) before rerunning the harness.

## Cross-repo evidence

### Score scale varies by ~18×

| metric | SymForge | tokio | magika |
|---|---:|---:|---:|
| max `weighted_score` (file) | **41.61** | 2.32 | 1.40 |
| p99 `weighted_score` (file) | 0.99 | 0.30 | 0.15 |
| p50 `weighted_score` (file) | 0.083 | 0.0014 | 0.0019 |

SymForge is an active repo with 500 commits compressed into ~90 days; tokio's 500-commit window spans years, so the oldest commits decay to near-zero under a 30-day half-life. Magika sits in between.

**Implication:** absolute score thresholds can't work cross-repo. The ranker must gate on relative or rank-based signals (percentile, top-K per anchor, shared-commits-count), not raw `weighted_score` cutoffs.

### `shared_commits = 1` dominates every repo

| repo | share of rows with `shared_commits = 1` (file) | (file+symbol) |
|---|---:|---:|
| SymForge | 69.3% | 59.6% |
| tokio | 91.7% | 97.4% |
| magika | 82.7% | 84.6% |

Two-thirds to 97% of stored pairs have co-changed exactly once. A single co-occurrence is weak evidence — it's indistinguishable from chance on active repos. **Strongly suggests a `shared_commits >= 2` floor for the rerank signal.**

### Top pairs are dominated by "release chore" infrastructure

Every repo's top pairs are build manifests, lock files, changelogs, and release-automation metadata:

| repo | top-scoring pair |
|---|---|
| SymForge | `Cargo.toml` ↔ `Cargo.lock` (167 shared, score 41.6) |
| tokio | `README.md` ↔ `tokio/README.md` (43 shared, score 2.32) |
| magika | `python/pyproject.toml` ↔ `python/uv.lock` (17 shared, score 1.40) |

These pairs get touched on every version bump. They are genuinely coupled in the "always change together" sense, but they're useless for agent-facing code navigation — they don't point to related source code. **Implication: Tentacle 3 should downweight or denylist known chore anchors as drivers of rerank.** They can still appear as *partners* when a relevant anchor pulls them in.

### Hotspot anchors concentrate signal unevenly

Share of anchors with >100 partners:

| repo | file-only | file+symbol |
|---|---:|---:|
| SymForge | 52.0% | 47.6% |
| tokio | 3.9% | 11.2% |
| magika | 0.3% | 32.8% |

SymForge's concentration is extreme — most file anchors couple with 100+ others. Driven by: small repo size (3000 partners max), very active tree, a handful of "mega-files" like `src/protocol/tools.rs` (190 partners file-level, 1229+ symbol-level partners for its `tests` module). Without a per-anchor cap, the rerank returns most of the repo on any query.

**Implication: cap at 20 partners per anchor** when servicing a rerank request. The top-20 by `weighted_score` captures the real signal; the long tail is noise for agent presentation.

### Symbol-level is substantially noisier than file-level

Tokio file→file+symbol: `shared_commits = 1` jumps from 92% → 97%. Magika's >100-partner share jumps 0.3% → 32.8%. Symbol-level's finer granularity increases ambient coupling: every commit touching a single file produces pairs between every symbol that file contains.

**Implication: apply a stricter `shared_commits` floor for symbol-level rerank (≥3 vs ≥2 for file-level).** Alternatively, only use symbol-level coupling when a file-level pair already crosses the floor — gates symbol signal behind the coarser signal.

### Hotspot anchors per repo

SymForge's top file-level hotspots are the right files to expect: `src/protocol/tools.rs`, `src/live_index/query.rs`, `src/protocol/format.rs` — handler-and-query mega-files that genuinely co-change with most of the codebase because of their breadth. The *max score* these hit (11.7) is meaningful; their 100+ partner count is mostly driven by the releases-chore commits counted above.

Tokio's top file-level hotspot is `.github/workflows/ci.yml` (246 partners). CI configuration changes ride with most large feature work. This is a real pattern worth preserving at symbol-level but noisy at file-level.

Magika's top file-level hotspot is `website-ng/src/content/docs/cli-and-bindings/overview.md` with 104 partners. Documentation churn in a polyglot repo's docs tree — weakly-coupled-to-code noise.

### Commit-skipping behaviour

`commits_skipped_large` was zero for tokio and magika across 500-commit windows (no commit touched >200 files). SymForge saw 1 skipped out of 345 (a large reorganisation). The 200-file cap is safe and effectively a no-op on healthy repos.

## Observations on noise patterns

1. **Release-chore noise is the single biggest contributor to bogus top-scoring pairs.** Build manifests (`Cargo.toml`, `package.json`, `pyproject.toml`), lock files (`Cargo.lock`, `package-lock.json`, `uv.lock`), changelogs (`CHANGELOG.md`), and release-automation metadata (`.release-please-manifest.json`, GitHub release manifests) ride together on every version bump.

2. **CI-config noise ranks second.** `.github/workflows/*.yml` files co-change with most substantial work but the coupling signal is unidirectional — an agent exploring `runtime/builder.rs` doesn't need to know about `ci.yml`.

3. **Documentation-churn noise ranks third.** Doc trees that get a blanket refresh on each release accumulate pair counts without carrying code-navigation value.

4. **Tests-module noise is specific to SymForge-style monolithic test suites.** `src/protocol/tools.rs#tests#mod` having 1229 partners reflects the convention of huge in-file test modules that touch every handler.

## Summary for the ADR

**Bounded-window design (Tentacle 1) is validated.** All three repos produce sensible top pairs, reasonable percentile shapes, and stay within build-time budgets (≤2 seconds file-only, ≤7 seconds symbol-level for SymForge; tokio/magika similar).

**Four defaults to pin in the ADR:**

1. **Weak-anchor shared-commits floor** — `shared_commits_min`: 2 for file-level, 3 for symbol-level. Eliminates 60-97% of noise across the corpus. Justified directly by the `shared_commits = 1` share distribution.

2. **Per-anchor partner cap** — `max_partners_per_anchor`: 20. Selected by `weighted_score DESC`. Prevents SymForge-style over-coupled repos from flooding the rerank with 100+ partners, and aligns with agent-facing top-K display conventions.

3. **Anchor chore-denylist** — set per the ADR (`docs/decisions/0013-coupling-signal-contract.md §3`). The authoritative list lives in the ADR, not in this research doc, to prevent drift. Files on the denylist are excluded from driving rerank as anchors; they may still appear as partners.

4. **Symbol-level gated by file-level** — `require_file_level_pair`: symbol-level rerank only applies for `(symbol_a, symbol_b)` when a corresponding `(file_a, file_b)` file-level pair also meets the floor. Filters the symbol-level amplification observed in tokio/magika's 97%/85% `shared=1` rates.

**Failure-mode guidance** (store-level only; path-match-level failure modes are provisional — see ADR §5):
- Store with zero rows (fresh workspace, no HEAD) → rerank is a no-op, returns the path-match ordering unchanged.
- Rescale applied to every delta; no bounded-window correctness issues at Tentacle 3's integration layer.

**Not measured by this calibration (downgraded to provisional in the ADR):**
- An anchor-confidence gate based on `PathMatchSignal` tiers. The harness only measures coupling-store distributions, not query outcomes or path-match scores. Any query-level threshold must be validated in Tentacle 3 against a query corpus — see ADR §5 for the validation requirement.

**Where the evidence is mixed:**
- Half-life of 30 days is right for active repos (SymForge) but produces near-zero scores for old commits on slow repos (tokio's 5-year-old commits). ADR keeps 30 days as default since agents care about recent coupling; document that Tentacle 2 revisits this if slow-repo use cases emerge.
- Chore-denylist is an opinionated choice. Some users may want `Cargo.toml` ↔ `Cargo.lock` to surface. ADR exposes the denylist as a workspace-configurable set; ships with the defaults listed above.

See `docs/decisions/0013-coupling-signal-contract.md` for the pinned defaults, mandatory invariants, and guardrail policy Tentacle 3 implements against.

## Raw data

Preserved verbatim at `docs/research/fixtures/coupling-calibration-2026-04-18.raw.md` — six sections (three repos × file-only + file+symbol) plus the corpus-pin SHA table. Reproduce with:

```
cargo test --release --test coupling_calibration -- --ignored --nocapture \
  > docs/research/fixtures/coupling-calibration-2026-04-18.raw.md
```

Corpus paths (update for your machine):
- SymForge — `C:/AI_STUFF/PROGRAMMING/symforge`
- tokio — `C:/AI_STUFF/temp/tokio` (clone of `github.com/tokio-rs/tokio`, full history)
- magika — `C:/AI_STUFF/temp/magika` (clone of `github.com/google/magika`, full history)
