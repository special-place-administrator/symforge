# 0012. Edit-tool and Rank-signal Extension Points

Date: 2026-04-18
Status: Accepted

## Context

Two feature tentacles queued behind this refactor —
`worktree-awareness` and `frecency-ranking` — both need to modify the same
shared code paths:

- **The seven edit-tool handlers** on `impl SymForgeServer` in
  [`src/protocol/tools.rs`](../../src/protocol/tools.rs)
  (`replace_symbol_body`, `insert_symbol`, `delete_symbol`,
  `edit_within_symbol`, `batch_edit`, `batch_rename`, `batch_insert` —
  roughly L6220-6925). `worktree-awareness` needs to intercept the path the
  handler writes to (redirecting `symbol.indexed_absolute_path` when the caller
  supplies a `working_directory`). `frecency-ranking` needs to observe every
  committed edit to update per-file frecency scores.
- **The search ranker fusion** that feeds `search_files`
  ([`src/protocol/tools.rs`](../../src/protocol/tools.rs) L3710-3956 plus
  [`src/live_index/search.rs`](../../src/live_index/search.rs) and the
  tier-based comparator in
  [`src/live_index/query.rs`](../../src/live_index/query.rs)
  `capture_search_files_view`). Both tentacles want to contribute new scoring
  signals — worktree-local recency from one side, per-project frecency from
  the other.

Before this ADR, shipping those two features meant amending the same shared
function bodies. The concrete cost shows up three ways:

1. **Lock-step delivery.** Two feature tentacles cannot both edit the same
   handler body in parallel. One has to land, the other has to rebase, and
   every later feature that needs the same hook point repeats the cycle.
2. **Hidden coupling.** Every new scoring input added inline to the ranker
   fusion grows the blast radius of any future weight rebalance. Today's
   path-match and co-change contributions are already entangled with the
   tier comparator; another inline addition would compound it.
3. **Shared-body merge conflicts.** Small edits to different parts of the
   same 100-line handler land as textual conflicts even when the intents
   are orthogonal. The refactor cost is paid by whichever tentacle loses
   the race.

The project's five-domain tentacle layout depends on orthogonal shared
surfaces. Leaving the edit handlers and ranker fusion as non-extensible
shared bodies is the single biggest blocker to parallel feature delivery
in this codebase.

## Decision

Introduce two registration-based extension points, owned by this tentacle
and consumed by feature tentacles without further edits to the shared
bodies. The layer that owns each extension point does not know about any
specific feature — feature tentacles register their impls; the layer walks
whatever is registered.

### `EditHook` — the shared-edit-body extension point

`src/protocol/edit_hooks.rs` (new module) defines:

```rust
pub trait EditHook: Send + Sync {
    fn resolve_target_path(&self, ctx: &EditContext) -> Result<PathBuf>;
    fn after_edit_committed(&self, ctx: &EditContext, resolved_path: &Path);
}

pub struct DefaultEditHook;           // no-op: returns ctx's indexed path unchanged
pub fn register(hook: Box<dyn EditHook>);
pub fn resolve(ctx: &EditContext) -> Result<PathBuf>;
pub fn after_commit(ctx: &EditContext, resolved: &Path);
```

The seven edit handlers in `src/protocol/tools.rs` migrate to call
`edit_hooks::resolve(&ctx)` in place of their inline `indexed_absolute_path`
read, and `edit_hooks::after_commit(&ctx, &resolved)` after a successful
write. `DefaultEditHook` is registered at module-init time so the registry
is never empty and today's behavior is preserved byte-for-byte.

Feature tentacles then plug in without touching the handlers:

- `worktree-awareness` registers a hook whose `resolve_target_path` rewrites
  the indexed path to the caller-supplied working-directory root.
- `frecency-ranking` registers a hook whose `after_edit_committed` bumps the
  per-file frecency score in its own SQLite store.

### `RankSignal` — the ranker-fusion extension point

[`src/live_index/rank_signals.rs`](../../src/live_index/rank_signals.rs)
(new module) defines:

```rust
pub trait RankSignal: Send + Sync {
    fn name(&self) -> &'static str;
    fn weight(&self) -> f32;
    fn score(&self, path: &Path, ctx: &RankCtx) -> f32;
}

pub struct PathMatchSignal;            // reserves the slot for today's lexical contribution
pub struct CoChangeSignal;             // reserves the slot for today's git-temporal contribution
pub fn register(signal: Box<dyn RankSignal>);
pub fn combine(path: &Path, ctx: &RankCtx) -> f32;   // weighted-sum fusion
```

`PathMatchSignal` and `CoChangeSignal` are registered at module-init time.
Feature tentacles register additional signals — e.g.
`frecency-ranking` adds a `FrecencySignal` whose `score()` reads its
SQLite store and returns a per-path decayed-recency value; its weight is
chosen by the tentacle that registers it, not by this layer.

### Registration model — feature tentacles plug in; this layer is feature-blind

