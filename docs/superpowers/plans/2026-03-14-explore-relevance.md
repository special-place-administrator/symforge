# Explore Semantic Relevance Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `explore` project-aware with module-path boosting, concept+remainder merging, and expanded concept coverage.

**Architecture:** Three changes to two files: (1) `explore.rs` — expand CONCEPT_MAP, change `match_concept` to return matched key, add word-boundary matching. (2) `tools.rs` — add Phase 0 module-path boosting, implement concept+remainder merging with stopword filtering.

**Tech Stack:** Rust

**Spec:** `docs/superpowers/specs/2026-03-14-explore-relevance-design.md`

---

## Task 1: Expand CONCEPT_MAP and fix `match_concept`

**Files:**
- Modify: `src/protocol/explore.rs`

- [ ] **Step 1: Change `match_concept` signature and use word-boundary matching**

Current `match_concept` (lines 114-120) uses `lower.contains(key)`. Replace with word-boundary matching that returns the matched key:

```rust
/// Find the best matching concept for a query.
/// Returns the matched key and pattern, using word-boundary matching
/// (concept key words must appear as contiguous subsequence of query words).
pub fn match_concept(query: &str) -> Option<(&'static str, &'static ConceptPattern)> {
    let query_words: Vec<&str> = query.split_whitespace().collect();
    CONCEPT_MAP
        .iter()
        .find(|(key, _)| {
            let key_words: Vec<&str> = key.split_whitespace().collect();
            query_words
                .windows(key_words.len())
                .any(|window| {
                    window.iter().zip(key_words.iter()).all(|(qw, kw)| {
                        qw.eq_ignore_ascii_case(kw)
                    })
                })
        })
        .map(|(key, pattern)| (*key, pattern))
}
```

- [ ] **Step 2: Sort CONCEPT_MAP by key length descending**

Reorder the existing 7 entries so longer keys match first. "error handling" (14 chars) before "api" (3 chars). Then add the ~10 new entries (also sorted by length descending within the additions).

- [ ] **Step 3: Add new concept entries**

Add after the existing entries (maintaining length-descending order across all entries):

```rust
("file watching", ConceptPattern {
    label: "File Watching",
    symbol_queries: &["watcher", "notify", "debounce", "event"],
    text_queries: &["notify::Event", "DebouncedEvent", "file_event", "inotify"],
    kind_filters: &[],
}),
("serialization", ConceptPattern {
    label: "Serialization",
    symbol_queries: &["serialize", "deserialize", "serde", "json", "postcard"],
    text_queries: &["#[derive(Serialize", "#[derive(Deserialize", "serde_json", "postcard::"],
    kind_filters: &[],
}),
("permissions", ConceptPattern {
    label: "Permissions / Authorization",
    symbol_queries: &["permission", "role", "policy", "acl", "authorize"],
    text_queries: &["forbidden", "unauthorized", "access_control", "RBAC"],
    kind_filters: &[],
}),
("deployment", ConceptPattern {
    label: "Deployment / Release",
    symbol_queries: &["release", "deploy", "version", "publish", "migrate"],
    text_queries: &["npm publish", "cargo publish", "release-please", "changelog"],
    kind_filters: &[],
}),
("networking", ConceptPattern {
    label: "Networking",
    symbol_queries: &["socket", "listener", "bind", "connect", "server"],
    text_queries: &["TcpListener", "hyper", "axum", "reqwest", "tonic"],
    kind_filters: &[],
}),
("indexing", ConceptPattern {
    label: "Indexing",
    symbol_queries: &["index", "reindex", "snapshot", "persist"],
    text_queries: &["LiveIndex", "index.bin", "reindex", "rebuild_reverse"],
    kind_filters: &[],
}),
("parsing", ConceptPattern {
    label: "Parsing",
    symbol_queries: &["parse", "parser", "ast", "node", "tree_sitter"],
    text_queries: &["tree_sitter::", ".parse(", "syntax tree", "grammar"],
    kind_filters: &[],
}),
("caching", ConceptPattern {
    label: "Caching",
    symbol_queries: &["cache", "lru", "memoize", "ttl", "expire"],
    text_queries: &["LruCache", "cache.get(", "cached::", "moka::"],
    kind_filters: &[],
}),
("logging", ConceptPattern {
    label: "Logging / Observability",
    symbol_queries: &["log", "trace", "span", "metric", "telemetry"],
    text_queries: &["tracing::", "log::", "debug!", "warn!", "info!"],
    kind_filters: &[],
}),
("cli", ConceptPattern {
    label: "CLI / Command Line",
    symbol_queries: &["cli", "args", "command", "subcommand"],
    text_queries: &["clap", "structopt", "Arg::", "Command::new"],
    kind_filters: &[],
}),
// Aliases — point to same-shaped patterns
("watcher", ConceptPattern {
    label: "File Watching",
    symbol_queries: &["watcher", "notify", "debounce", "event"],
    text_queries: &["notify::Event", "DebouncedEvent", "file_event", "inotify"],
    kind_filters: &[],
}),
("parser", ConceptPattern {
    label: "Parsing",
    symbol_queries: &["parse", "parser", "ast", "node", "tree_sitter"],
    text_queries: &["tree_sitter::", ".parse(", "syntax tree", "grammar"],
    kind_filters: &[],
}),
```

