# Symbol Edit Boundaries Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make symbol-addressed structural edits boundary-aware so SymForge can handle attribute/decorator-adjacent edits without falling back to raw shell/text tooling.

**Architecture:** Introduce `item_byte_range` as the full editable-item boundary on `SymbolRecord`, preserve it through indexing and snapshots, teach parser helpers to compute it, then switch structural edit tools to use item boundaries while keeping `edit_within_symbol` scoped to the core node range. Audit existing `effective_start()` consumers before repurposing shared helpers so edit-boundary work does not accidentally change unrelated read/display surfaces.

**Tech Stack:** Rust, tree-sitter language parsers already in the repo, existing edit pipeline in `src/protocol/edit.rs`

**Spec:** `docs/superpowers/specs/2026-03-19-symbol-edit-boundaries-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/domain/index.rs` | Modify | Add `item_byte_range` and boundary helpers on `SymbolRecord` |
| `src/live_index/store.rs` | Modify | Preserve `item_byte_range` in `IndexedFile` flows |
| `src/live_index/persist.rs` | Modify | Snapshot round-trip for richer symbol boundary data |
| `src/live_index/query.rs` | Review / maybe modify | Decide whether context bundle uses doc-aware or full-item boundaries |
| `src/parsing/languages/mod.rs` | Modify | Add generic symbol push helpers that accept full item boundaries |
| `src/parsing/languages/python.rs` | Modify | Migrate decorated definitions to generic item-range support |
| `src/parsing/languages/rust.rs` | Modify | Include outer attributes in symbol item ranges |
| `src/parsing/languages/javascript.rs` | Modify | Model export/decorator-aware item ranges where supported |
| `src/parsing/languages/typescript.rs` | Modify | Model export/decorator-aware item ranges where supported |
| `src/parsing/languages/csharp.rs` | Modify | Include attribute blocks in symbol item ranges |
| `src/parsing/config_extractors/*.rs` | Modify | Default `item_byte_range` to `byte_range` for config pseudo-symbols |
| `src/protocol/edit.rs` | Modify | Switch structural edit primitives to `item_range`, keep `edit_within_symbol` on core range |
| `src/protocol/tools.rs` | Modify | Preserve tool semantics and add regressions for boundary-aware edits |
| `src/protocol/format.rs` | Review / maybe modify | Decide whether symbol display should show core range or full item range |
| `tests/` and inline test modules | Modify | Regression coverage for attribute/decorator boundary cases |

---

## Chunk 1: Domain Model + Persistence

### Task 1: Add full-item boundary data to `SymbolRecord`

**Files:**
- Modify: `src/domain/index.rs`

- [ ] **Step 1: Add `item_byte_range` to `SymbolRecord`**

Add an optional field:

```rust
pub item_byte_range: Option<(u32, u32)>,
```

Place it next to `byte_range` / `doc_byte_range` so boundary semantics stay visually grouped.

- [ ] **Step 2: Add boundary helper methods**

Add methods on `SymbolRecord`:

```rust
pub fn item_start(&self) -> u32
pub fn item_end(&self) -> u32
pub fn item_range(&self) -> (u32, u32)
pub fn core_range(&self) -> (u32, u32)
```

Rules:
- `item_*` falls back to `byte_range` when `item_byte_range` is `None`
- `core_range()` always returns `byte_range`

- [ ] **Step 3: Add new helpers without repurposing `effective_start()` yet**

Do not delete `effective_start()` in this chunk, and do not blindly repurpose it to `item_start()` yet.

Reason:
- `effective_start()` is used by structural edit code
- it is also used by read/display code such as symbol rendering and context-bundle capture

Keep the old behavior stable until the read-surface audit below decides whether full-item boundaries should also become the default display boundary.

- [ ] **Step 4: Fix all symbol construction sites**

Search for every `SymbolRecord {` construction across:
- `src/parsing/languages/`
- `src/parsing/config_extractors/`
- `src/live_index/`
- `src/protocol/`
- tests

Add:

```rust
item_byte_range: None,
```

or a specific value where already known.

- [ ] **Step 5: Audit `effective_start()` consumers before migration**

Search for `effective_start()` across the repo and classify each call site as one of:
- structural edit boundary
- overlap/ordering logic for edits
- read/display rendering
- context bundle / retrieval

At minimum, review:
- `src/protocol/edit.rs`
- `src/protocol/tools.rs`
- `src/protocol/format.rs`
- `src/live_index/query.rs`

Record the migration intent in a short note in this plan while coding:
- edit paths should move to `item_*` helpers
- read/display paths must be an explicit product decision, not an accidental side effect

- [ ] **Step 6: Compile check**

Run:

```bash
cargo check --workspace
```

Expected: compiler guides all remaining construction sites and equality/serialization fixes.

### Task 2: Preserve item boundaries through index storage and snapshots

**Files:**
- Modify: `src/live_index/store.rs`
- Modify: `src/live_index/persist.rs`

- [ ] **Step 1: Preserve the field in in-memory index state**

Ensure any conversions from parsing output into stored/indexed forms keep the new symbol field intact.

