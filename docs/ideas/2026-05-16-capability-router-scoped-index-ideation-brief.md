# SymForge Capability Router / Scoped Index Ideation Brief

Date: 2026-05-16
Audience: GPT-5.5 Pro or another architecture reviewer with access to the SymForge repo.
Status: ideation brief, not an approved implementation spec.

## Prompt For Reviewer

You are reviewing SymForge, a Rust-native local-first MCP server for code intelligence. Please evaluate the product and architecture idea below. You may reject it, reshape it, or propose a better approach. Focus on whether the idea solves the actual agent UX problem without compromising SymForge's determinism, safety, speed, and local-first architecture.

The core question:

Should SymForge replace process-level environment-gated capabilities with a call-time capability router, scoped index views, and scoped derived stores so LLMs can request advanced capabilities when needed without restarting or preconfiguring the MCP server?

## Current Product Friction

SymForge currently advertises advanced features in the README, but several require environment variables to be set before the MCP server launches:

- `SYMFORGE_FRECENCY=1`
  - Enables frecency ranking for `search_files(rank_by="frecency")`.
  - Uses `.symforge/frecency.db`.
- `SYMFORGE_DEBUG_RANKING=1`
  - Adds per-signal scores in `search_files` responses and a last-10 bumps section in `health`.
- `SYMFORGE_COUPLING=1`
  - Builds and refreshes `.symforge/coupling.db`.
  - Enables useful evidence for `search_files(rank_by="path+cochange", anchor_path=...)`.
- `SYMFORGE_WORKTREE_AWARE=1`
  - Enables edit-tool write rerouting when callers supply `working_directory`.

The README currently says these are unset by default. The stated rationale was compatibility, determinism, and avoiding background work or persistent state unless explicitly requested.

User objection:

For an MCP agent product, this makes the advertised functionality feel unavailable. An LLM normally cannot restart the MCP server with new environment variables mid-task. If a capability appears in the tool schema or README, the LLM should be able to request it on the fly. Normal behavior should remain unchanged unless the call asks for the advanced behavior.

This is especially important for:

- `rank_by="frecency"`: the LLM should be able to ask for frecency ranking when useful.
- `rank_by="path+cochange"` with `anchor_path`: the LLM should be able to ask for co-change ranking when useful.
- edit tools with `working_directory`: the LLM should be able to target a worktree explicitly without hidden process-level setup.
- ranking diagnostics: ideally a call-time debug flag, not global response noise.

## Current Relevant Architecture

SymForge is local-first:

- Rust MCP server.
- In-process `LiveIndex` as the primary query engine.
- Local `.symforge/` state for snapshots and derived artifacts.
- Tree-sitter parsing and symbol/reference extraction.
- Daemon is optional but useful for sharing an index across sessions.

Important existing concepts and code areas:

- `LiveIndex`: authoritative in-memory code index.
- `.symforge/index.bin`: warm startup snapshot.
- `RankSignal`: trait-based ranking extension point.
  - See `src/live_index/rank_signals.rs`.
  - ADR: `docs/decisions/0012-edit-and-ranker-hook-architecture.md`.
- `EditHook`: trait-based edit lifecycle extension point.
  - Worktree-aware routing currently hangs off this pattern.
- Frecency:
  - `src/live_index/frecency.rs`
  - `src/live_index/persist.rs`
  - `.symforge/frecency.db`
  - Records commitment-tool bumps: edit tools and read tools that imply real work on a file.
  - Discovery tools deliberately do not bump.
- Co-change:
  - `src/live_index/coupling/`
  - `.symforge/coupling.db`
  - Built/refreshed from git history.
  - Used by `search_files(rank_by="path+cochange", anchor_path=...)`.
- Worktree awareness:
  - `src/worktree.rs`
  - edit tools accept `working_directory`.
  - Current flag gates routing behavior.
- Edit safety:
  - `src/edit_safety/tee.rs`
  - recent Wave 3 work added pre-write tee snapshots.
  - This demonstrates `.symforge/` as a local safety/artifact area.

The current model has useful capability code, but the process-level env gates make some functionality unavailable at call time.

## User's Architecture Idea

Initial idea:

Maybe multiple SymForge instances run at once, each optimized for a certain thing and each with its own index. The LLM connects to a daemon. The daemon connects to a router. The router routes calls to the right SymForge without affecting the one it is calling. This suggests multitenancy.

Follow-up refinement:

Maybe the better version is not multiple full SymForge processes, but a better organized scoped index and databases.

User intuition:

A proper schema can solve the confusion. The system should know what capability state exists, what it is scoped to, and how it should be initialized or used.

## Candidate Direction

Preferred framing to evaluate:

One daemon, many project tenants. Each tenant owns one authoritative `LiveIndex` and multiple scoped derived stores/views.

Conceptual structure:

```text
LLM / MCP client
  -> SymForge daemon
    -> capability router
      -> project tenant
        -> authoritative LiveIndex
        -> scoped index views
        -> derived stores
```

