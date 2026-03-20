# Roadmap: SymForge RTK Adoption & Quality Fixes

## Overview

This milestone fixes 5 remaining issues from the v1.6.0 external review: search result completeness, token budget enforcement, C# symbol disambiguation, hook diagnostic context, and Codex ceiling documentation. Three phases deliver tool output correctness, symbol resolution fixes, and observability improvements -- each independently verifiable.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Tool Output Correctness** - Fix search_text ranked filtering and get_symbol_context budget enforcement
- [ ] **Phase 2: Symbol Disambiguation** - Auto-resolve C# class/constructor ambiguity by kind priority
- [ ] **Phase 3: Diagnostics & Documentation** - Hook failure context, verbose mode, and Codex ceiling docs

## Phase Details

### Phase 1: Tool Output Correctness
**Goal**: search_text and get_symbol_context produce complete, budget-respecting output
**Depends on**: Nothing (first phase)
**Requirements**: SRCH-01, SRCH-02, BUDG-01, BUDG-02, BUDG-03
**Success Criteria** (what must be TRUE):
  1. `search_text` with `ranked=true` returns every file that `ranked=false` returns (order differs, count does not)
  2. When ranked results are truncated, a footer shows how many files matched vs how many are shown
  3. `get_symbol_context` in default and trace modes stops emitting output at the `max_tokens` boundary (nearest line)
  4. Truncated context output ends with a `[truncated -- exceeded {N} token budget]` footer
**Plans**: TBD

Plans:
- [ ] 01-01: TBD
- [ ] 01-02: TBD

### Phase 2: Symbol Disambiguation
**Goal**: C# symbol lookups resolve without false ambiguity errors
**Depends on**: Nothing (independent, but ordered after Phase 1 to avoid query.rs merge conflicts)
**Requirements**: SYMB-01, SYMB-02, SYMB-03
**Success Criteria** (what must be TRUE):
  1. Looking up a C# class by name returns the class definition even when a same-named constructor exists
  2. `resolve_symbol_selector` applies kind-tier priority (class > constructor > method > other) to auto-disambiguate cross-kind matches
  3. Two symbols at the same priority tier still produce an Ambiguous error with candidates listed
**Plans**: TBD

Plans:
- [ ] 02-01: TBD

### Phase 3: Diagnostics & Documentation
**Goal**: Users can diagnose hook failures and understand SymForge's limits in Codex
**Depends on**: Nothing (independent, but ordered last as lowest severity)
**Requirements**: HOOK-01, HOOK-02, HOOK-03, DOCS-01, DOCS-02
**Success Criteria** (what must be TRUE):
  1. A `NoSidecar` adoption log entry distinguishes "port missing" from "port stale" and includes the project root path
  2. Setting `SYMFORGE_HOOK_VERBOSE=1` produces stderr diagnostic output during hook execution
  3. The first `NoSidecar` event in a session writes a one-time hint explaining how to start the sidecar
  4. A `docs/codex-ceiling.md` file documents what works (MCP tools), what doesn't (hooks/sidecar), and what requires Codex changes
  5. README or CLAUDE.md links to the Codex ceiling doc for client-specific guidance
**Plans**: TBD

Plans:
- [ ] 03-01: TBD
- [ ] 03-02: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Tool Output Correctness | 0/2 | Not started | - |
| 2. Symbol Disambiguation | 0/1 | Not started | - |
| 3. Diagnostics & Documentation | 0/2 | Not started | - |