- [ ] **Step 4: Update existing tests**

The 3 tests in `explore.rs` that call `match_concept` need to destructure the new return type:

```rust
// Old:
let concept = match_concept("error handling");
assert!(concept.is_some());
assert_eq!(concept.unwrap().label, "Error Handling");

// New:
let result = match_concept("error handling");
assert!(result.is_some());
let (key, concept) = result.unwrap();
assert_eq!(key, "error handling");
assert_eq!(concept.label, "Error Handling");
```

- [ ] **Step 5: Add word-boundary test**

```rust
#[test]
fn test_match_concept_word_boundary_no_substring() {
    // "clinical" contains "cli" but should NOT match the "cli" concept
    assert!(match_concept("clinical trial data").is_none());
    // "capital" contains "api" but should NOT match
    assert!(match_concept("capital investment").is_none());
    // But exact word matches should work
    assert!(match_concept("cli tools").is_some());
    assert!(match_concept("api endpoints").is_some());
}
```

- [ ] **Step 6: Verify and commit**

Run: `cargo test --all-targets -- --test-threads=1`
Run: `cargo fmt -- --check`

```
feat: expand CONCEPT_MAP and add word-boundary matching

Add 10 new concept entries (file watching, parsing, serialization,
indexing, logging, cli, networking, caching, permissions, deployment)
plus 2 aliases. Sort by key length descending for priority matching.
Change match_concept to use word-boundary matching (split_whitespace
subsequence check) and return matched key for remainder computation.
```

---

## Task 2: Module-path boosting (Phase 0) in explore handler

**Files:**
- Modify: `src/protocol/tools.rs` (`explore` handler, ~line 2084)

- [ ] **Step 1: Add Phase 0 module-path boosting**

In the `explore` method, after computing `symbol_queries` and `text_queries` but before Phase 1, add Phase 0. The terms for Phase 0 are: for fallback queries, all terms; for concept matches, the remainder terms (computed in Task 3). For now, use all terms — Task 3 will refine this.

Insert before `// Phase 1: Symbol search`:

```rust
// Phase 0: Module-path boosting — symbols from files whose path matches
// query terms get a weight boost. Runs unconditionally (concept + fallback).
let boost_terms = &symbol_queries; // refined to remainder in Task 3
for term in boost_terms {
    let term_lower = term.to_ascii_lowercase();
    for (file_path, file) in guard.all_files() {
        let segments: Vec<&str> = file_path.split(&['/', '\\'][..]).collect();
        let best_match = segments.iter().filter_map(|seg| {
            let seg_lower = seg.to_ascii_lowercase();
            if seg_lower == term_lower {
                Some(2usize) // exact segment match
            } else if seg_lower.contains(&term_lower) {
                Some(1usize) // substring segment match
            } else {
                None
            }
        }).max();
        if let Some(weight) = best_match {
            let mut injected = 0;
            for sym in &file.symbols {
                if injected >= limit {
                    break; // per-directory cap
                }
                let entry = (sym.name.clone(), sym.kind.to_string(), file_path.clone());
                *match_counts.entry(entry).or_default() += weight;
                injected += 1;
            }
        }
    }
}
```

