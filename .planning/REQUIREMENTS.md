# Requirements: SymForge RTK Adoption & Quality Fixes

**Defined:** 2026-03-20
**Core Value:** Every tool SymForge advertises must work correctly, and hooks must reliably route source-code workflows through SymForge — trust is the product.

**Origin:** AI reviewer tested v1.6.0. Original 9 items — codebase verification on 2026-03-20 confirmed 7 already fixed in d13e76b. These 8 remain (3 SYMB + 3 HOOK + 2 DOCS).

## v1 Requirements

### Symbol Disambiguation

- [x] **SYMB-01**: `resolve_symbol_selector` auto-disambiguates when candidates span different kind tiers (class > constructor > method > other)
- [x] **SYMB-02**: C# class lookup with name matching both class and constructor returns the class without error
- [x] **SYMB-03**: Genuine ambiguity (two symbols at same priority tier) still returns Ambiguous error

### Hook Diagnostics

- [ ] **HOOK-01**: `NoSidecar` adoption log entry includes whether sidecar.port is missing vs stale, and the project root
- [ ] **HOOK-02**: `SYMFORGE_HOOK_VERBOSE=1` env var enables stderr diagnostic output during hook execution
- [ ] **HOOK-03**: First `NoSidecar` event per session writes a one-time hint to adoption log explaining how to start the sidecar

### Documentation

- [ ] **DOCS-01**: `docs/codex-ceiling.md` documents what works in Codex (MCP tools), what doesn't (hooks, sidecar), and what requires Codex changes
- [ ] **DOCS-02**: README or CLAUDE.md references the Codex ceiling doc for client-specific guidance

## Out of Scope

| Feature | Reason |
|---------|--------|
| validate_file_syntax dispatch | Fixed in d13e76b — verified in daemon.rs:1643 |
| search_text regex noise policy | Fixed in d13e76b — `for_current_code_search` method in search.rs |
| search_text ranked over-filtering | Fixed in d13e76b — ranked default limit boosted to 200 in tools.rs:889 |
| max_tokens budget (default mode) | Fixed in d13e76b — `enforce_token_budget` called in tools.rs:1916 |
| max_tokens budget (trace mode) | Fixed in d13e76b — `enforce_token_budget` called in tools.rs:1839 |
| max_tokens truncation footer | Fixed in d13e76b — `enforce_token_budget` appends `[truncated]` in format.rs:2856 |
| Callee deduplication | Fixed in d13e76b — `occurrence_count` in query.rs, dedup in format.rs |
| Daemon fallback for hooks | Fixed in d13e76b — `DaemonFallback` outcome in hook.rs |
| Git temporal context | Separate milestone, not related to RTK adoption |
| Full Codex hook integration | Client limitation, not SymForge bug |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SYMB-01 | Phase 1 | Complete |
| SYMB-02 | Phase 1 | Complete |
| SYMB-03 | Phase 1 | Complete |
| HOOK-01 | Phase 2 | Pending |
| HOOK-02 | Phase 2 | Pending |
| HOOK-03 | Phase 2 | Pending |
| DOCS-01 | Phase 2 | Pending |
| DOCS-02 | Phase 2 | Pending |

**Coverage:**
- v1 requirements: 8 total
- Mapped to phases: 8
- Unmapped: 0

---
*Requirements defined: 2026-03-20*
*Last updated: 2026-03-20 after codebase verification — 5 items moved to Out of Scope (already fixed)*
