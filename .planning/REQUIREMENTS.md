# Requirements: SymForge RTK Adoption & Quality Fixes

**Defined:** 2026-03-20
**Core Value:** Every tool SymForge advertises must work correctly, and hooks must reliably route source-code workflows through SymForge — trust is the product.

**Origin:** AI reviewer tested v1.6.0. 4 of 9 original items fixed in commit d13e76b. These 5 remain.

## v1 Requirements

### Search Quality

- [ ] **SRCH-01**: `search_text` with `ranked=true` returns the same file set as `ranked=false` (only order changes, not count)
- [ ] **SRCH-02**: Ranked mode diagnostic footer shows `{N} files matched, showing top {M} ranked by importance` when results are truncated

### Budget Enforcement

- [ ] **BUDG-01**: `get_symbol_context` in default mode respects `max_tokens` parameter (truncates output at nearest line boundary)
- [ ] **BUDG-02**: `get_symbol_context` in trace mode respects `max_tokens` parameter
- [ ] **BUDG-03**: Truncated output appends `[truncated — exceeded {N} token budget]` footer

### Symbol Disambiguation

- [ ] **SYMB-01**: `resolve_symbol_selector` auto-disambiguates when candidates span different kind tiers (class > constructor > method > other)
- [ ] **SYMB-02**: C# class lookup with name matching both class and constructor returns the class without error
- [ ] **SYMB-03**: Genuine ambiguity (two symbols at same priority tier) still returns Ambiguous error

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
| validate_file_syntax dispatch | Already fixed in d13e76b — verified in daemon.rs:1643 |
| search_text regex noise policy | Already fixed in d13e76b — `for_current_code_search` method exists |
| Callee deduplication | Already fixed in d13e76b — `occurrence_count` field and dedup logic in query.rs |
| Daemon fallback for hooks | Already fixed in d13e76b — `DaemonFallback` outcome and `DAEMON_FALLBACK_DEADLINE` in hook.rs |
| Git temporal context | Separate milestone, not related to RTK adoption |
| Full Codex hook integration | Client limitation, not SymForge bug |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SRCH-01 | TBD | Pending |
| SRCH-02 | TBD | Pending |
| BUDG-01 | TBD | Pending |
| BUDG-02 | TBD | Pending |
| BUDG-03 | TBD | Pending |
| SYMB-01 | TBD | Pending |
| SYMB-02 | TBD | Pending |
| SYMB-03 | TBD | Pending |
| HOOK-01 | TBD | Pending |
| HOOK-02 | TBD | Pending |
| HOOK-03 | TBD | Pending |
| DOCS-01 | TBD | Pending |
| DOCS-02 | TBD | Pending |

**Coverage:**
- v1 requirements: 13 total
- Mapped to phases: 0
- Unmapped: 13

---
*Requirements defined: 2026-03-20*
*Last updated: 2026-03-20 after initial definition*
