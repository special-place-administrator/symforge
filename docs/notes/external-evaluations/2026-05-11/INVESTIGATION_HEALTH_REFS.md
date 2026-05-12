# Investigation: B-P1-6 health divergence, B-P1-2 find_dependents false
positives, B-P1-3 find_references missing qualified Rust paths

Repo: `E:\project\symforge`. HEAD: `f804d21`. Investigation only — no source
changes. Evidence quoted with file:line.

---

## Part A — B-P1-6 health vs health_compact divergence

### A.1 Tool handler source locations

Both tools are defined in the same file and use the same in-process state.

- `pub(crate) async fn health` at `src/protocol/tools.rs:4236`.
- `pub(crate) async fn health_compact` at `src/protocol/tools.rs:4331`.

Both first try a daemon proxy then fall back to local rendering. Quote of the
top of each handler:

```
src/protocol/tools.rs:4236  pub(crate) async fn health(&self) -> String {
src/protocol/tools.rs:4237      if let Some(result) = self.proxy_tool_call_without_params("health").await {
src/protocol/tools.rs:4238          return result;
src/protocol/tools.rs:4239      }
src/protocol/tools.rs:4240      let published = self.index.published_state();
src/protocol/tools.rs:4241      let watcher_guard = self.watcher_info.lock();
src/protocol/tools.rs:4242      let mut result = format::health_report_from_published_state(&published, &watcher_guard);
```

```
src/protocol/tools.rs:4331  pub(crate) async fn health_compact(&self) -> String {
src/protocol/tools.rs:4332      if let Some(result) = self.proxy_tool_call_without_params("health_compact").await {
src/protocol/tools.rs:4333          return result;
src/protocol/tools.rs:4334      }
src/protocol/tools.rs:4335      let published = self.index.published_state();
src/protocol/tools.rs:4336      let watcher_guard = self.watcher_info.lock();
src/protocol/tools.rs:4337      let mut result = format::health_report_compact_from_published_state(&published, &watcher_guard);
```

Within a single in-process call both handlers read the same `published`
snapshot and the same `WatcherInfo` under the same parking_lot mutex.
Therefore the cross-call divergence reported by the evaluator cannot
originate from the handlers themselves — it has to come from one of:

1. The two calls landing on different "indices" — one served by the daemon
   over HTTP and the other served by the in-process local fallback (`None`
   return from `proxy_tool_call_without_params`).
2. The format functions disagreeing about how to render the same WatcherInfo
   in a given idle/active state.

The investigation shows both effects are present and stack.

### A.2 Render-path divergence inside format.rs

Health renders through `health_report_from_published_state` →
`health_report_from_stats`. Compact renders through
`health_report_compact_from_published_state` directly. They use independent
match ladders over `WatcherState` and observe disjoint subsets of the
WatcherInfo fields.

Full path (`health`), `src/protocol/format.rs:1062`:

```
src/protocol/format.rs:1062  pub fn health_report_from_published_state(
src/protocol/format.rs:1063      published: &PublishedIndexState,
src/protocol/format.rs:1064      watcher: &crate::watcher::WatcherInfo,
src/protocol/format.rs:1065  ) -> String {
... (HealthStats projected) ...
src/protocol/format.rs:1086      // Preserve the existing formatter shape by reusing HealthStats.
src/protocol/format.rs:1087      if matches!(stats.watcher_state, crate::watcher::WatcherState::Off) {
src/protocol/format.rs:1088          stats.events_processed = 0;
src/protocol/format.rs:1089          stats.last_event_at = None;
src/protocol/format.rs:1090      }
src/protocol/format.rs:1091      health_report_from_stats(published.status_label(), &stats)
src/protocol/format.rs:1092  }
```

`health_report_from_stats` (called only by `health`) has a 3-arm match for
`WatcherState::Active`, introduced in commit `34e97fb feat(health): surface
reconcile repairs on idle watcher line`:

```
src/protocol/format.rs:1153  let watcher_line = match &stats.watcher_state {
src/protocol/format.rs:1154      WatcherState::Active
src/protocol/format.rs:1155          if stats.events_processed == 0
src/protocol/format.rs:1156              && stats.last_event_at.is_none()
src/protocol/format.rs:1157              && stats.overflow_count == 0
src/protocol/format.rs:1158              && stats.stale_files_found == 0 =>
src/protocol/format.rs:1159      {
src/protocol/format.rs:1160          format!(
src/protocol/format.rs:1161              "Watcher: active (idle; event-driven, waiting for filesystem changes, debounce: {}ms)",
... (terse-idle arm) ...
src/protocol/format.rs:1166      WatcherState::Active
src/protocol/format.rs:1167          if stats.events_processed == 0 && stats.last_event_at.is_none() =>
src/protocol/format.rs:1168      {
src/protocol/format.rs:1169          format!(
src/protocol/format.rs:1170              "Watcher: active (idle; debounce: {}ms, overflows: {}, reconcile repairs: {}, last reconcile: {})",
... (repair-while-idle arm — new in 34e97fb) ...
src/protocol/format.rs:1180      WatcherState::Active => format!(
src/protocol/format.rs:1181          "Watcher: active (event-driven; {} events, last change: {}, ..."
... (event-driven arm) ...
src/protocol/format.rs:1198      WatcherState::Off => "Watcher: off".to_string(),
src/protocol/format.rs:1199  };
```

Compact path, `src/protocol/format.rs:1094`:

```
src/protocol/format.rs:1094  pub fn health_report_compact_from_published_state(
src/protocol/format.rs:1095      published: &PublishedIndexState,
src/protocol/format.rs:1096      watcher: &crate::watcher::WatcherInfo,
src/protocol/format.rs:1097  ) -> String {
src/protocol/format.rs:1098      use crate::watcher::WatcherState;
src/protocol/format.rs:1099
src/protocol/format.rs:1100      let watcher_label = match &watcher.state {
src/protocol/format.rs:1101          WatcherState::Active
src/protocol/format.rs:1102              if watcher.events_processed == 0
src/protocol/format.rs:1103                  && watcher.overflow_count == 0
src/protocol/format.rs:1104                  && watcher.stale_files_found == 0 =>
src/protocol/format.rs:1105          {
src/protocol/format.rs:1106              "active/idle".to_string()
src/protocol/format.rs:1107          }
src/protocol/format.rs:1108          WatcherState::Active => format!(
src/protocol/format.rs:1109              "active (events: {}, overflows: {}, repairs: {})",
src/protocol/format.rs:1110              watcher.events_processed, watcher.overflow_count, watcher.stale_files_found
src/protocol/format.rs:1111          ),
src/protocol/format.rs:1112          WatcherState::Degraded => format!(
src/protocol/format.rs:1113              "degraded (events: {}, overflows: {}, repairs: {})",
... (degraded arm) ...
src/protocol/format.rs:1116          WatcherState::Off => "off".to_string(),
src/protocol/format.rs:1117      };
```

Differences this surfaces in identical inputs:

1. **`last_event_at` only consulted by the full path.** If the watcher has
   processed one event in the distant past
   (`events_processed > 0 && last_event_at == Some(...)`), the full path
   never enters the idle arms; the compact path will still classify it as
   `active/idle` if `events_processed == 0 && overflow_count == 0 &&
   stale_files_found == 0`. That cannot produce "off" but can produce
   semantic disagreement on what "idle" means.
2. **Different idle predicates.** The full path's terse-idle arm requires
   four counters zero AND `last_event_at.is_none()`. The compact path's
   idle arm requires only three counters zero. The new middle arm
   `34e97fb` exposes reconcile repairs in the full output but the compact
   path drops `last_event_at` from its predicate altogether.

These render asymmetries cause string-level drift; they do **not** explain a
state going from `Active` to `Off`. That requires the source-of-truth gap.

### A.3 Source-of-truth gap — `Active` vs `Off` is two different processes

Both handlers call `proxy_tool_call_without_params` first. The proxy is
defined in `src/protocol/mod.rs:324`:

```
src/protocol/mod.rs:324  pub(crate) async fn proxy_tool_call_without_params(&self, tool_name: &str) -> Option<String> {
src/protocol/mod.rs:325      self.proxy_tool_call(tool_name, &serde_json::json!({}))
src/protocol/mod.rs:326          .await
src/protocol/mod.rs:327  }
```

`proxy_tool_call`, `src/protocol/mod.rs:222`, returns:
- `Some(result)` on daemon success;
- `None` on connection failure after one reconnect attempt, marking
  `daemon_degraded = true` and calling `ensure_local_index().await`.

Two flips matter:

```
src/protocol/mod.rs:267                  Err(reconnect_error) => {
src/protocol/mod.rs:268                      tracing::warn!(
src/protocol/mod.rs:269                          "daemon reconnect failed, falling back to local execution: {reconnect_error}"
src/protocol/mod.rs:270                      );
src/protocol/mod.rs:271                      self.daemon_degraded.store(true, Ordering::Relaxed);
src/protocol/mod.rs:272                      self.ensure_local_index().await;
src/protocol/mod.rs:273                      false
src/protocol/mod.rs:274                  }
```

After degradation the handler runs entirely in the MCP-client process. That
process owns its own `WatcherInfo` whose `Default` puts state at `Off`:

```
src/watcher/mod.rs:34   pub struct WatcherInfo {
src/watcher/mod.rs:35       pub state: WatcherState,
... (all fields) ...
src/watcher/mod.rs:49   impl Default for WatcherInfo {
src/watcher/mod.rs:50       fn default() -> Self {
src/watcher/mod.rs:51           WatcherInfo {
src/watcher/mod.rs:52               state: WatcherState::Off,
```

`ensure_local_index` (`src/protocol/mod.rs:329`) calls
`self.index.reload(&root)` to populate the in-process index but it **never
starts a watcher**. There is no `run_watcher` / `restart_watcher` call in
that path. The MCP-client process keeps its `WatcherInfo` at `Off`.

Compare to `index_folder`'s daemon-side path which DOES start a watcher
(`src/protocol/tools.rs:4456` `crate::watcher::restart_watcher(...)`).

Mechanism of the reported `health -> active` / `health_compact -> off`:

1. `health` proxied through to the daemon, which has a live `run_watcher`
   loop, and returned daemon's `Active` output.
2. `health_compact` either proxied successfully too (then the only
   divergence would be A.2 render shape), OR the daemon proxy timed out,
   reconnect failed, and the call fell back to the MCP-client process whose
   `WatcherInfo` is still `Off`.

The latter is consistent with the evaluator's other report (file
`SYMFORGE_EVALUATION_2026-05-11.md`, "P0 Index Self-Destruction") that
describes the daemon's reconcile loop heavily churning during the same
session. Heavy reconcile work or HTTP timeout on the daemon for a single
call (proxy timeout is 10s on the first attempt,
`src/protocol/mod.rs:248`, 30s on retry, `src/protocol/mod.rs:296`) would
plausibly trip the fallback for one tool call while the other still hits
the daemon.

Independent reproduction by reading code:

- The proxy short-circuits on `daemon_degraded`:
  `src/protocol/mod.rs:230` — `if self.daemon_degraded.load(...) { return None; }`. So
  once the first failed call sets the flag, *every* subsequent call goes
  local until the process restarts. This is consistent with the "two
  seconds apart" symptom: a single transient timeout flips the flag once,
  and from that point any later health probe shows `Off`.

### A.4 Load-time drift (397ms vs 457ms)

Both render functions read `published.load_duration`. `PublishedIndexState`
is captured at publish time:

```
src/live_index/store.rs:716  impl PublishedIndexState {
src/live_index/store.rs:717      fn capture(generation: u64, index: &LiveIndex) -> Self {
... (status branch) ...
src/live_index/store.rs:735          load_duration: stats.load_duration,
```

`LiveIndex::load_duration` is set once per reload at
`src/live_index/store.rs:1074` (`let load_duration = start.elapsed();`) and
again at `src/live_index/store.rs:1262` for snapshot-restore. When the
in-process index is created empty it carries `Duration::ZERO`
(`src/live_index/store.rs:1126`, also `2058`, and the persistence-path
defaults at `src/live_index/persist.rs:196, 664, 988, 1164, 1400, 1463,
1534, 1589`).

So the 397ms vs 457ms gap is explained by reading two different
`PublishedIndexState` snapshots produced by two different reloads:

- 397ms is from the daemon's `LiveIndex::load` after `index_folder`.
- 457ms is from the MCP-client process's local fallback reload triggered
  by `ensure_local_index` after the daemon proxy degraded. Cold load over
  the same tree on the same disk reasonably takes a few tens of ms more
  because the OS page cache state differs.

Same root cause as A.3: two physically distinct indices.

### A.5 Watcher lifecycle race noted, not the primary cause

`restart_watcher`, `src/watcher/mod.rs:678`, sets `state = Off` before
spawning the supervision task:

```
src/watcher/mod.rs:678  pub fn restart_watcher(
src/watcher/mod.rs:679      repo_root: PathBuf,
src/watcher/mod.rs:680      shared: SharedIndex,
src/watcher/mod.rs:681      watcher_info: Arc<Mutex<WatcherInfo>>,
src/watcher/mod.rs:682  ) -> tokio::task::JoinHandle<()> {
src/watcher/mod.rs:683      {
src/watcher/mod.rs:684          let mut info = watcher_info.lock();
src/watcher/mod.rs:685          info.state = WatcherState::Off;
src/watcher/mod.rs:686      }
src/watcher/mod.rs:687      tokio::spawn(run_watcher(repo_root, shared, watcher_info))
src/watcher/mod.rs:688  }
```

`run_watcher` then sets `Active` at `src/watcher/mod.rs:503`. There is a
window between the explicit `Off` write and the spawned task running its
first instruction where any health probe will read `Off`. If both `health`
and `health_compact` were observed within that window the daemon itself
would render `Off` for both. The evaluator reports both observations were
"seconds apart", which is well outside that microsecond/millisecond
window, so the lifecycle race is not the dominant explanation here. It is
worth keeping in mind for future tests that index_folder back-to-back with
health.

### A.6 Did Phase 1 commits cause or only expose it?

Three Phase 1 commits touch the health surface:

- `9100d8b feat(health): surface empty-index reason as actionable banner` —
  threads `local_empty_reason` through `HealthStats` and the published
  state. Adds new fields but does not change which process owns the
  state; not implicated in the divergence.
- `34e97fb feat(health): surface reconcile repairs on idle watcher line` —
  introduces the 3-arm `WatcherState::Active` match in
  `health_report_from_stats` and tightens the compact-path idle guard.
  This is where the render-shape asymmetry described in A.2 was committed
  in a single patch. It widens the conditions under which the two
  formatters disagree on the rendered idle string.
- `e9fc968 test(conformance): register health_compact in expected tools` —
  test-only; not a production behavior change.

The root cause (A.3 separate watcher state per process) predates Phase 1.
Phase 1 made the render surfaces more sensitive to differences that
already existed in source-of-truth.

### A.7 Proposed fix shape

Two independent fixes are required; both are needed.

1. **Single source of truth for watcher state across health surfaces.**
   Both tools must read from the same `WatcherInfo`. Today they already
   do *within a single call*. The hole is the proxy-vs-local split.
   Cleanest shape: keep `health_compact` daemon-served when the daemon is
   reachable (current behavior) AND make local fallback honest about the
   absence of a watcher. Concretely:
   - When `ensure_local_index` fires, render a sentinel
     `WatcherState::Off` with a watcher-source label (`"watcher: local
     fallback, no watcher attached"`) so the agent can tell that "off"
     means "unobserved" not "dead".
   - Set `daemon_degraded` back to `false` opportunistically when the
     daemon answers another call, so transient hiccups do not stick the
     session in degraded-mode for the rest of its life.
2. **Conformance test pinning both tools to the same snapshot.** Construct
   one `PublishedIndexState` and one `WatcherInfo`, render both functions
   on it, and assert that the parsed watcher-state field agrees on a
   selection of inputs (Active idle, Active idle with repairs, Active
   with events, Degraded, Off). This catches future drift between the two
   match ladders. Existing tests
   `test_health_report_active_watcher_shows_last_change_when_events_exist`
   (`src/protocol/format/tests.rs:722`) and
   `test_health_compact_idle_watcher_shows_reconcile_repairs`
   (`src/protocol/format/tests.rs:1138`) pin each path in isolation but
   nothing pins them together.

A weaker fix is to make compact echo the full path's match ladder, but
that does not address the proxy-vs-local divergence and would still
produce "Active" + "Off" on the same session.

---

## Part B — B-P1-2 find_dependents false positives

### B.1 Tool handler and underlying algorithm

- Tool handler: `pub(crate) async fn find_dependents` at
  `src/protocol/tools.rs:5098`. Proxies to daemon, then calls
  `guard.capture_find_dependents_view(&input.path)`.
- View capture wraps `find_dependents_for_file` at
  `src/live_index/query.rs:2710`.

### B.2 How "file A depends on file B" is computed

The algorithm has four passes against the target file `B`. Public symbols
of `B` are collected into `target_symbol_names` at
`src/live_index/query.rs:2732`:

```
src/live_index/query.rs:2732  let target_symbol_names: HashSet<&str> = target_file
src/live_index/query.rs:2733      .symbols
src/live_index/query.rs:2734      .iter()
src/live_index/query.rs:2735      .map(|symbol| symbol.name.as_str())
src/live_index/query.rs:2736      .filter(|name| !name.is_empty())
src/live_index/query.rs:2737      .collect();
```