Note: `match_counts` must be initialized BEFORE Phase 0 (it already is, from our earlier scoring fix). `guard.all_files()` is available since we're inside the `guard` scope.

- [ ] **Step 2: Write module-boosting test**

Add to the test module in `src/protocol/tools.rs`:

```rust
#[tokio::test]
async fn test_explore_module_path_boosting() {
    // File path contains "watcher" → symbols should be module-boosted
    let content = b"pub struct WatcherInfo {\n    debounce_ms: u64,\n}\n";
    let watcher_sym = SymbolRecord {
        name: "WatcherInfo".to_string(),
        kind: SymbolKind::Struct,
        depth: 0,
        sort_order: 0,
        byte_range: (0, content.len() as u32),
        line_range: (0, 2),
        doc_byte_range: None,
    };
    let (key, file) = make_file("src/watcher/mod.rs", content, vec![watcher_sym]);
    let server = make_server(make_live_index_ready(vec![(key, file)]));
    let result = server
        .explore(Parameters(ExploreInput {
            query: "watcher".to_string(),
            limit: Some(10),
            depth: None,
        }))
        .await;
    assert!(
        result.contains("WatcherInfo"),
        "WatcherInfo should appear via module-path boosting: {result}"
    );
}
```

- [ ] **Step 3: Verify and commit**

Run: `cargo test --all-targets -- --test-threads=1`
Run: `cargo fmt -- --check`

```
feat: add module-path boosting to explore (Phase 0)

Symbols from files whose path segment matches a query term get
a weight boost (+2 exact, +1 substring). Per-directory cap of
limit symbols prevents flooding from large directories.
```

---

## Task 3: Concept + remainder merging

**Files:**
- Modify: `src/protocol/tools.rs` (`explore` handler)

- [ ] **Step 1: Add stopword set and remainder computation**

Near the top of `tools.rs` (or inside the `explore` method), add the stopword filter:

```rust
fn compute_remainder_terms(query: &str, concept_key: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "a", "an", "the", "in", "on", "of", "for", "to", "and", "or",
        "is", "it", "my", "at", "by", "do", "no", "so", "up", "if",
        "with", "from", "this", "that",
    ];
    let key_words: Vec<&str> = concept_key.split_whitespace().collect();
    query
        .split_whitespace()
        .filter(|w| {
            let lower = w.to_ascii_lowercase();
            !key_words.iter().any(|kw| kw.eq_ignore_ascii_case(w))
                && !STOPWORDS.contains(&lower.as_str())
                && lower.len() >= 3
        })
        .map(|w| w.to_ascii_lowercase())
        .collect()
}
```

- [ ] **Step 2: Restructure the explore handler for concept+remainder**

The current code has two branches — concept match and fallback. Restructure to:

```rust
let concept = super::explore::match_concept(&params.0.query);

let (label, symbol_queries, text_queries, remainder_terms): (String, Vec<String>, Vec<String>, Vec<String>) =
    if let Some((key, c)) = concept {
        let remainder = compute_remainder_terms(&params.0.query, key);
        (
            c.label.to_string(),
            c.symbol_queries.iter().map(|s| s.to_string()).collect(),
            c.text_queries.iter().map(|s| s.to_string()).collect(),
            remainder,
        )
    } else {
        let terms = super::explore::fallback_terms(&params.0.query);
        if terms.is_empty() {
            return "Explore requires a non-empty query.".to_string();
        }
        (format!("'{}'", params.0.query), terms.clone(), terms.clone(), vec![])
    };
```

- [ ] **Step 3: Update Phase 0 to use appropriate terms**

Change Phase 0's boost terms to use remainder for concept matches, all terms for fallback:

```rust
// Phase 0: Module-path boosting
let boost_terms = if remainder_terms.is_empty() {
    &symbol_queries
} else {
    &remainder_terms
};
```

