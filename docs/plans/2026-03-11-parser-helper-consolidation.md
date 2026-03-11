# Parser Helper Consolidation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce repeated symbol-extraction boilerplate in `src/parsing/languages` without changing extraction behavior.

**Architecture:** Add a small shared helper layer inside `src/parsing/languages/mod.rs` for three things only: pushing `SymbolRecord`s, walking child nodes with the standard depth rule, and finding simple child-name nodes. Keep language-specific control flow and special declarations in each language module. Avoid macros and avoid changing the public parsing API.

**Tech Stack:** Rust, tree-sitter, existing parser unit/integration tests, `cargo test`, `cargo clippy`

---

### Task 1: Add failing helper tests

**Files:**
- Modify: `src/parsing/languages/mod.rs`

**Step 1: Write failing tests**

- Add tests for:
  - shared helper pushes a `SymbolRecord` with byte range, line range, depth, and incremented sort order
  - shared child walker increments depth only when the parent node was recorded as a symbol
  - simple shared child-name finder returns the first matching node kind and `None` when absent

**Step 2: Run tests to verify they fail**

Run: `cargo test parsing::languages::tests -- --nocapture`

Expected: helper tests fail because the shared helpers do not exist yet.

### Task 2: Implement the shared helper layer

**Files:**
- Modify: `src/parsing/languages/mod.rs`

**Step 1: Add minimal helper API**

- Add a small internal helper surface:
  - `push_symbol(...)`
  - `push_named_symbol(...)`
  - `walk_children(...)`
  - `find_first_named_child(...)`

**Step 2: Keep scope tight**

- Do not change `pub fn extract_symbols(...)` dispatch.
- Do not introduce macros.
- Do not try to unify the special cases in C/C++, Rust impl names, Elixir defs, or declaration fan-out logic.

**Step 3: Run helper tests**

Run: `cargo test parsing::languages::tests -- --nocapture`

Expected: new helper tests pass.

### Task 3: Migrate the simple language extractors

**Files:**
- Modify: `src/parsing/languages/csharp.rs`
- Modify: `src/parsing/languages/dart.rs`
- Modify: `src/parsing/languages/kotlin.rs`
- Modify: `src/parsing/languages/php.rs`
- Modify: `src/parsing/languages/python.rs`
- Modify: `src/parsing/languages/ruby.rs`
- Modify: `src/parsing/languages/swift.rs`
- Modify: `src/parsing/languages/javascript.rs`
- Modify: `src/parsing/languages/typescript.rs`
- Modify: `src/parsing/languages/java.rs`
- Modify: `src/parsing/languages/go.rs`
- Modify: `src/parsing/languages/perl.rs`
- Modify: `src/parsing/languages/elixir.rs`
- Modify: `src/parsing/languages/rust.rs`
- Modify: `src/parsing/languages/c.rs`
- Modify: `src/parsing/languages/cpp.rs`

**Step 1: Migrate only where the helper genuinely reduces duplication**

- Replace repeated `SymbolRecord` construction with `push_symbol`/`push_named_symbol`.
- Replace repeated child traversal loops with `walk_children`.
- Replace simple `find_name` scans with `find_first_named_child` where the language logic is truly just “first matching child kind”.

**Step 2: Keep custom logic custom**

- Preserve custom name extraction for:
  - C/C++ declarator walking
  - Rust `impl` naming
  - Perl symbol-kind-sensitive name selection
  - Elixir `def` extraction
  - declaration fan-out in Go/Java/JS/TS where one node can emit multiple symbols

**Step 3: Run targeted tests**

Run:
- `cargo test parsing::languages -- --nocapture`
- `cargo test parsing::tests -- --nocapture`

Expected: parsing tests remain green.

### Task 4: Verify the refactor across the repo

**Files:**
- Verify only

**Step 1: Run full linting**

Run: `cargo clippy --all-targets -- -W clippy::all`

Expected: pass cleanly.

**Step 2: Run the full suite**

Run: `cargo test`

Expected: full suite passes with no regressions.
