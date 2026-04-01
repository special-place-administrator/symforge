# Session Handoff — 2026-03-17 — Sprint 0 + Rename Cleanup

## Project
**SymForge** — Rust MCP server for AI code intelligence.
**Repo:** `C:/AI_STUFF/PROGRAMMING/symforge` (renamed from the pre-SymForge repository name)
**GitHub:** `github.com/special-place-administrator/symforge`

## What Was Completed This Session

### Rename finalization
- GitHub repo renamed to `symforge` ✓
- Local folder renamed to `symforge` ✓
- All 83 source files renamed internally (commit `1e34a79` → rebased to `6366cd0`) ✓
- Cargo.lock conflict resolved (missing `[[package]]` header — fixed in `67d9327`) ✓
- npm/package.json, Cargo.toml, CHANGELOG.md all updated to `symforge` v0.33.0 ✓
- symforge npm package is now LIVE on npm registry ✓
- legacy npm package to be deprecated/uninstalled by user after this session

### Sprint 0 — Index Freshness Guarantee (COMMITTED, not yet pushed)
Commit message: `feat: Sprint 0 — index freshness guarantee`

**Changes made:**
- `src/live_index/store.rs`: Added `mtime_secs: u64` to `IndexedFile` struct; `with_mtime()` builder; populate mtime in `load()` and `build_reload_data()`
- `src/watcher/mod.rs`: Capture mtime in `maybe_reindex()`; added `freshen_file_if_stale()`; added `reconcile_stale_files()`; watcher overflow → trigger reconciliation; periodic reconciliation timer (30s, configurable via `SYMFORGE_RECONCILE_INTERVAL`); extended `WatcherInfo` with overflow_count, last_overflow_at, stale_files_found, last_reconcile_at
- `src/protocol/edit.rs`: Capture mtime in `reindex_after_write()`
- `src/live_index/persist.rs`: Added `mtime_secs: 0` to snapshot restore literal

**Cargo check: PASSES**

### Sprint 0 — STILL TODO (next session)
- **0.4**: Update `HealthStats` in `query.rs` — add `stale_warnings: Vec<String>` populated from WatcherInfo overflow/reconcile stats
- **Wire up `freshen_file_if_stale`** in `SymForgeServer` (protocol/mod.rs) and call from targeted tool handlers: `get_file_content`, `get_symbol`, `get_symbol_context`
  - `SymForgeServer` has `index: SharedIndex` and `repo_root: Arc<RwLock<Option<PathBuf>>>`
  - Add `fn freshen_path(&self, relative_path: &str)` method
  - Call it before file-specific queries in tools.rs

## Pending Actions (User)
1. Uninstall the legacy npm package after exiting
2. Deprecate the legacy npm package in favor of `symforge`
3. Push Sprint 0 commit: `git push origin main` (from `C:/AI_STUFF/PROGRAMMING/symforge`)
4. Verify crates.io email → re-check if `cargo publish` now works in CI
5. Check Actions tab for release workflow green run

## Next Sprint After 0
Sprint 1 quick wins (from `plans/community-feedback-improvements.md`):
- 1.1: Better `get_symbol` error without path
- 1.2: Ambiguity shows signatures not just line numbers
- 1.3: `diff_symbols` clearer "no changes" output
- 1.4: `dry_run` on insert/delete_symbol
- 1.5: `search_files` space-separated fuzzy matching
- 1.6: Document `find_dependents` Mermaid/Graphviz output

## Key File Paths
- Plan: `plans/community-feedback-improvements.md`
- Watcher: `src/watcher/mod.rs` — `freshen_file_if_stale`, `reconcile_stale_files`
- Server: `src/protocol/mod.rs` — `SymForgeServer` struct (has index + repo_root)
- Tools: `src/protocol/tools.rs` — targeted tools need `freshen_path()` calls
- Health: `src/live_index/query.rs` — `HealthStats` struct at L596

## CLAUDE.md Note
CLAUDE.md now has BOTH a legacy MCP section AND a "SymForge MCP" section (the init hook added the SymForge section). The legacy section should be removed because it duplicates the SymForge one. File: `~/.claude/CLAUDE.md`
