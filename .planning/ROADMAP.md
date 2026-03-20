# Roadmap: SymForge RTK Adoption & Quality Fixes

## Overview

This milestone addresses 8 remaining items from the v1.6.0 external review after codebase verification confirmed 7 of the original 9 items were already fixed in commit d13e76b. Two phases: C# symbol disambiguation (code fix) and hook diagnostics + Codex documentation (observability).

## Phases

**Phase Numbering:**
- Integer phases (1, 2): Planned milestone work
- Decimal phases (1.1, 1.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Symbol Disambiguation** - Auto-resolve C# class/constructor ambiguity by kind priority
- [ ] **Phase 2: Diagnostics & Documentation** - Hook failure context, verbose mode, and Codex ceiling docs

## Phase Details

### Phase 1: Symbol Disambiguation
**Goal**: C# symbol lookups resolve without false ambiguity errors
**Depends on**: Nothing (first phase)
**Requirements**: SYMB-01, SYMB-02, SYMB-03
**Success Criteria** (what must be TRUE):
  1. Looking up a C# class by name returns the class definition even when a same-named constructor exists
  2. `resolve_symbol_selector` applies kind-tier priority (class > constructor > method > other) to auto-disambiguate cross-kind matches
  3. Two symbols at the same priority tier still produce an Ambiguous error with candidates listed
**Plans**: 1 plan

Plans:
- [ ] 01-01-PLAN.md — Add kind-tier disambiguation to resolve_symbol_selector with comprehensive tests

### Phase 2: Diagnostics & Documentation
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
- [ ] 02-01: TBD
- [ ] 02-02: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Symbol Disambiguation | 0/1 | Not started | - |
| 2. Diagnostics & Documentation | 0/2 | Not started | - |