Both extension points share one property: the layer that defines the trait
does not know about any specific feature. `src/protocol/edit_hooks.rs` has
no `worktree` identifier in it; `src/live_index/rank_signals.rs` has no
`frecency` identifier in it. Each feature tentacle owns its own module
(`src/worktree/`, `src/live_index/frecency.rs`) and calls `register()` at
its own initialization time. Registration is order-independent; weights
compose via the fused sum; path-resolution hooks compose by the first
non-default hook winning (with `DefaultEditHook` as the terminal fallback).

### Deferred consumer — ranker fusion stays tier-based for now

`RankSignal::combine()` is the target fusion point, but today's ranker in
`src/live_index/query.rs::capture_search_files_view` is tier-based
(`StrongPath` / `Basename` / `LoosePath` / `CoChange`), not weighted-sum.
Migrating tier logic into weighted signals is itself a behavior change and
violates this tentacle's byte-identical parity bar. This ADR lands the
scaffolding (traits, default signals, registry, `combine()` returning
`0.0` for every default) while leaving the search layer on its existing
comparator. The consumer migration is a follow-up tentacle whose acceptance
bar is "rankings match today's tier-based goldens under weighted-sum." See
[`src/live_index/rank_signals.rs`](../../src/live_index/rank_signals.rs)
module-level docs for the caveat in code form.

## Consequences

**Easier**

- Feature tentacles ship in parallel. `worktree-awareness` and
  `frecency-ranking` no longer serialize on shared-body edits — each owns
  its own module and registers at init time. Future features that need the
  same hook points get the same plug-in surface for free.
- Shared handlers stop growing. The seven edit handlers and the ranker
  fusion reach a size ceiling: new behavior lives in registered impls, not
  in the handler bodies.
- Byte-identical refactor shield. Every change in this tentacle is a pure
  extraction — `DefaultEditHook` returns the unchanged path, the two
  default `RankSignal` impls score `0.0`, and the handlers route through
  the registry instead of inline reads. The parity tests in
  [`tests/edit_hook_behavior.rs`](../../tests/edit_hook_behavior.rs) and
  [`tests/rank_signal_behavior.rs`](../../tests/rank_signal_behavior.rs)
  gate this invariant; see ADR 0012's acceptance bar (and this tentacle's
  CONTEXT.md §Acceptance) — *invisibility is the bar*.

**Harder**

- Two new process-wide registries to reason about. Both
  `src/protocol/edit_hooks.rs` and
  [`src/live_index/rank_signals.rs`](../../src/live_index/rank_signals.rs)
  hold a `OnceLock<RwLock<Vec<Box<dyn _>>>>` whose ordering depends on
  registration order. Tests that assume a specific registry state must
  call the module's `reset_for_tests()` helper (test-only) to restore
  defaults; ordinary callers cannot observe registration order.
- Registration timing discipline. A feature tentacle that registers its
  hook *after* the first request hits the handler silently falls through
  to `DefaultEditHook` / scoreless signals. Every feature tentacle MUST
  register at module-init (via a `ctor`-style call from its module's
  `init()` or similar), not lazily on first use.
- The `RankSignal` scaffolding is inert until the search-layer migration
  lands. Anyone reading `combine()` today sees `0.0` returns and may
  mistake the scaffolding for dead code. The module docstring and the
  deferred-consumer paragraph above exist to prevent that reading; still,
  the follow-up tentacle is a real commitment and should ship before any
  new signal is added.

**New invariants future code must respect**

1. The seven edit handlers in `src/protocol/tools.rs` MUST route path
   resolution through `edit_hooks::resolve()` and post-write bookkeeping
   through `edit_hooks::after_commit()`. Inlining either step again is a
   regression — it re-couples the handler to a specific feature's
   concerns. The parity tests in
   [`tests/edit_hook_behavior.rs`](../../tests/edit_hook_behavior.rs)
   guard the *behavior* side of this invariant; code review guards the
   *structural* side.
2. `src/protocol/edit_hooks.rs` and
   [`src/live_index/rank_signals.rs`](../../src/live_index/rank_signals.rs)
   MUST NOT import anything feature-specific. A reference to `worktree`,
   `frecency`, or any other feature name inside these files is a layering
   violation; feature-specific code belongs in the feature's own module.
3. `DefaultEditHook` and the two default `RankSignal` impls
   (`PathMatchSignal`, `CoChangeSignal`) MUST remain registered at
   module-init. Removing either default empties the fallback path and
   can silently break callers that relied on no-op behavior.
4. Trait-object safety is load-bearing. Both `EditHook` and `RankSignal`
   are `Send + Sync` with only `&self` methods so feature tentacles can
   register via `Box<dyn _>`. Any future method addition MUST preserve
   object-safety; a non-object-safe signature breaks every registered
   impl.
5. When the search-layer migration lands and `combine()` becomes the
   live fusion point, the follow-up tentacle MUST preserve today's
   tier-based rankings as the parity bar. Today's implicit weights
   (additive, unit-weight per tier's contribution) become the starting
   point; a weight rebalance is a separate, ADR-worthy change.