Project tenant:

```text
ProjectTenant {
  root: PathBuf,
  live_index: LiveIndex,
  scopes: ScopeRegistry,
  stores: DerivedStoreRegistry,
  policy: CapabilityPolicy,
}
```

Derived store registry:

```text
DerivedStoreRegistry {
  frecency: LazyStore<FrecencyStore>,
  coupling: LazyStore<CouplingStore>,
  semantic: Option<LazyStore<SemanticStore>>,
  edit_safety: EditSafetyArtifacts,
}
```

Scoped views might include:

- whole repo
- `src/`
- tests
- docs/config
- changed files
- current branch or commit range
- current worktree target
- language-specific slices
- active task scope

Derived stores might include:

- frecency path touch history
- co-change graph
- semantic/fuzzy recall cache in the future
- edit recovery snapshots
- trust/edit policy state
- debug/ranking traces

## Desired Product Contract

Default calls should remain deterministic and low-surprise.

Advanced behavior should be activated by tool parameters, not process env prerequisites:

```json
{
  "query": "routes",
  "rank_by": "path+cochange",
  "anchor_path": "src/auth/routes.rs",
  "scope": "src/"
}
```

Expected router behavior:

1. Resolve project tenant.
2. Resolve scope view.
3. Inspect requested capability.
4. Check required derived store or runtime state.
5. Initialize lazily if allowed and cheap enough.
6. Run base index query.
7. Apply requested rank signals or edit routing.
8. Return evidence:
   - capability applied
   - warming/incomplete
   - unavailable
   - disabled by policy
   - fallback used

Example:

```text
search_files rank_by="path+cochange"
  -> coupling store exists and fresh
  -> apply co-change signal
  -> response: "Co-change ranking active..."
```

Fallback example:

```text
search_files rank_by="path+cochange"
  -> coupling store absent
  -> schedule lazy build or run bounded warmup
  -> return path-ranked results now
  -> response: "Co-change evidence warming; returned path ranking."
```

This is better than requiring `SYMFORGE_COUPLING=1` before startup.

## Capability/State Schema Idea

SymForge may need a capability-state schema in addition to MCP tool schemas.

Potential schema fields:

```text
CapabilityState {
  capability_name,
  tenant_key,
  scope_key,
  backing_store,
  source_authority,
  freshness,
  freshness_policy,
  init_policy,
  fallback_policy,
  cost_class,
  safety_class,
  availability,
  last_error,
  response_evidence,
}
```

Possible values:

- source authority:
  - current file bytes
  - git history
  - user activity
  - derived analysis
  - external semantic model, if ever added
- freshness:
  - current
  - stale
  - unknown
  - warming
  - unavailable
- cost class:
  - cheap
  - lazy
  - background
  - expensive
- safety class:
  - read-only
  - ranking-only
  - persistent-derived-state
  - write-routing
  - destructive/recovery
- init policy:
  - always-on
  - lazy-on-first-request
  - lazy-with-background-refresh
  - manual-only
  - disabled-by-policy
- fallback policy:
  - preserve default output
  - include note
  - fail loudly
  - return unavailable

Potential key product requirement:

If a capability is in the tool schema, the system should either run it or return a precise, actionable capability-state explanation. It should not require silent pre-launch env setup.

## Specific Capability Questions

### Frecency

Problem:

Frecency ranking needs history. If collection only starts after the first `rank_by="frecency"` call, early results are weak.

Options:

1. Always collect lightweight frecency bumps by default, but only use frecency in ranking when requested.
2. Start collecting lazily after first request and clearly say there is not much history yet.
3. Keep env opt-in but expose an MCP tool to enable it for the current daemon/tenant.
4. Use an in-memory recent-session frecency signal by default, with persistent `.symforge/frecency.db` as optional.

Questions:

- Is always-on lightweight collection acceptable?
- How much statefulness is too much for default SymForge?
- Should frecency be per repo, per user, per session, per worktree, or scoped by task?
- How should privacy and surprise be handled?

### Co-change Coupling

Problem:

Co-change ranking depends on git-history analysis and `.symforge/coupling.db`, which may be non-trivial to build or refresh.

Options:

1. Lazy build on first `rank_by="path+cochange"` call.
2. Background warm whenever daemon starts in a git repo.
3. Build only when requested and return fallback while warming.
4. Keep env var only as `DISABLE` or background policy override, not a requirement.

Questions:

- Should first call block for bounded initialization or immediately fall back?
- What freshness guarantee is needed for git-derived ranking?
- Should coupling be scoped to repo root only, or can it support subdir/language scopes?
- Should the store be considered advisory and stale-tolerant?

### Worktree-Aware Edits

Problem:

Worktree routing changes write-target semantics. It is risky to enable silently, but requiring a process env var defeats call-time use.

Potential better contract:

If an edit tool receives `working_directory`, treat that as explicit opt-in for that call. Validate the path is a known/safe worktree. Surface `rerouted`, `wrote_to`, and `indexed_path` in every response.

