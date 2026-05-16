---
title: RTK OnceLock Audit
type: audit
status: complete
date: 2026-05-16
roadmap_unit: "Wave 3a / Unit 3a.1"
---

# RTK OnceLock Audit

## Scope

Roadmap Unit 3a.1 asks for a research-only audit of hot-path initialization in
`src/` that could migrate from per-call initialization to `OnceLock`. No source
code was patched.

Evidence used:

- SymForge sidecar reindexed to `C:\AI_STUFF\PROGRAMMING\symforge` and returned
  378 files / 12,475 symbols.
- `rg "OnceLock|Lazy<|LazyLock|lazy_static" src/`
- Focused `rg` searches for `RegexBuilder::new`, `Regex::new`,
  `GlobBuilder::new`, `Parser::new`, `Pattern::try_new`,
  `FrecencyStore::open`, `open_existing_readonly`, `feature_flag_enabled`, and
  `extractor_for`.
- Targeted SymForge context reads and reference lookups for the candidate sites.
- ReminDB / agentmemory follow-up for prior SymForge frecency and RTK notes.
  A vault todo still describes an older `search_files` / `search_text` /
  `search_symbols` frecency DB-open violation. Current `rg` evidence in `src/`
  narrows that concern: only `search_files` uses `open_existing_readonly` and
  `health` uses `FrecencyStore::open` on read-side paths; no current
  `search_text` or `search_symbols` store-open path was found.
- The `SymForge Co-Change Signal Fusion` vault note was checked. Current Wave 2
  shape is `search_files(rank_by="path+cochange", anchor_path=...)`; current
  code reads coupling evidence through `LiveIndex::coupling_store()` rather than
  opening the coupling DB per query. Rule 5 remains provisional, but that is a
  calibration issue, not a `OnceLock` migration issue.

## Existing Conventions

The repo already uses process-wide one-time initialization where the initialized
value is static, reusable, and safe for process lifetime:

- `src/parsing/xref.rs:359-374` stores one `OnceLock<Query>` per xref language.
  This is the clearest current pattern: expensive `tree_sitter::Query`
  compilation is language-static and reused across every parse.
- `src/live_index/frecency.rs:398-414` uses
  `OnceLock<Mutex<HashMap<PathBuf, Arc<FrecencyStore>>>>` for per-workspace
  write-capable frecency stores.
- `src/live_index/store.rs:30-79` uses a `OnceLock<rayon::ThreadPool>` for the
  indexing pool and folds the Windows stack-size env var into first
  initialization.
- `src/live_index/coupling/lifecycle.rs:30-40` uses `OnceLock` for the
  per-workspace in-flight guard. Co-change read paths reuse the store carried by
  `LiveIndex`, so the Wave 2 ranker path did not produce a new per-call DB-open
  candidate.
- `src/live_index/rank_signals.rs:232-238` and
  `src/protocol/edit_hooks.rs:82-84` use `OnceLock<RwLock<Vec<Box<...>>>>` for
  process-wide registries.
- `src/protocol/tools.rs:1245-1247` uses `std::sync::LazyLock<regex::Regex>`
  for a static regex. This is equivalent guidance for simple static values;
  prefer std `OnceLock` / `LazyLock` over adding new `lazy_static!` sites.

## Candidate Sites

### 1. Structural search pattern compilation

- Site: `src/live_index/search.rs:1064-1145`
- Init pattern: `search_structural` loops over every candidate file and calls
  `crate::parsing::ast_grep::structural_search(&content_str, pattern, lang)`.
- Underlying compile: `src/parsing/ast_grep.rs:142-153` constructs `SgLang` and
  calls `Pattern::try_new(pattern_str, sg_lang.clone())`.
- Hot-path assessment: hot for `search_text(structural=true)`, especially on
  broad scopes. The same `(pattern, language)` is recompiled for every file of
  that language in the candidate set.
- Migration recommendation: highest-priority candidate. Compile once per
  `(LanguageId, pattern)` per request first, then consider a process-wide
  `OnceLock<Mutex<HashMap<PatternKey, CachedPattern>>>` only if the
  `ast_grep_core::Pattern` type is `Send + Sync` enough for a global cache.