- [ ] **Step 2: Extend snapshot schema**

Update snapshot serialization/deserialization to preserve `item_byte_range`.

If snapshot format versioning is used, bump the version and keep old-snapshot behavior deterministic.

- [ ] **Step 3: Add round-trip test**

Add or extend a persistence test so a symbol with:
- `byte_range`
- `doc_byte_range`
- `item_byte_range`

survives snapshot serialize/load unchanged.

- [ ] **Step 4: Run targeted tests**

Run:

```bash
cargo test persist -- --test-threads=1
```

---

## Chunk 2: Parser Infrastructure

### Task 3: Add generic parser support for full item ranges

**Files:**
- Modify: `src/parsing/languages/mod.rs`

- [ ] **Step 1: Introduce a generic push helper that can accept core and item boundaries separately**

Refactor the existing helper layer so language parsers can express:
- core node for symbol identity/name lookup
- outer item node or explicit outer byte range for editing

One acceptable shape:

```rust
push_symbol_with_item_range(...)
push_named_symbol_with_item_range(...)
```

Do not force every caller to supply an outer range. Ordinary parsers should still be able to use the simpler path.

- [ ] **Step 2: Default item range to the core range**

The generic helper should set:

```rust
item_byte_range: Some(core_range)
```

when no richer item range is provided.

That avoids `None`-heavy downstream logic and makes the boundary model explicit.

- [ ] **Step 3: Keep doc comment scanning orthogonal**

Do not merge doc comments into the item range in ad hoc parser code unless the caller explicitly wants that full item span. The helper should combine:
- core range
- optional doc range
- optional outer item range

predictably.

- [ ] **Step 4: Add helper-level tests**

Add tests in `src/parsing/languages/mod.rs` for:
- default item range equals core range
- explicit outer range overrides core range
- `doc_byte_range` and `item_byte_range` can coexist without conflicting

---

## Chunk 3: High-Value Language Migrations

### Task 4: Migrate Python to the generic full-item model

**Files:**
- Modify: `src/parsing/languages/python.rs`

- [ ] **Step 1: Replace the current ad hoc decorated-definition handling**

Python already uses `decorated_definition` as the effective symbol span. Re-express that through the new generic helper so Python becomes the first clean implementation of the shared model.

- [ ] **Step 2: Keep nested symbol behavior unchanged**

Decorated class/method/function behavior must still recurse into the inner declaration as it does today.

- [ ] **Step 3: Add regression tests**

Add tests proving:
- `item_byte_range` begins at `@decorator`
- `byte_range` still points to the inner declaration node if the model keeps those distinct
- decorated definitions still produce the same symbol names and kinds

### Task 5: Add Rust outer-attribute item ranges

**Files:**
- Modify: `src/parsing/languages/rust.rs`

- [ ] **Step 1: Detect outer attributes attached to items**

For supported Rust item kinds (`function_item`, `struct_item`, `enum_item`, `trait_item`, `impl_item`, `const_item`, `static_item`, `mod_item`, `type_item`), compute an item start that includes preceding outer attributes.

- [ ] **Step 2: Preserve existing doc comment support**

Do not regress `DOC_SPEC` behavior. Rust items with:
- doc comments only
- attributes only
- doc comments plus attributes

must all produce sane full-item ranges.

- [ ] **Step 3: Add Rust regressions**

Add parser tests for:
- `#[derive(Clone)] struct Foo`
- stacked attributes
- doc comments followed by attributes followed by item

Expected: `item_byte_range` starts before the first attached attribute or doc comment.

### Task 6: Extend JS/TS/C# for attribute/decorator/export boundaries

**Files:**
- Modify: `src/parsing/languages/javascript.rs`
- Modify: `src/parsing/languages/typescript.rs`
- Modify: `src/parsing/languages/csharp.rs`

- [ ] **Step 1: JavaScript / TypeScript**

Handle the highest-value wrapper cases first:
- exported declarations
- decorated class/method declarations where the grammar exposes a stable outer node

If some decorator forms are not represented consistently by the grammar, document the limitation in code comments and tests rather than guessing.

Do not block this chunk on import/re-export group editing. Import/re-export groups are not currently modeled as ordinary symbols in the same way as function/class/type declarations. Treat that as a separate symbolization problem or a future raw-range-edit use case.

- [ ] **Step 2: C#**

Include leading attribute lists in `item_byte_range` for declarations that SymForge indexes.

- [ ] **Step 3: Add focused parser tests**

Add at least one regression per language proving that item boundaries expand to include the leading wrapper trivia.

---

## Chunk 4: Config and Fallback Symbol Construction

### Task 7: Default non-tree-sitter symbols to full-item range = core range

**Files:**
- Modify: `src/parsing/config_extractors/env.rs`
- Modify: `src/parsing/config_extractors/json.rs`
- Modify: `src/parsing/config_extractors/markdown.rs`
- Modify: `src/parsing/config_extractors/toml_ext.rs`
- Modify: `src/parsing/config_extractors/yaml.rs`
- Modify any other direct `SymbolRecord` builders

- [ ] **Step 1: Set `item_byte_range` explicitly**