Questions:

- Is the presence of `working_directory` enough consent?
- Should routing still require a trust gate?
- Should unknown worktree paths fail loudly?
- Should all edit tools get a unified "resolved target" evidence block?

### Debug Ranking

Problem:

Debug ranking is response noise if globally enabled, but useful when diagnosing tool behavior.

Potential better contract:

Add a call-time `debug_ranking: bool` or `explain: ["ranking"]` parameter.

Questions:

- Should debug output be controlled by tool param, env var, or both?
- Should `health` always expose minimal ranking state without noisy details?
- Could an `explain_capability` tool replace debug env vars?

## Possible Architecture Approaches

### Approach A: Minimal Parameter-First Fix

Keep one daemon and current stores. Remove env prerequisites for advertised capabilities. Tool parameters trigger capability usage:

- `rank_by="frecency"` uses frecency if available; otherwise initializes or explains lack of history.
- `rank_by="path+cochange"` lazily opens/builds coupling store or falls back with note.
- `working_directory` enables worktree routing for that call after validation.
- `debug_ranking` param controls ranking diagnostics.

Pros:

- Smallest product correction.
- Makes LLM-facing tools usable immediately.
- Preserves one authoritative index.

Cons:

- Could accumulate ad hoc lazy-init logic in handlers.
- Does not fully solve capability observability unless a state schema is added.

### Approach B: Capability Router + Derived Store Registry

Introduce a capability router inside the daemon/protocol layer. Tool handlers declare required capabilities. Router resolves state, initializes stores, applies fallback policy, and returns evidence.

Pros:

- Coherent long-term architecture.
- Scales to frecency, coupling, semantic search, trust gates, scoped views, and debug explainers.
- Makes capabilities inspectable and testable.

Cons:

- Bigger refactor.
- Requires careful boundaries to avoid overengineering.
- Must not slow common paths.

### Approach C: Multi-Process / Multi-Index Router

Run multiple SymForge instances per project, each optimized for a capability profile. Daemon routes calls to specialized instances.

Pros:

- Strong isolation.
- Different profiles can have different state/cost/safety policies.
- Could support read-only vs write-capable agents.

Cons:

- More memory and process overhead.
- Risk of index drift.
- Harder edit consistency.
- Likely unnecessary before proving simpler scoped-store architecture is insufficient.

## Initial Recommendation To Evaluate

The likely best direction is Approach B, but implemented incrementally:

1. Stop treating env vars as prerequisites for advertised tool behavior.
2. Introduce a small capability-state abstraction, not a full framework.
3. Convert frecency and coupling to call-time lazy capability checks.
4. Convert worktree awareness to explicit per-call opt-in via `working_directory`, guarded by validation and response evidence.
5. Add an `explain` or `debug_ranking` parameter for ranking diagnostics.
6. Keep env vars as operational overrides:
   - disable feature
   - force background warm
   - force debug default
   - cap cost

This keeps one authoritative `LiveIndex` and adds scoped derived stores where needed.

## Important Constraints

Do not compromise these SymForge principles:

- Local-first query serving.
- Read path should stay in-process and fast.
- Current file bytes and symbol spans are authoritative.
- Derived stores are advisory and must not silently override current bytes.
- Default tool behavior should stay deterministic unless caller requests adaptive ranking.
- Corruption should be quarantined, not served.
- Long-running operations should be resumable or bounded.
- Mutating operations should be fail-safe and explicit.
- MCP clients and LLMs should not need to know server launch environment details to use advertised tool parameters.

## Concrete Questions For GPT-5.5 Pro

1. Is the product critique valid: are env-gated MCP capabilities a bad UX when the tool schema advertises the feature?
2. Should frecency collection be always-on, lazy, in-memory by default, or still opt-in?
3. Should co-change coupling build lazily on first request, warm in background, or stay manual?
4. Is `working_directory` itself sufficient opt-in for worktree-aware writes?
5. What minimal capability-state schema should SymForge add without creating architecture bloat?
6. Should scoped index views be explicit user-facing tool params, internal router concepts, or both?
7. Does a multi-process/multi-index router solve a real problem now, or is it premature?
8. What is the safest migration path from env vars to call-time capabilities?
9. What should responses say when a requested capability cannot be applied?
10. What tests/acceptance criteria would prove the architecture works?

## Desired Output From Reviewer

Please produce:

1. A clear recommendation: accept, reject, or reshape the idea.
2. A minimal architecture proposal.
3. A migration plan from current env-gated behavior.
4. Risks and failure modes.
5. Acceptance criteria and tests.
6. Any code areas in the repo that should be changed first.

## Non-Goals For First Pass

- Do not design a cloud service.
- Do not add an external control plane.
- Do not replace `LiveIndex` as source of truth.
- Do not require multiple processes unless there is a strong reason.
- Do not make default search results non-deterministic unless the caller requested adaptive behavior.
- Do not silently route writes to another worktree without explicit call-time evidence.