For concept matches with no remainder (e.g., query is exactly "error handling"), Phase 0 is skipped (no terms to boost). For concept matches with remainder (e.g., "error handling in the watcher"), Phase 0 boosts on remainder terms ("watcher").

- [ ] **Step 4: Add remainder terms to Phase 1 and Phase 2**

After the concept's symbol/text queries run through Phases 1-2, also run the remainder terms:

```rust
// After concept queries run through Phase 1:
for sq in &remainder_terms {
    let result = search::search_symbols(&guard, sq, None, limit * 3);
    for hit in &result.hits {
        let entry = (hit.name.clone(), hit.kind.clone(), hit.path.clone());
        *match_counts.entry(entry).or_default() += 1;
    }
}

// After concept queries run through Phase 2 text search loop:
for tq in &remainder_terms {
    // ... same text search + enclosing symbol injection as existing Phase 2
}
```

The cleanest way: merge `symbol_queries` + `remainder_terms` into the Phase 1 loop, and `text_queries` + `remainder_terms` into the Phase 2 loop. Since remainder_terms is empty for fallback queries, this is a no-op in that case.

- [ ] **Step 5: Write concept+remainder test**

```rust
#[tokio::test]
async fn test_explore_concept_plus_remainder() {
    // "error handling" concept should match, "watcher" should be remainder
    // Create a file in src/watcher/ with an error-related symbol
    let content = b"pub fn handle_watcher_error() {}\n";
    let sym = SymbolRecord {
        name: "handle_watcher_error".to_string(),
        kind: SymbolKind::Function,
        depth: 0,
        sort_order: 0,
        byte_range: (0, content.len() as u32),
        line_range: (0, 0),
        doc_byte_range: None,
    };
    let (key, file) = make_file("src/watcher/errors.rs", content, vec![sym]);
    let server = make_server(make_live_index_ready(vec![(key, file)]));
    let result = server
        .explore(Parameters(ExploreInput {
            query: "error handling in the watcher".to_string(),
            limit: Some(10),
            depth: None,
        }))
        .await;
    // Should find the watcher error handler via concept + remainder merging
    assert!(
        result.contains("handle_watcher_error") || result.contains("watcher"),
        "concept+remainder should surface watcher-related results: {result}"
    );
}
```

- [ ] **Step 6: Write exact-concept-no-remainder test**

```rust
#[tokio::test]
async fn test_explore_exact_concept_no_remainder() {
    // Query is exactly a concept key — no remainder, no module boosting
    let content = b"pub enum SymForgeError {}\n";
    let sym = SymbolRecord {
        name: "SymForgeError".to_string(),
        kind: SymbolKind::Enum,
        depth: 0,
        sort_order: 0,
        byte_range: (0, content.len() as u32),
        line_range: (0, 0),
        doc_byte_range: None,
    };
    let (key, file) = make_file("src/error.rs", content, vec![sym]);
    let server = make_server(make_live_index_ready(vec![(key, file)]));
    let result = server
        .explore(Parameters(ExploreInput {
            query: "error handling".to_string(),
            limit: Some(10),
            depth: None,
        }))
        .await;
    // Should use concept queries (symbol_queries includes "Error")
    assert!(
        result.contains("SymForgeError"),
        "exact concept match should find error-related symbols: {result}"
    );
}
```

- [ ] **Step 7: Verify and commit**

Run: `cargo test --all-targets -- --test-threads=1`
Run: `cargo fmt -- --check`

```
feat: concept+remainder merging in explore

When a concept matches a compound query, extract remainder terms
and run them through the full fallback path (module boosting +
symbol search + text search) alongside the concept's curated
queries. Stopword filter removes noise words from remainder.
```

---

## Dependency Graph

```
Task 1 (explore.rs: CONCEPT_MAP + match_concept)
  └─► Task 2 (tools.rs: Phase 0 module boosting)
        └─► Task 3 (tools.rs: concept+remainder merging)
```

Sequential — each task depends on the previous. Task 1 changes the `match_concept` API that Tasks 2-3 consume. Task 2 adds the module boosting infrastructure that Task 3 refines with remainder terms.
