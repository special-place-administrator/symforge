# SymForge MCP Feature Feasibility Analysis
**Date**: 2026-03-17  
**Scope**: 3 features for Kilo Code compatibility and search quality

## Executive Summary

All 3 features are **feasible, independent, and low-to-medium effort**. No blockers identified. Recommended implementation order: Feature 1 → Feature 3 → Feature 2 (by urgency and complexity).

| # | Feature | Effort | Lines | Risk | Status |
|---|---------|--------|-------|------|--------|
| 1 | Lenient Vec Deserializer | Small | ~104 | Low | Ready to implement |
| 2 | Semantic Search Ranking | Small-Medium | ~210 | Low-Medium | Ready to implement |
| 3 | Kilo Code Auto-Detection in Init | Small | ~240 | Low | Ready to implement |

## Cross-Feature Dependencies

All 3 features are **fully independent**. They touch different modules with no overlapping symbols. Can be implemented in parallel or any order.

One synergy: If Feature 1 (Lenient Vec) ships first, Feature 2's new `ranked` param should use `lenient_bool` (already available). If Feature 2 adds a new `Vec` param later, it should use the new `lenient_option_vec` from Feature 1.

---

## Feature 1: Lenient Vec Deserializer (Kilo Compatibility Fix)

### Problem
MCP clients like Kilo Code stringify JSON arrays when passing tool parameters. SymForge's serde deserialization rejects these with "expected a sequence". This breaks all batch/array tools (get_symbol targets, batch_edit, batch_insert, search_text terms, get_file_context sections, get_symbol_context sections).

### Affected Files and Symbols

| File | Symbols | Change Type |
|------|---------|-------------|
| `src/protocol/tools.rs` | `lenient_option_vec` (new), `lenient_vec_required` (new) | New deserializer functions |
| `src/protocol/tools.rs` | `GetSymbolInput`, `GetSymbolsInput`, `SearchTextInput`, `GetFileContextInput`, `GetSymbolContextInput`, `TraceSymbolInput` | Add serde annotations |
| `src/protocol/edit.rs` | `BatchEditInput`, `BatchInsertInput` | Add serde annotations |

### Fields Requiring Annotation

| File | Struct | Field | Type | Element Type |
|------|--------|-------|------|-------------|
| `src/protocol/tools.rs:144` | `GetSymbolInput` | `targets` | `Option<Vec<SymbolTarget>>` | `SymbolTarget` |
| `src/protocol/tools.rs:180` | `GetSymbolsInput` | `targets` | `Vec<SymbolTarget>` | `SymbolTarget` |
| `src/protocol/tools.rs:211` | `SearchTextInput` | `terms` | `Option<Vec<String>>` | `String` |
| `src/protocol/tools.rs:457` | `GetFileContextInput` | `sections` | `Option<Vec<String>>` | `String` |
| `src/protocol/tools.rs:470` | `GetSymbolContextInput` | `sections` | `Option<Vec<String>>` | `String` |
| `src/protocol/tools.rs:519` | `TraceSymbolInput` | `sections` | `Option<Vec<String>>` | `String` |
| `src/protocol/edit.rs:622` | `BatchEditInput` | `edits` | `Vec<SingleEdit>` | `SingleEdit` |
| `src/protocol/edit.rs:1250` | `BatchInsertInput` | `targets` | `Vec<InsertTarget>` | `InsertTarget` |

**Total: 8 fields across 2 8 structs.**

### Existing Pattern

The codebase already has 5 lenient deserializers at `src/protocol/tools.rs` lines 26–116 (`lenient_u32`, `lenient_bool`, `lenient_u64`, `lenient_i64`, `lenient_f64`). They all use `#[serde(untagged)]` enum with `NumOrStr` variants, return `Result<Option<T>, D::Error>`, and are applied via `#[serde(default, deserialize_with = "lenient_xxx")]`.

### Implementation

#### `lenient_option_vec` — for `Option<Vec<T>>` fields

```rust
pub(crate) fn lenient_option_vec<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum VecOrStr<T> {
        Vec(Vec<T>),
        Str(String),
        Null,
    }
    match VecOrStr::<T>::deserialize(deserializer)? {
        VecOrStr::Vec(v) => Ok(Some(v)),
        VecOrStr::Str(s) if s.is_empty() || s == "null" => Ok(None),
        VecOrStr::Str(s) => serde_json::from_str::<Vec<T>>(&s)
            .map(Some)
            .map_err(serde::de::Error::custom),
        VecOrStr::Null => Ok(None),
    }
}
```

#### `lenient_vec_required` — for bare `Vec<T>` fields

