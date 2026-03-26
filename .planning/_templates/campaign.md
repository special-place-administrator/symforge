# Campaign: {Campaign Name}

Status: active
Started: {ISO timestamp, e.g., 2026-03-20T14:30:00Z}
Direction: {The original user direction that created this campaign}

## Claimed Scope
<!-- Directories this campaign will modify. Used by coordination system to prevent collisions. -->
- src/api/auth/
- src/middleware/

## Phases
<!-- 3-8 phases. Mark status as: [pending], [in-progress], [complete], [skipped], [failed] -->
1. [pending] Research: Audit existing auth implementation
2. [pending] Plan: Design token refresh architecture
3. [pending] Build: Implement JWT middleware
4. [pending] Build: Add refresh token endpoint
5. [pending] Wire: Connect auth to existing routes
6. [pending] Verify: Run full test suite, manual verification

## Feature Ledger
<!-- Track what was actually built. Updated after each phase. -->
| Feature | Status | Phase | Notes |
|---------|--------|-------|-------|

## Decision Log
<!-- Timestamped decisions with reasoning. Prevents re-debating in future sessions. -->
<!-- Example:
- 2026-03-20: Chose jose over jsonwebtoken for JWT handling
  Reason: ESM native, better TypeScript types, actively maintained
-->

## Active Context
<!-- Where the campaign is RIGHT NOW. Updated on every session. -->
Campaign just created. Starting with Phase 1 (Research).

## Continuation State
<!-- Machine-readable state for the next Archon invocation. -->
Phase: 1
Sub-step: not started
Files modified: (none yet)
Blocking: none