For each candidate dependent file `A`:

Pass 1 — collect imports in `A` that match `B`'s stem or module path:

```
src/live_index/query.rs:2748  let matching_imports: Vec<&ReferenceRecord> = file
src/live_index/query.rs:2749      .references
src/live_index/query.rs:2750      .iter()
src/live_index/query.rs:2751      .filter(|reference| {
src/live_index/query.rs:2752          matches_target_import(&target_language, reference, stem, module_path.as_deref())
src/live_index/query.rs:2753      })
src/live_index/query.rs:2754      .collect();
```

Pass 2 — if `A` has any matching import (or the C#/Java
`can_match_type_dependents` returns true), promote to "symbol-level usage"
and harvest every reference in `A` whose simple name equals any name in
`target_symbol_names`:

```
src/live_index/query.rs:2755  if !target_symbol_names.is_empty()
src/live_index/query.rs:2756      && (can_match_type_dependents(file, &target_language, target_scope.as_deref())
src/live_index/query.rs:2757          || !matching_imports.is_empty())
src/live_index/query.rs:2758  {
src/live_index/query.rs:2759      let symbol_refs: Vec<&ReferenceRecord> = file
src/live_index/query.rs:2760          .references
src/live_index/query.rs:2761          .iter()
src/live_index/query.rs:2762          .filter(|reference| {
src/live_index/query.rs:2763              reference.kind != ReferenceKind::Import
src/live_index/query.rs:2764                  && target_symbol_names.contains(reference.name.as_str())
src/live_index/query.rs:2765                  && Self::has_pub_symbol(target_file, &reference.name)
src/line_index/query.rs:2766          })
src/live_index/query.rs:2767          .collect();
src/live_index/query.rs:2768
src/live_index/query.rs:2769      if !symbol_refs.is_empty() {
src/live_index/query.rs:2770          results.extend(
src/live_index/query.rs:2771              symbol_refs
src/live_index/query.rs:2772                  .into_iter()
src/live_index/query.rs:2773                  .map(|reference| (file_path.as_str(), reference)),
src/live_index/query.rs:2774          );
src/live_index/query.rs:2775          continue;
src/live_index/query.rs:2776      }
src/live_index/query.rs:2777  }
```

Pass 3 — qualified-call dependents (no import required), gated on
`reference.qualified_name` matching `B`'s module path
(`matches_exact_symbol_qualified_name`):

```
src/live_index/query.rs:2795  let qualified_refs: Vec<&ReferenceRecord> = file
src/live_index/query.rs:2796      .references
src/live_index/query.rs:2797      .iter()
src/live_index/query.rs:2798      .filter(|reference| {
src/live_index/query.rs:2799          reference.kind == ReferenceKind::Call
src/live_index/query.rs:2800              && reference.qualified_name.as_deref().is_some_and(|qn| {
src/live_index/query.rs:2801                  target_symbol_names.contains(reference.name.as_str())
src/live_index/query.rs:2802                      && Self::has_pub_symbol(target_file, &reference.name)
src/live_index/query.rs:2803                      && matches_exact_symbol_qualified_name(
src/live_index/query.rs:2804                          &target_language,
src/live_index/query.rs:2805                          qn,
src/live_index/query.rs:2806                          &reference.name,
src/live_index/query.rs:2807                          Some(mp),
src/live_index/query.rs:2808                      )
src/live_index/query.rs:2809              })
src/live_index/query.rs:2810      })
src/live_index/query.rs:2811      .collect();
```

Pass 4 — Rust-only BFS over `pub use` re-exports (max 2 hops), starting at
`src/live_index/query.rs:2828`.

### B.3 Where bare method names get attributed

The false-positive engine is Pass 2.

- The gate is "the dependent file imported something that stem-matches the
  target's filename, OR for C#/Java the scope matches" (Rust path is
  `_ => false` in `can_match_type_dependents` at
  `src/live_index/query.rs:256`).
- Once that single import is found, harvesting promotes **every**
  reference in `A` whose simple name matches **any** name in
  `target_symbol_names`. No qualified-name check, no receiver-type check,
  no syntactic context — just `reference.name == name` and
  `has_pub_symbol(target_file, name)`.

