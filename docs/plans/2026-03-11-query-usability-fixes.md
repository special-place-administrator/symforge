# Query Usability Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the highest-value real-world query gaps around typed references, dependency discovery, context bundles, and search ergonomics without regressing existing language behavior.

**Architecture:** Keep the semantic fixes in the parser and live-index query layers so protocol formatting remains thin. Add protocol/schema changes only where the public tool surface needs richer inputs or better git-aware defaults.

**Tech Stack:** Rust, tree-sitter, rmcp, git CLI, unit/integration tests

---

### Task 1: Add failing semantic-indexing tests

**Files:**
- Modify: `src/parsing/xref.rs`
- Modify: `src/live_index/query.rs`

**Steps:**
1. Add a failing C# xref test for constructor/field type usage.
2. Add a failing C# xref test proving `using Foo.Bar;` keeps the full qualified namespace.
3. Add failing dependent-query tests for C#/Java typed dependencies.
4. Add a failing callee-query test proving class-level bundles should include calls inside nested methods.

### Task 2: Implement parser/query semantic fixes

**Files:**
- Modify: `src/parsing/xref.rs`
- Modify: `src/live_index/query.rs`
- Modify: `src/protocol/format.rs`

**Steps:**
1. Extend C# xref extraction to record type usages and full qualified imports.
2. Teach `find_dependents_for_file` to use exported symbol names plus namespace/package heuristics for C#/Java typed dependents.
3. Broaden `callees_for_symbol` so a class/struct/module can surface calls made inside enclosed methods.
4. Update dependent formatting to label the actual reference kind instead of hard-coding `import`.

### Task 3: Add failing protocol/query tests

**Files:**
- Modify: `src/protocol/tools.rs`
- Modify: `src/protocol/format.rs`

**Steps:**
1. Add a failing `search_symbols(kind: ...)` test.
2. Add a failing `search_text` multi-term OR or regex-style test.
3. Add a failing `what_changed` test for default-uncommitted and git-ref-based behavior.

### Task 4: Implement protocol/tooling changes

**Files:**
- Modify: `src/protocol/tools.rs`
- Modify: `src/protocol/format.rs`
- Modify: `README.md`

**Steps:**
1. Split the shared search input structs so symbol and text search can evolve independently.
2. Add symbol-kind filtering while preserving current ranking output.
3. Add multi-term OR and optional regex search with trigram prefiltering where practical.
4. Add git-aware `what_changed` modes, defaulting to uncommitted changes when a git repo is available.
5. Document the new behavior and any remaining heuristics honestly.

### Task 5: Verify

**Files:**
- Modify: `README.md`

**Steps:**
1. Run targeted tests for each red/green cycle.
2. Run the full test suite and capture any unrelated failures separately.
3. Smoke-test the changed tools locally with exact commands and record outputs.