- Risk / reason to defer source patch: `Pattern` ownership and trait bounds must
  be verified. A global cache also needs bounded growth or explicit invalidation
  because patterns are user input.

### 2. Frecency store opens from read paths

- Sites:
  - `src/protocol/tools.rs:4549-4582` opens a read-only frecency store for
    `search_files(rank_by="frecency")`.
  - `src/protocol/tools.rs:4694-4710` opens the frecency store while rendering
    `health` diagnostics when `SYMFORGE_FRECENCY=1`.
- Existing nearby cache: `src/live_index/frecency.rs:398-414` caches
  write-capable stores per workspace with `OnceLock<Mutex<HashMap<...>>>`.
- Hot-path assessment: medium. `search_files` is an interactive discovery tool
  and can be called repeatedly; `health` is diagnostic but also called often
  during dogfood. Both currently pay SQLite open costs outside the existing
  same-process cache path.
- Migration recommendation: add a public cached read-only or shared-read helper
  in `src/live_index/frecency.rs`, probably using the existing
  `OnceLock<Mutex<HashMap<PathBuf, Arc<FrecencyStore>>>>` pattern. Preserve
  the discovery invariant from `open_existing_readonly`: no parent directory,
  DB file, schema, or frecency footprint should be created by search.
- Risk / reason to defer source patch: the write cache applies HEAD reset policy
  on first open. Search/health must not accidentally trigger commitment-side
  policy or create a DB. This needs a focused design before implementation.

### 3. Config extractor construction

- Sites:
  - `src/parsing/mod.rs:49-52` calls `config_extractors::extractor_for` for
    config files.
  - `src/parsing/config_extractors/mod.rs:72-81` returns a new boxed stateless
    extractor for JSON, TOML, YAML, Markdown, or Env.
  - `src/parsing/config_extractors/mod.rs:83-86` reuses the same boxed path for
    edit-capability lookup.
- Hot-path assessment: medium-low. Config files are part of indexing, and the
  current path allocates a box per config extraction / capability lookup, but
  each extractor is currently a stateless zero-sized type and likely cheap.
- Migration recommendation: consider replacing `Option<Box<dyn
  ConfigExtractor>>` with `Option<&'static dyn ConfigExtractor>` backed by
  static values or `OnceLock` only if profiling shows measurable allocation
  cost. This is more of an API cleanup than an urgent hot-path win.
- Risk / reason to defer source patch: changing the trait object lifetime will
  touch config extractor APIs and tests. The perf payoff may be too small.

### 4. Worktree feature flag checks

- Sites:
  - `src/worktree.rs:339-343` reads `SYMFORGE_WORKTREE_AWARE` on every
    `feature_flag_enabled()` call and explicitly says callers should cache if
    hot-path sensitive.
  - `src/protocol/mod.rs:170-179` calls it from
    `note_worktree_misuse_if_flag_on` when edit handlers omit
    `working_directory`.
  - `src/worktree.rs:346-356` already uses `OnceLock<()>` to make hook
    registration one-shot.
- Hot-path assessment: low to medium. It is in the edit-tool path, but only the
  missing-`working_directory` diagnostic path currently reads it per call.
- Migration recommendation: if edit-path telemetry shows this is hot, cache the
  env flag with `OnceLock<bool>` or move the cached decision into server startup
  state. Keep the existing `REGISTERED: OnceLock<()>` convention for hook
  registration.
- Risk / reason to defer source patch: some tests and dogfood workflows flip
  env vars inside one process. `src/live_index/persist.rs:374-383` deliberately
  avoids caching `SYMFORGE_FRECENCY` so tests can enable it after boot; apply the
  same caution before caching worktree env behavior.

### 5. Tree-sitter parser construction during indexing

- Site: `src/parsing/mod.rs:191-250`
- Init pattern: `parse_source` creates a fresh `tree_sitter::Parser`, maps
  `LanguageId` to a tree-sitter language, sets the language, parses, then
  extracts symbols and xrefs.
