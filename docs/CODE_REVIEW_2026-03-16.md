# Code Review — 2026-03-16 (General Codebase)

Full-codebase review, not scoped to a PR or sprint. Findings ranked by user after initial review.

## Backlog Table

| ID | Finding | Priority | Status | Location | Action |
|----|---------|----------|--------|----------|--------|
| CR1 | `find_qualified_usages` panics on non-ASCII UTF-8 source (byte-index slicing into `&str`) | **P0** | NEW | `src/protocol/edit.rs:1523-1730` | Add non-ASCII test, fix byte-walking to respect char boundaries |
| CR2 | `LiveIndex::load` circuit-breaker nondeterminism — `par_iter` ordering affects which files survive tripping | **P0** | NEW | `src/live_index/store.rs:594-827` | Collect all results first, evaluate breaker holistically, then decide |
| CR3 | Error type inconsistency — edit operations return `Result<_, String>` instead of `SymForgeError` | **P1** | NEW | `src/protocol/edit.rs` (multiple fns) | Add `EditError` variant or use `SymForgeError` consistently |
| CR4 | `open_project_session` activation-race fragility — double write-lock window between insert and activate | **P1** | VERIFY vs C6 fix | `src/daemon.rs:250-298` | Audit post-C6 branch; may already be mitigated by ActivationState enum |
| CR5 | `line_byte_offset` in `find_qualified_usages` assumes LF-only — works for CRLF but drifts on bare `\r` | **P1** | NEW | `src/protocol/edit.rs:1536` | Add comment or defensive test for mixed-ending edge case |
| CR6 | Same-path concurrent `atomic_write_file` — no test coverage | **P1** | NEW | `src/protocol/edit.rs:134-150` | Add test: two threads writing same path concurrently |
| CR7 | Duplicate `EnvVarGuard`/`CwdGuard` test helpers across modules | **P2** | NEW | `src/daemon.rs`, `src/protocol/tools.rs` tests | Extract to shared `#[cfg(test)]` module |
| CR8 | `find_qualified_usages` — duplicated `prec2`/`fol2` match+push logic (5 copies) | **P2** | NEW | `src/protocol/edit.rs:1523-1730` | Extract local closure |
| CR9 | `is_binary_content` magic number 0.30 threshold | **P2** | NEW | `src/discovery/mod.rs:350` | Name constant, add rationale comment |
| CR10 | `BurstTracker` debounce constants not configurable | **P2** | NEW | `src/watcher/mod.rs:64-67` | Consider env-var override for deployment tuning |
| CR11 | `execute_tool_call` 200-line match block boilerplate | **P2** | NEW | `src/daemon.rs:1297-1510` | Dispatch table refactor — only after correctness items done |

## Relationship to Sprint 16

Sprint 16 P0 items (C1-C6) are already planned/in-progress. This review's findings are **additive**:

- **CR1, CR2** — genuinely new P0 issues not in the Sprint 16 backlog
- **CR4** — overlaps with C6 (open_project_session); needs verification against post-fix code
- **CR3, CR5-CR11** — new items, no overlap with existing backlog

## Strengths (confirmed)

- `safe_repo_path` path traversal prevention — correct and consistently applied
- `atomic_write_file` via `NamedTempFile` + `persist()` — correct pattern
- Rollback on partial failure in batch edit/rename — well-implemented
- Test coverage ratio is high across all modules
- Circuit breaker pattern protects against pathological repos
- Backward-compat aliases in daemon — clean API evolution pattern
- 3-phase admission pipeline — robust binary/size filtering

## Test Gaps (prioritized)

1. **Non-ASCII source content test for `find_qualified_usages`** — directly validates CR1
2. **Same-path concurrent `atomic_write_file` test** — validates CR6
3. CRLF through full MCP tool pipeline (lower priority — unit coverage exists)
4. Watcher `run_watcher` integration test (complex async — acceptable gap for now)
