# Wave 2 Close-Out Evidence (2026-05-16)

Audit basis: local `main` at `416d1da4dd5d07185808b72663391c9386a71783` before this close-out patch. Local `main` is ahead of `origin/main` by `c90a757` and `416d1da`; release, push, and release-please state mutation are intentionally out of scope for this close-out.

## Objective

Close out Wave 2 (CoChange Ranker Fusion) as release-ready for v7.9.0 while preserving the explicit constraint not to push, publish a release, or modify release-please state.

## Landed units

| Unit | Scope | Commit(s) | Evidence | Status |
| --- | --- | --- | --- | --- |
| 2.1 | Add co-change inputs to `RankCtx` | `86327db` | `src/live_index/rank_signals.rs`, `src/live_index/query.rs`; prior gate recorded `cargo check`, `cargo test --lib rank_signals -- --test-threads=1`, `cargo test --all-targets -- --test-threads=1`, `cargo build --release` | PASS |
| 2.2 | Expose `LiveIndex::coupling_store()` | `51810d7` | `src/live_index/store.rs`, `src/live_index/coupling/lifecycle.rs`, call-site constructor updates; prior gate recorded `cargo check`, `cargo test --lib coupling -- --test-threads=1`, `cargo test --all-targets -- --test-threads=1`, `cargo build --release` | PASS |
| 2.3 | Implement `CoChangeSignal::score()` contract handling | `c90a757` | `src/live_index/rank_signals.rs`; `tests/cochange_signal_rules.rs` covers weak shared-commit floor, chore anchor denial, fail-closed missing/invalid evidence, Rule 5 weak-anchor no-op, Rule 6 no absolute threshold, and partner cap ordering | PASS |
| 2.4 | Wire `rank_by="path+cochange"` + `anchor_path` into `search_files` | `c90a757` | `src/live_index/query.rs`, `src/protocol/tools.rs`, `tests/cochange_fusion.rs`; tests cover partner promotion, below-floor fallback, weak-anchor fallback, tiny relative score, unmatched-neighbor byte-identical fallback, and no-context default ordering | PASS |
| 2.5 | Deprecate `changed_with=` compatibility path | `416d1da` | `src/protocol/tools.rs`; deprecation warning points callers to `rank_by="path+cochange"` with `anchor_path=<path>` while preserving existing compatibility behavior | PASS |
| 2.6 | Close-out docs, evidence, vault status, clippy gate fix, and stale source-comment cleanup | this close-out commit | This file, `docs/plans/2026-05-15-symforge-post-h-roadmap.md`, `src/live_index/coupling/lifecycle.rs`, `src/live_index/rank_signals.rs`, and `wiki/concepts/SymForge Co-Change Signal Fusion.md` via Obsidian MCP | PASS |

## Rule 5 calibration audit

ADR 0013 Rule 5 remains **PROVISIONAL**.

Evidence checked:

- `docs/decisions/0013-coupling-signal-contract.md` still tags Rule 5 as `PROVISIONAL - validate in Tentacle 3`.
- No `docs/notes/2026-05-16-rule5-calibration.md` or equivalent query-level calibration note exists in the repo.
- `tests/cochange_signal_rules.rs::rule5_missing_or_weak_anchor_confidence_does_not_drive_score` and `tests/cochange_fusion.rs::path_cochange_falls_back_when_anchor_confidence_is_weak` prove fail-closed behavior for the conservative `Basename` floor, but they do not measure query-level precision across SymForge/tokio/magika or another query corpus.

Close-out decision: do not update ADR 0013 to mark Rule 5 CALIBRATED. The v7.9.0-ready behavior keeps the conservative gate in code and documents the missing calibration as a follow-up.

Follow-up acceptance criteria before promotion:

- Build a query-level calibration harness or equivalent measured corpus for `rank_by="path+cochange"`.
- Compare at least `StrongPath`, `Basename`, `Prefix`, and `LoosePath` anchor thresholds.
- Record precision/regression outcomes and examples in `docs/notes/YYYY-MM-DD-rule5-calibration.md`.
- Amend ADR 0013 only after the measured result supports the selected threshold, changes it, or removes the gate.

## Release readiness

- `Cargo.toml` currently declares `7.8.2`.
- `.github/.release-please-manifest.json` currently declares `7.8.2`.
- `.github/release-please-config.json` uses release type `rust`, includes `CHANGELOG.md`, and updates `npm/package.json` as an extra file.
- Local `origin/main..HEAD` contains `c90a757 feat(search-files): fuse cochange ranking`, which is the conventional-commit signal expected to produce a minor v7.9.0 release-please PR after push.
- No push, release, tag, changelog edit, manifest edit, or release-please PR creation was performed in this close-out.

## Verification

Final local gates after the close-out docs, vault update, clippy fix, and stale source-comment cleanup:

| Gate | Result | Notes |
| --- | --- | --- |
| `cargo check` | PASS | Re-run after the clippy source fix. |
| `cargo test --all-targets -- --test-threads=1` | PASS | Re-run after the clippy source fix. |
| `cargo test --all-targets` | PASS | Re-run after the clippy source fix. |
| `cargo clippy -- -D warnings` | PASS | First run failed on `clippy::needless_borrow` in `src/live_index/coupling/lifecycle.rs`; close-out fixed the two stale `&store` call sites and the rerun passed. |
| `cargo build --release` | PASS | Release profile build completed after the final source fix. |

## Open follow-ups

- Rule 5 query-level calibration remains open; keep ADR 0013 Rule 5 PROVISIONAL until measured evidence exists.
- Public v7.9.0 release remains open because push/release/release-please state mutation was explicitly out of scope for this task.