- Callers: SymForge found `process_file_with_classification` at
  `src/parsing/mod.rs:89`, `extract_symbols_for_diff` at
  `src/parsing/mod.rs:263`, and symbol-resolution work in
  `src/live_index/coupling/walker.rs:486`.
- Hot-path assessment: hot. This runs for source files during indexing and
  diff-oriented parsing.
- Migration recommendation: do not use a simple global `OnceLock<Parser>`.
  `Parser` is mutable and language-specific at use time. If benchmarks show
  parser construction matters, investigate a thread-local parser cache or a
  small parser pool keyed by `LanguageId`; `OnceLock` may only be useful for
  initializing that pool.
- Risk / reason to defer source patch: parser reuse can introduce hidden state,
  locking, or cross-language mistakes. The current per-call parser is
  deterministic and straightforward.

### 6. Text regex, whole-word regex, Aho-Corasick, and glob compilation

- Sites:
  - `src/live_index/search.rs:921-957` compiles user-provided regex patterns.
  - `src/live_index/search.rs:982-997` compiles the whole-word regex or
    multi-term Aho-Corasick automaton for literal searches.
  - `src/live_index/search.rs:1258-1285` compiles include/exclude text globs.
  - `src/live_index/query.rs:1500-1515` compiles glob path queries for
    `search_files`.
- Hot-path assessment: hot for repeated searches, but all of these are driven
  by user-provided query text or options.
- Migration recommendation: do not migrate directly to unbounded process-wide
  `OnceLock` singletons. If performance data justifies it, design a bounded
  query-cache keyed by pattern/options. `OnceLock` could initialize the cache
  container, not the compiled matcher itself.
- Risk / reason to defer source patch: user input as cache key creates unbounded
  memory risk, and invalid-pattern error behavior must stay exact.

### 7. Ast-grep language wrapper construction

- Site: `src/parsing/ast_grep.rs:55-121`
- Init pattern: `SgLang::from_language_id` rebuilds a small wrapper around the
  tree-sitter language and expando char for each structural search call.
- Hot-path assessment: hot only because it sits under structural search, but it
  is cheap relative to `Pattern::try_new` and the AST walk.
- Migration recommendation: treat as part of Candidate 1, not as a standalone
  follow-up. If pattern caching is implemented, the language wrapper may become
  cached as a side effect.
- Risk / reason to defer source patch: premature standalone caching adds
  indirection without addressing the expensive compile.

## Recommended Migration Order

1. `search_structural` / `ast_grep::structural_search` pattern reuse. First
   prove request-local compile-once-per-language behavior; only promote to a
   process-wide `OnceLock` cache after trait bounds and cache-size policy are
   clear.
2. Frecency read-path store opens. Add a cached read-only/shared helper that
   preserves the discovery-no-footprint contract.
3. Config extractor registry cleanup. Convert boxed stateless extractors to
   static references only if a microbenchmark or allocation profile shows value.
4. Worktree feature flag caching. Keep low priority unless edit-path telemetry
   shows env lookup cost or noise.
5. Parser reuse investigation. This is a real hot path but probably not a plain
   `OnceLock` migration; handle as a separate parser-pool design if needed.
6. Dynamic regex/glob/AC matcher caching. Defer until there is repeated-query
   evidence and a bounded cache design.

## Explicit Deferrals

- Do not add `lazy_static!`; new one-time init should use std `OnceLock` or
  `LazyLock`.
- Do not cache user-provided regex/glob/pattern inputs in an unbounded global
  map.
- Do not cache env flags until tests and runtime expectations are audited.
- Do not replace fresh tree-sitter parsers with a single global parser.

## Follow-Up Acceptance Criteria

Any implementation follow-up from this audit should include:

- A before/after benchmark or targeted regression test that proves fewer
  repeated compiles/opens on the selected path.
- Tests preserving invalid regex/glob/structural-pattern error behavior.
- A cache growth policy for any user-input-keyed cache.
- Confirmation that discovery-only tools do not create frecency files.
