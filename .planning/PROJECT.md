# SymForge

## What This Is

A Rust-native MCP server that keeps an entire project live in memory, parasitically integrates with Claude Code's native Read/Edit/Write/Grep tools via PostToolUse hooks, and delivers cross-reference-powered retrieval that saves 80-95% of tokens on typical code exploration tasks. Supports 13 languages, persists index to disk for instant restart, and tracks token savings per session.

## Core Value

Measurable token savings (80%+) on multi-file code exploration — the model gets the same understanding with a fraction of the context, and it happens automatically via hooks with zero behavior change required.

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
- ✓ Kind-tier symbol disambiguation (class > constructor > method > other) — v1.6 RTK
- ✓ Hook verbose diagnostics (SYMFORGE_HOOK_VERBOSE=1) — v1.6 RTK
- ✓ Port-missing vs port-stale distinction in adoption log — v1.6 RTK
- ✓ One-time sidecar hint with freshness marker — v1.6 RTK
- ✓ Codex integration ceiling documentation — v1.6 RTK

### Active

(None — fresh for next milestone)

### Out of Scope

- Full Codex hook integration — Codex lacks hook/session-start surface, client limitation
- Git temporal context (churn scores, co-change) — deferred from v2.0, separate milestone
- PHP/Swift/Perl language support — ABI 15 grammar incompatibility with tree-sitter 0.24
- Multi-repo support — one LiveIndex per process, v3+

## Context

Shipped v1.6 RTK Adoption & Quality Fixes milestone on 2026-03-20.
17 files changed, 1191 insertions, 476 deletions across 2 phases.
Tech stack: Rust, tree-sitter (0.24), rmcp, tokio, axum, notify-debouncer-full, postcard, dashmap.

**Prior milestones:** v1.0 (7 phases, shipped 2026-03-10), v2.0 (5 phases, shipped 2026-03-12), v1.6 RTK (2 phases, shipped 2026-03-20).

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| AD-1: In-process LiveIndex over external DB | Repos <10K files fit in RAM. No IPC overhead. | ✓ Good |
| AD-2: Parasitic hooks over tool replacement | Models drift from CLAUDE.md instructions. Hooks are deterministic. | ✓ Good |
| AD-3: Syntactic xrefs only (tree-sitter) | 85% coverage in weeks vs 100% requiring months + language servers | ✓ Good |
| AD-4: 4-tier kind disambiguation | Replaces binary container-vs-member heuristic. Handles C#, Java, Kotlin class/constructor. | ✓ Good |
| AD-5: Env-var gated verbose diagnostics | No noise in normal operation, full debug when needed. | ✓ Good |
| AD-6: Marker file for one-time hints | 30-min freshness avoids repeated messages without session state. | ✓ Good |
| AD-7: Brownfield GSD with no research | Detailed bug analysis already documented; research would waste tokens. | ✓ Good |

---
*Last updated: 2026-03-20 after v1.6 RTK milestone*