For config pseudo-symbols, set:

```rust
item_byte_range: Some(byte_range)
```

This keeps the model uniform even where there is no richer leading trivia concept.

- [ ] **Step 2: Add one representative test**

Choose one config extractor and assert that `item_byte_range == Some(byte_range)` for produced symbols.

---

## Chunk 5: Edit Engine Switch-Over

### Task 8: Make structural edit primitives use full item boundaries

**Files:**
- Modify: `src/protocol/edit.rs`

- [ ] **Step 1: Update `build_insert_before()`**

Use `item_start()` rather than doc-comment-only logic as the primary anchor.

Preserve indentation and line-ending behavior from the current implementation.

- [ ] **Step 2: Update `build_insert_after()`**

Insert after `item_end()` rather than `byte_range.1`.

- [ ] **Step 3: Update structural replacement/deletion paths**

Use `item_range()` for:
- `replace_symbol_body`
- `delete_symbol`
- batch structural edits that rely on symbol boundaries

This includes:
- overlap detection in `execute_batch_edit`
- reverse-offset ordering for batched structural edits
- old/new byte-count reporting for structural edit summaries

- [ ] **Step 4: Keep `edit_within_symbol()` on the core range**

Do not silently expand it to `item_range()`. It must remain the narrow, body-local primitive.

- [ ] **Step 5: Reduce reliance on orphaned-doc heuristics**

Keep `extend_past_orphaned_docs()` only as a compatibility fallback where parser coverage is still incomplete. Once `item_range()` is trusted in a path, do not double-expand it.

- [ ] **Step 6: Decide read-surface behavior explicitly**

Using the consumer audit from Chunk 1, make an explicit call for:
- `render_symbol_detail` in `src/protocol/format.rs`
- `capture_context_bundle_view` in `src/live_index/query.rs`

Allowed outcomes:
- keep them doc-aware/core-based for display stability
- move them to full-item boundaries for consistency with editing

Either choice is acceptable, but it must be deliberate and covered by tests.

- [ ] **Step 7: Add edit-unit regressions**

Add tests in `src/protocol/edit.rs` for:
- insert-before on a Rust attributed item
- replace on an attributed/decorated item
- delete on an attributed/decorated item
- unchanged `edit_within_symbol` scope
- batch overlap detection uses full item boundaries, not just core ranges

---

## Chunk 6: Tool-Level Regression Coverage

### Task 9: Add tool tests for real failure modes

**Files:**
- Modify: `src/protocol/tools.rs`
- Modify integration tests if needed

- [ ] **Step 1: Add `insert_symbol` regression**

Write a test proving `insert_symbol(position=\"before\")` does not land between a Rust `#[derive(...)]` block and the struct it belongs to.

- [ ] **Step 2: Add `replace_symbol_body` regression**

Write a test proving replacement removes the full attributed/decorated definition instead of leaving wrappers behind.

- [ ] **Step 3: Add `delete_symbol` regression**

Write a test proving delete removes the whole item and reindexes cleanly.

- [ ] **Step 4: Run targeted tool tests**

Run:

```bash
cargo test protocol::tools -- --test-threads=1
cargo test protocol::edit -- --test-threads=1
```

---

## Chunk 7: Final Verification + Cleanup

### Task 10: Full validation pass

**Files:** all touched files

- [ ] **Step 1: Run compile and no-run tests**

```bash
cargo check --workspace
cargo test --no-run
```

- [ ] **Step 2: Run the highest-signal full test slices**

At minimum:

```bash
cargo test parsing::languages -- --test-threads=1
cargo test protocol::edit -- --test-threads=1
cargo test protocol::tools -- --test-threads=1
cargo test live_index::persist -- --test-threads=1
```

- [ ] **Step 3: Review edit behavior manually through SymForge tools**

Use the actual MCP tools against a small fixture or scratch file to confirm:
- before/after insertion anchors
- replace/delete full-item handling
- `edit_within_symbol` staying core-scoped

If Chunk 5 changed read/display behavior, also verify:
- `get_symbol`
- `get_symbol_context` / context bundle paths
- any user-visible byte counts still make sense

- [ ] **Step 4: Update user-facing docs only if behavior changed materially**

If tool descriptions or help text now need to explain “full item” vs “symbol body,” update them in `src/protocol/tools.rs` and related docs.

---

## Deferred Follow-Up: Raw Range Escape Hatch

This is **not part of the main implementation path**.

If real sessions still expose deterministic non-symbol edits that boundary-aware symbol tools cannot express cleanly, create a separate spec/plan for a bounded low-level primitive such as:

```rust
splice_file_range(path, start_byte, end_byte, new_text)
```

Only pursue that after the boundary-aware symbol model is complete and verified.

---

## Dependency Graph

1. Task 1 must land before everything else.
2. Task 2 depends on Task 1.
3. Task 3 depends on Task 1.
4. Tasks 4, 5, and 6 depend on Task 3.
5. Task 7 depends on Task 1.
6. Task 8 depends on Tasks 1, 3, and at least one language migration being complete.
7. Task 9 depends on Task 8.
8. Task 10 depends on all prior tasks.