```rust
pub(crate) fn lenient_vec_required<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum VecOrStr<T> {
        Vec(Vec<T>),
        Str(String),
    }
    match VecOrStr::<T>::deserialize(deserializer)? {
        VecOrStr::Vec(v) => Ok(v),
        VecOrStr::Str(s) => serde_json::from_str::<Vec<T>>(&s)
            .map_err(serde::de::Error::custom),
    }
}
```

#### Annotation Pattern

For `Option<Vec<T>>` fields:
```rust
#[serde(default, deserialize_with = "lenient_option_vec")]
pub sections: Option<Vec<String>>,
```

For bare `Vec<T>` fields:
```rust
#[serde(deserialize_with = "lenient_vec_required")]
pub targets: Vec<SymbolTarget>,
```

For cross-module (`edit.rs` using `tools.rs` helpers):
```rust
#[serde(deserialize_with = "super::tools::lenient_vec_required")]
pub edits: Vec<SingleEdit>,
```

### Tests Required

- `test_lenient_vec_accepts_native_array`
- `test_lenient_vec_accepts_stringified_array`
- `test_lenient_option_vec_accepts_null`
- `test_lenient_option_vec_accepts_empty_string`
- `test_lenient_vec_required_accepts_stringified_complex` (for `SymbolTarget`)
- `test_lenient_vec_required_rejects_invalid_string`

### Effort Estimate

| Component | Lines |
|-----------|-------|
| 2 new functions | ~36 |
| 8 annotation changes | ~8 |
| 6 test functions | ~60 |
| **Total** | **~104** |

### Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| `#[serde(untagged)]` with generic `T` | Low | Standard serde pattern, well-tested |
| Ambiguity for bare strings to `Vec<String>` | Low | `serde_json::from_str("hello")` fails correctly — only stringified JSON arrays accepted |
| Cross-module access from `edit.rs` | Low | Existing pattern already used for `lenient_u32`/`lenient_bool` |
| Performance | Negligible | String path only triggered by Kilo-style clients |

---

## Feature 2: Semantic Search Ranking

### Problem
`search_text` returns results ordered by match count per file, not by semantic importance. A match inside a highly-connected function with 20 callers ranks the same as a match in dead code.

### Affected Files and Symbols

| File | Symbols | Change Type |
|------|---------|-------------|
| `src/live_index/search.rs` | `TextSearchOptions`, `collect_text_matches`, `TextFileMatches`, `TextLineMatch` | Core ranking logic |
| `src/protocol/tools.rs` | `SearchTextInput`, `search_text_options_from_input`, `search_text` handler | New `ranked` param |
| `src/protocol/format.rs` | `search_text_result_view` | Optional: display ranking score |
| `src/live_index/store.rs` | `LiveIndex.reverse_index`, `SharedIndexHandle` | Caller-count lookup exposure |

### Current Behavior

Entry point: `search_text_with_options()` in `search.rs:805`. Delegates to `collect_text_matches()` at line 1007. Current sort: **match-count descending, then alphabetical**. Enclosing symbol data is resolved per match but **not used for ranking**, only for display.

### Ranking Signal Design

**Composite score per file** (opt-in via `ranked: bool` parameter):

```
score = 0.30 × match_count_normalized
      + 0.40 × caller_count_normalized
      + 0.15 × churn_score
      + 0.15 × kind_priority
```

#### Signal Availability

| Signal | Source | Cost | Available? |
|--------|--------|------|------------|
| Caller count | `LiveIndex.reverse_index` HashMap | O(1) per symbol | ✅ Always |
| Git churn | `GitTemporalIndex.files[path].churn_score` | O(1) per file | ✅ When computed |
| Kind priority | `EnclosingMatchSymbol.kind` match arm | Free | ✅ Always |

#### Kind Priority Map

```
Function/Method → 1.0
Class/Struct/Enum/Interface/Trait → 0.8
Impl → 0.7
Module → 0.5
Constant → 0.4
Variable/Type → 0.3
Key/Section/Other → 0.1
(no enclosing symbol) → 0.0
```

#### Caller Count Normalization
- `caller_score = min(1.0, max_caller_count / 20.0)` (cap at 20)
- Filter to `ReferenceKind::Call` only to avoid import-inflation

### API Design

```rust
/// When true, re-rank results by importance (caller count, churn, symbol kind).
/// Default: false for backward compatibility.
#[serde(default, deserialize_with = "lenient_bool")]
pub ranked: Option<bool>,
```

### Architecture: Git Temporal Access

**Recommended**: Pass `Option<&GitTemporalIndex>` as additional param to `collect_text_matches()`. Protocol handler clones `Arc<GitTemporalIndex>` before acquiring `LiveIndex` read lock (avoids lock ordering issues).

