# Phase 2: Diagnostics & Documentation - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Enrich hook diagnostic output when sidecar is unavailable: gate stderr output behind SYMFORGE_HOOK_VERBOSE env var, ensure NoSidecar log entries distinguish "missing" vs "stale" port files, and limit the hint message to once per session. Documentation (DOCS-01, DOCS-02) already completed by Kilo Code.

</domain>

<decisions>
## Implementation Decisions

### Hook Diagnostics
- `emit_no_sidecar_diagnostic` already exists at hook.rs:848 — needs to be gated behind `SYMFORGE_HOOK_VERBOSE=1` env var check
- `NoSidecarDetail` struct exists at hook.rs:837 with `reason`, `searched_path`, `suggestion` fields — verify the `reason` field distinguishes "port file missing" from "port file stale"
- One-time hint: use a static AtomicBool or check if the adoption log already has a recent entry for this session to avoid repeating the hint on every hook invocation
- `record_hook_outcome_with_detail` at hook.rs:869 already accepts `NoSidecarDetail` — ensure it's called with the right detail

### Documentation (ALREADY COMPLETE)
- DOCS-01: `docs/codex-integration-ceiling.md` created by Kilo Code (297 lines)
- DOCS-02: CLAUDE.md line 144 references the doc
- No work needed — mark as complete in REQUIREMENTS.md

### Claude's Discretion
- Exact wording of diagnostic messages
- Whether to use AtomicBool vs file-based once-per-session gating
- Whether stale detection checks port file age or tries to connect

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `emit_no_sidecar_diagnostic` at hook.rs:848 — already prints to stderr with repo root and daemon status
- `NoSidecarDetail` struct at hook.rs:837 — already has reason/searched_path/suggestion
- `record_hook_outcome_with_detail` at hook.rs:869 — already writes structured detail to adoption log
- `ADOPTION_LOG_FILE` constant at hook.rs:20 — log file path

### Established Patterns
- Hook output: JSON on stdout only, diagnostics on stderr
- Adoption log: append-only event log at `.symforge/hook-adoption.log`
- Fail-open: hooks must never block or error — always return valid JSON

### Integration Points
- `run_hook` at hook.rs:209 — the call site at line 264 where `emit_no_sidecar_diagnostic` is called
- No sidecar/daemon changes needed — purely hook.rs changes

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