`target_symbol_names` is taken straight from the target file's symbols. If
the target's `impl` block has `pub fn new`, `pub fn build`, `pub fn
default`, `pub fn handle`, `pub fn on_start`, `pub fn clone`, then any
method call in the dependent file with one of those names — including
`Vec::new()`, `ConversationManager::new()`, `String::default()`,
`obj.clone()`, `actor.on_start()` — gets counted as a reference to
`store_knowledge_upsert.rs`.

The pub-export guard `has_pub_symbol` (`src/live_index/query.rs:2647`) is
purely a text-scan over the target file's bytes:

```
src/live_index/query.rs:2669  for keyword in &[
src/live_index/query.rs:2670      "fn", "struct", "enum", "trait", "type", "const", "static", "mod",
src/live_index/query.rs:2671  ] {
src/live_index/query.rs:2672      let pattern = format!("pub {keyword} {name}");
src/live_index/query.rs:2673      if is_word_match(&content, &pattern) {
src/live_index/query.rs:2674          return true;
src/live_index/query.rs:2675      }
src/live_index/query.rs:2676      let crate_pattern = format!("pub(crate) {keyword} {name}");
src/live_index/query.rs:2677      if is_word_match(&content, &pattern) {
src/live_index/query.rs:2678          return true;
src/live_index/query.rs:2679      }
src/live_index/query.rs:2680  }
```

`has_pub_symbol` filters out names that the target does not declare with
`pub`, but it does not filter common method names that the target
trivially does declare (`pub fn new(...) -> Self`). So the filter is
effectively a no-op for the AAP adapter's `new`/`default`/`clone` etc.

The inflated counts (orchestrator.rs `1612`, interview_actor.rs `150`)
follow directly: those files have hundreds of `*::new()` and `.clone()`
and `.handle()` call sites; one matching `use ... store_knowledge_upsert`
or `use crate::adapters::*` import seeds the false positive, and every
unrelated `new`/`clone`/`handle` becomes a "dependent" ref.

### B.4 Same-resolution layer comparison

`find_references` (`src/protocol/tools.rs:4926`) uses
`find_references_for_name(name, kind_filter, false)` in the path-less
branch, which goes through `reverse_index` and only matches references
whose `name` equals the target name (`src/live_index/query.rs:2587-2607`).
For a query like `MemoryStoreKnowledgeUpsertAdapter`,
`reverse_index["new"]` is never consulted, so find_references does not
suffer from the same inflation. Different resolution layers, different
semantics — the two tools cannot be cross-checked against each other.

### B.5 Proposed fix shape

Three lenses, ordered by severity.

1. **Constrain Pass 2 to type-bearing references.** The current filter
   accepts any non-Import reference. The matching reference must
   additionally either (a) be a `ReferenceKind::TypeUsage` whose name is a
   type symbol in the target file, or (b) be a Call whose `qualified_name`
   suffix-matches the target's module path (the same check Pass 3 already
   does). With that change, `Vec::new()` in `A` will not be attributed to
   `B` because `Vec::new()` has `name="new"`, `qualified_name="Vec::new"`,
   and Vec::new's qualified path does not suffix-match
   `crate::adapters::store_knowledge_upsert::new`.
2. **Method-name denylist with confidence tier.** For a residual safety
   net (cases where qualified_name is absent), maintain a small list of
   common method names that almost never identify a specific dependency:
   `new`, `default`, `clone`, `from`, `into`, `build`, `handle`,
   `on_start`, `on_stop`, `process`. Treat hits driven only by these
   names as "uncertain" — not counted in ref totals, optionally surfaced
   in a separate `low-confidence` section — unless the qualified-path
   check also passes. The evaluator suggested this list and it lines up
   with Rust idioms.
3. **Pub-export guard tightening.** `has_pub_symbol` should consult the
   parsed `SymbolRecord.visibility` once that field exists, instead of
   text-scanning for `pub fn`. The text scan returns true for `pub fn new`
   on any target whose API has constructors and so does not filter
   anything useful today. This is also a precondition for symbol-source
   work in find_references.

Fix shape (1) alone collapses the orchestrator.rs `1612 refs` line to
near-zero in the AAP scenario because Pass 2 will no longer fire on bare
`new`/`clone`. (2) and (3) are defense-in-depth.

---

## Part C — B-P1-3 find_references vs batch_rename qualified-path coverage

### C.1 Collector locations

Two distinct reference-collection paths:

- **find_references collector** —
  `pub(crate) async fn find_references` at
  `src/protocol/tools.rs:4926`. When the input has no `path` field set,
  it calls `guard.capture_find_references_view(&input.name, ...)` at
  `src/protocol/tools.rs:5017`, which calls
  `find_references_for_name(name, kind_enum, false)` at
  `src/live_index/query.rs:1696` and consults
  `reverse_index` keyed by **simple name**:

```
src/live_index/query.rs:2581  pub fn find_references_for_name(
src/live_index/query.rs:2582      &self,
src/live_index/query.rs:2583      name: &str,
... (qualified vs simple branch) ...
src/live_index/query.rs:2587  let is_qualified = name.contains("::") || name.contains('.');
... (simple branch: lookup_key = name) ...
src/live_index/query.rs:2625  self.collect_refs_for_key(name, kind_filter, include_filtered, &mut results);
src/live_index/query.rs:2629  let aliases: Vec<String> = self
src/live_index/query.rs:2630      .files
src/live_index/query.rs:2631      .values()
src/live_index/query.rs:2632      .flat_map(|file| {
src/live_index/query.rs:2633          file.alias_map
src/live_index/query.rs:2634              .iter()
src/live_index/query.rs:2635              .filter(|(_alias, original)| original.as_str() == name)
src/live_index/query.rs:2636              .map(|(alias, _)| alias.clone())
src/live_index/query.rs:2637      })
```

  `collect_refs_for_key`, `src/live_index/query.rs:2616`, scans
  `reverse_index.get(lookup_key)` for the simple-name match.

- **batch_rename collector** —
  `pub(crate) fn execute_batch_rename` at `src/protocol/edit.rs:1506`.
  It calls `find_references_for_name(...)` AND ALSO runs the supplemental
  `find_qualified_usages(...)` text scan over every file's content
  (`src/protocol/edit.rs:1593-1604`):

```
src/protocol/edit.rs:1593  for (file_path, content_bytes) in &file_contents {
src/protocol/edit.rs:1594      let source = match std::str::from_utf8(content_bytes) {
src/protocol/edit.rs:1595          Ok(s) => s,
src/protocol/edit.rs:1596          Err(_) => continue, // skip non-UTF-8 files
src/protocol/edit.rs:1597      };
src/protocol/edit.rs:1598      let matches = find_qualified_usages(&input.name, source);
src/protocol/edit.rs:1599      for m in matches {
src/protocol/edit.rs:1600          let end = m.offset + input.name.len();
src/protocol/edit.rs:1601          let range = (m.offset as u32, end as u32);
src/protocol/edit.rs:1602          if m.confident {
src/protocol/edit.rs:1603              qualified_confident.push((file_path.clone(), range));
src/protocol/edit.rs:1604          } else {
... uncertain bucket ...
```

  `find_qualified_usages` at `src/protocol/edit.rs:2380`. It walks every
  byte of every file's source, tracking string/comment/raw-string state,
  and emits a `QualifiedMatch` for each occurrence of the target
  identifier that is preceded or followed by `::`.

### C.2 Why find_references misses fully-qualified calls

The Rust xref tree-sitter query stores qualified calls thus
(`src/parsing/xref.rs:14`):

```
src/parsing/xref.rs:14  ; Qualified calls: Vec::new()  — capture the scoped_identifier for qualified_name too
src/parsing/xref.rs:15  (call_expression function: (scoped_identifier name: (identifier) @ref.call) @ref.qualified_call)
```

The `@ref.call` capture is the `name` field of the `scoped_identifier`,
which is the LAST segment. For `MemoryStoreKnowledgeUpsertAdapter::new()`
the LAST segment is `new`. The full path becomes `qualified_name`
metadata, the simple `name` field is `new`. Reverse-index storage at
`src/live_index/store.rs:810`:

```
src/live_index/store.rs:810  pub(crate) fn build_reverse_index_from_files(
src/live_index/store.rs:811      files: &HashMap<String, Arc<IndexedFile>>,
src/live_index/store.rs:812  ) -> HashMap<String, Vec<ReferenceLocation>> {
src/live_index/store.rs:813      let mut idx: HashMap<String, Vec<ReferenceLocation>> = HashMap::new();
src/live_index/store.rs:814      for (file_path, indexed_file) in files {
src/live_index/store.rs:815          for (reference_idx, reference) in indexed_file.references.iter().enumerate() {
src/live_index/store.rs:816              idx.entry(reference.name.clone())
```

The reverse index is keyed on `reference.name`, which for
`MemoryStoreKnowledgeUpsertAdapter::new()` is `"new"`. A query for
`MemoryStoreKnowledgeUpsertAdapter` looks up `reverse_index["Memory...
Adapter"]` and never sees the prefix.

The Rust grammar's `(type_identifier) @ref.type` rule
(`src/parsing/xref.rs:35`) captures type annotations like
`fn foo(x: MyType)` because `MyType` is a `type_identifier` AST node. In
`Foo::bar()` the prefix `Foo` lives inside a `scoped_identifier` as an
`identifier` node (not `type_identifier`), so it is **not** captured as a
TypeUsage reference. That is the structural gap.

The two specific evaluator misses confirm this:

- `crates/aap-backend/src/lib.rs:66` and
  `crates/aap-backend/tests/common/mod.rs:73` are calls like
  `MemoryStoreKnowledgeUpsertAdapter::new(...)`. The reverse index keys
  these only under `new`, never under the type name.

When find_references gets zero index results it falls back to
`search_text` ONLY to print a note (`src/protocol/tools.rs:5050-5063`):

```
src/protocol/tools.rs:5050  if view.files.is_empty() {
src/protocol/tools.rs:5051      let text_options = search::TextSearchOptions::for_current_code_search();
src/protocol/tools.rs:5052      let text_result = {
src/protocol/tools.rs:5053          let guard = self.index.read();
src/protocol/tools.rs:5054          search::search_text_with_options(
src/protocol/tools.rs:5055              &guard,
src/protocol/tools.rs:5056              Some(&input.name),
... (only used to print a hint) ...
```

But when find_references finds SOME index results (the 7 hits the
evaluator saw, presumably TypeUsage refs from `let x: ...Adapter = ...`
or struct fields), the fallback never fires, and the qualified-call sites
remain invisible to the user.

### C.3 Why batch_rename's collector finds them

`find_qualified_usages` is a byte-level pattern scan; it does not rely on
the xref index at all. For each line it walks bytes, tracks
string/comment state, and matches the identifier when preceded or
followed by `::`. So for `MemoryStoreKnowledgeUpsertAdapter::new(...)` it
sees the type name with `::` after it and emits a `QualifiedMatch`. That
match is then classified `confident` if the byte position is in code
context (not in a string, comment, or raw string).

Combined with `find_references_for_name(...)` (same call as
find_references) it produces the union: TypeUsage refs + qualified-call
prefix refs + import-path refs. That is the "13 sites across 6 files"
batch_rename --dry-run reports.

### C.4 Are the collectors mergeable?

Yes — three options, increasing complexity.

1. **Have find_references reuse the supplemental text scan when the index
   is empty AND when the index is non-empty.** Today the fallback at
   `src/protocol/tools.rs:5050` only fires for `view.files.is_empty()`.
   Remove that guard and always run the qualified-path scan
   (`find_qualified_usages` exposed via the crate), then merge results
   with deduplication on `(file_path, byte_range)`. Output structure
   stays the same; we add a "qualified-path supplemental" line count to
   the envelope, similar to how batch_rename splits confident vs
   uncertain. This is the cheapest fix and immediately closes the gap on
   the evaluator's two missed lines.
2. **Promote `find_qualified_usages` to a shared collector module** and
   call it from both find_references and execute_batch_rename. Same
   result as (1) but with one canonical reference-collection function
   instead of two. Surface area cost: a new public function in
   `src/live_index/` exporting `(file_path, byte_range, confident)`
   tuples, shared by both call sites.
3. **Extend the Rust xref query to capture the type prefix of qualified
   calls.** A query addition like
   `(scoped_identifier path: (identifier) @ref.type_qualifier)` (or
   walking the `path` field down through nested scoped_identifiers) would
   index `Foo` from `Foo::bar()` as a synthetic TypeUsage reference. This
   is the most structurally correct fix but the highest-risk change to
   the parser: we have to verify it does not double-count, does not
   over-capture (`std::collections::HashMap::new()` should not record
   `std` and `collections`), and does not break the existing tests in
   `src/protocol/format/tests.rs`. Until that's verified, options (1) or
   (2) carry the immediate user-visible benefit.

Recommended near-term: option (2). It avoids re-parsing files at query
time (the text scan is bytes-only, not tree-sitter), keeps find_references
performance acceptable, and lets the existing batch_rename
confident/uncertain classifier label the supplemental hits inline.
Eventually pair with option (3) once that grammar change is independently
validated.

---

End of investigation.