### Performance

| Operation | Cost | Frequency |
|-----------|------|-----------|
| Caller count lookup | O(1) HashMap | Per unique enclosing symbol |
| Git churn lookup | O(1) HashMap | Per result file |
| Kind priority | Free | Per match |
| Arc clone | ~10ns | Once per search |
| Re-sort | O(n log n) | Once per search |

**Worst case**: 50 files × 5 matches = 250 lookups → **<100µs overhead**

### Effort Estimate

| Component | Lines |
|-----------|-------|
| `ranked` param + serde | ~5 |
| `TextSearchOptions` update | ~2 |
| Plumbing (options, git temporal) | ~18 |
| Composite score function | ~40 |
| Kind priority helper | ~20 |
| Re-rank in `collect_text_matches` | ~30 |
| Optional format output | ~15 |
| Tests | ~80 |
| **Total** | **~210** |

### Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Common-name inflation (`new`, `get`, `default`) | Medium | Cap at 20, filter to `ReferenceKind::Call` |
| Git temporal not ready at search time | Low | Gracefully degrade: churn=0.0 |
| Lock ordering between `LiveIndex` and `git_temporal` | Low | Clone `Arc` before acquiring read lock |
| Backward compatibility | Low | `ranked` is opt-in, defaults to false |

---

## Feature 3: Kilo Code Auto-Detection in Init

### Problem
`symforge init` auto-detects Claude Code, Codex, and Gemini CLI but not Kilo Code (VS Code extension) or its CLI (@kilocode/cli). Users must manually configure.

### Affected Files and Symbols

| File | Symbols | Change Type |
|------|---------|-------------|
| `src/cli/mod.rs` | `InitClient` enum | Add `KiloCode`, `KiloCli` variants |
| `src/cli/init.rs` | `InitPaths` | Add `kilo_cli_config` field |
| `src/cli/init.rs` | `run_init_with_context()` | Add 2 new match branches |
| `src/cli/init.rs` | (new) `register_kilo_mcp_server()` | VS Code extension registration |
| `src/cli/init.rs` | (new) `register_kilo_cli_mcp_server()` | CLI registration |
| `src/cli/init.rs` | (new) `KILO_ALWAYS_ALLOW` | Tool allowlist constant |
| `tests/init_integration.rs` | (new tests) | Integration tests |

### Config Formats

**VS Code Extension** (`.kilocode/mcp.json` — workspace-local):
```json
{
  "mcpServers": {
    "SymForge": {
      "command": "symforge",
      "args": ["--stdio"],
      "alwaysAllow": ["health", "get_repo_map", "search_symbols", "..."]
    }
  }
}
```

**CLI** (`~/.config/kilo/kilo.json` — user-global):
```json
{
  "mcp": {
    "SymForge": {
      "type": "local",
      "command": ["symforge", "--stdio"]
    }
  }
}
```

### Key Design Decisions

1. **Two `InitClient` variants** — config formats differ significantly (workspace-local vs user-global, different JSON schemas)
2. **VS Code extension config is workspace-local** — new pattern since all existing clients (Claude, Codex, Gemini) write to `$HOME`
3. **No guidance file write** — Kilo Code doesn't have a `CLAUDE.md`-equivalent convention
4. **Follow existing registration patterns** — read-or-create JSON, upsert `SymForge` entry, write pretty-printed

### Effort Estimate

| Component | Lines |
|-----------|-------|
| `InitClient` enum additions | ~6 |
| `InitPaths` additions | ~8 |
| `KILO_ALWAYS_ALLOW` constant | ~15 |
| `register_kilo_mcp_server()` | ~45 |
| `register_kilo_cli_mcp_server()` | ~45 |
| `run_init_with_context()` branches | ~40 |
| Integration tests | ~80 |
| **Total** | **~240** |

### Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Kilo CLI config path on Windows | Medium | May need `%APPDATA%` instead of `~/.config/kilo/` — needs verification |
| Kilo CLI config format undocumented | Medium | Test with actual Kilo installation before release |
| Workspace-local `.kilocode/mcp.json` in `.gitignore` | Low | Add guidance in init output |

---

## Recommended Implementation Order

| Priority | Feature | Reason |
|----------|---------|--------|
| **1st** | Lenient Vec Deserializer | Unblocks all Kilo Code users immediately; smallest change; pure bugfix |
| **2nd** | Kilo Init Auto-Detection | Removes manual setup friction; small; pure addition |
| **3rd** | Semantic Search Ranking | Enhancement, not a fix; most architectural complexity; opt-in |
