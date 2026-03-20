# SymForge — RTK Adoption & Quality Fixes

## What This Is

SymForge is a Rust MCP server providing symbol-aware code navigation and editing tools for AI coding agents. This milestone addresses 6 bugs and 3 RTK (Read-Tool-Kit style) adoption gaps identified through external review of v1.6.0, closing the gap between SymForge's intended workflow ownership and actual runtime behavior.

## Core Value

Every tool that SymForge advertises must work correctly, and hooks must reliably route source-code workflows through SymForge when the sidecar is running — trust is the product.

## Requirements

### Validated

- ✓ In-memory LiveIndex with 13-language tree-sitter parsing — v1.0
- ✓ MCP protocol over stdio (rmcp crate) — v1.0
- ✓ File watcher keeps index fresh within 200ms — v1.0
- ✓ Cross-reference extraction (call sites, imports, type usages) — v1.0
- ✓ PostToolUse hooks enrich Read/Edit/Write/Grep — v1.0
- ✓ HTTP sidecar for hook communication — v1.0
- ✓ Recursive type resolution in context bundles — v2.0
- ✓ Trait/interface implementation mapping — v2.0
- ✓ Enriched file context with import/export summaries — v2.0
- ✓ Dependency visualization (Mermaid/DOT) — v2.0
- ✓ Hook workflow classifier and adoption metrics — v1.6.0
- ✓ Daemon fallback when sidecar.port missing — v1.6.0
- ✓ Callee deduplication with frequency counts — v1.6.0
- ✓ Token budget enforcement in get_symbol_context — v1.6.0
- ✓ Search defaults relaxed for regex mode — v1.6.0

### Active

- [ ] validate_file_syntax tool dispatch wiring (P0 — tool registered but broken at runtime)
- [ ] search_text ranked mode over-filtering fix (P1 — ranked returns fewer results than unranked)
- [ ] C# class/constructor symbol disambiguation (P1 — every C# class triggers ambiguity error)
- [ ] Hook bootstrap diagnostics when sidecar.port missing (P2 — fail-open is correct but opaque)
- [ ] Codex ceiling documentation (P3 — clarify what SymForge can/cannot do in Codex)

### Out of Scope

- Full Codex integration — Codex lacks hook/session-start surface, client limitation not SymForge bug
- Git temporal context (churn scores, co-change) — deferred from v2.0, separate milestone
- PHP/Swift/Perl language support — ABI 15 grammar incompatibility with tree-sitter 0.24
- Multi-repo support — one LiveIndex per process, v3+

## Context

An AI code reviewer tested SymForge 1.6.0 and found 6 bugs + 3 RTK adoption gaps. Commit `d13e76b` ("daemon fallback, callee dedup, token budget, search defaults") already addressed 4 of the 9 items. The remaining 5 items are the scope of this milestone.

Key source files:
- `src/daemon.rs` — Daemon dispatch, backward-compat aliases
- `src/protocol/tools.rs` — Tool handlers and input structs
- `src/protocol/format.rs` — Output formatters
- `src/live_index/query.rs` — Symbol resolution, callee queries
- `src/live_index/search.rs` — Text search, noise policy, ranking
- `src/cli/hook.rs` — Hook bootstrap, adoption logging
- `src/sidecar/handlers.rs` — Sidecar HTTP handlers

Reference documents:
- `TODO.md` — RTK adoption follow-up handoff
- `docs/superpowers/plans/2026-03-20-reviewer-bugs-and-rtk-gaps.md` — Full bug/gap analysis

## Constraints

- **Backward compatibility**: All existing tool signatures preserved; fixes are additive
- **Fail-open semantics**: Hook changes must never break the fail-open contract
- **CI**: `cargo test --all-targets -- --test-threads=1` and `cargo fmt -- --check` must pass
- **Binary size**: No changes that increase release binary by >5MB

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Skip items already fixed in d13e76b | Callee dedup, token budget, search defaults, daemon fallback already shipped | — Pending verification |
| Research disabled for this milestone | Brownfield codebase with detailed bug analysis already documented | — Pending |
| Coarse granularity | Only 5 remaining items, fits in 3 phases max | — Pending |

---
*Last updated: 2026-03-20 after initialization*
