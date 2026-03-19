# Symbol Edit Boundaries — Design Spec

**Date**: 2026-03-19
**Status**: Draft
**Scope**: Structural edit reliability for symbol-addressed tools

## Problem

SymForge's symbol-addressed edit tools are strong when the parser's `SymbolRecord.byte_range` already matches the full editable item. They become unreliable when real edit boundaries extend beyond the core symbol node:

- Rust attributes such as `#[derive(...)]`, `#[cfg(...)]`, `#[serde(...)]`
- decorators in TypeScript, JavaScript, and Python
- C# / Java / Kotlin annotations
- import and re-export statements that are definition-like but not modeled as ordinary symbols
- leading comment or modifier blocks that belong to the item semantically but are not represented by `doc_byte_range`

The current data model only gives the editor two boundary concepts:

- `byte_range` for the parser node
- `doc_byte_range` for attached doc comments

That is enough for:

- body replacement inside the node
- insertion before a symbol while keeping doc comments attached
- deletion that tries to pull in orphaned doc comments

It is not enough for:

- inserting above a Rust item without landing between `#[derive]` and the item
- replacing or deleting an item without leaving behind attributes or decorators
- performing "pure text-boundary" fixes that are adjacent to a symbol but not inside the symbol's current core range

This is why shell-level text edits still feel necessary in a few high-friction cases. The gap is in boundary semantics, not in file I/O, reindexing, or atomic write behavior.

## Goal

Make symbol-addressed edits boundary-aware enough that an LLM can stay inside SymForge for nearly all definition-adjacent edits, including attribute/decorator-heavy code, without falling back to raw shell/file tooling.

## Non-Goals

- Build a full AST rewrite engine.
- Introduce formatter-dependent mutation logic.
- Solve arbitrary non-symbol text edits in one step.
- Change the public mental model of existing tools more than necessary.

## Current State

### Existing edit pipeline

Current edit operations are centered in `src/protocol/edit.rs` and `src/protocol/tools.rs`.

Important behavior today:

- `SymbolRecord.byte_range` is the parser node range.
- `SymbolRecord.doc_byte_range` is optional and comment-focused.
- `effective_start()` in `src/domain/index.rs` uses `doc_byte_range` if present.
- `build_insert_before()` uses `effective_start()` and line-start splicing.
- `build_insert_after()` inserts at `sym.byte_range.1`.
- `delete_symbol()` and `replace_symbol_body()` rely on `byte_range` plus ad hoc doc-comment cleanup.
- `edit_within_symbol()` is intentionally scoped to the symbol body.

This model is internally coherent, but it treats doc comments as the only meaningful leading trivia.

### Important precedent already in the codebase

Python's parser already treats `decorated_definition` specially and uses the decorated node's full range for the symbol. That proves the core idea is correct: the editor wants the full item boundary, not just the inner declaration node.

The problem is that this is currently language-specific and ad hoc instead of being a first-class boundary model.

## Root Cause

The editor lacks a stable distinction between:

- the symbol's semantic core
- the full editable item
- leading trivia attached to the item

The current code approximates this with `doc_byte_range`, but attributes, decorators, annotations, and import-style definitions are not modeled the same way.

## Proposed Design

## Boundary Model

Extend `SymbolRecord` with a new range for the full editable item:

```rust
pub struct SymbolRecord {
    pub name: String,
    pub kind: SymbolKind,
    pub depth: u32,
    pub sort_order: u32,
    pub byte_range: (u32, u32),          // core symbol node
    pub line_range: (u32, u32),
    pub doc_byte_range: Option<(u32, u32)>,
    pub item_byte_range: Option<(u32, u32)>,
}
```

Semantics:

- `byte_range`: the parser's core node for the symbol
- `doc_byte_range`: attached doc-comment span only
- `item_byte_range`: the full editable item, including attached decorators / attributes / annotations / doc comments when they are semantically part of the definition

Rules:

- `item_byte_range` must always fully contain `byte_range`
- if `doc_byte_range` exists, `item_byte_range.0 <= doc_byte_range.0`
- if a language cannot compute anything richer, `item_byte_range = Some(byte_range)`

## Boundary Helpers

Add explicit helper methods in `src/domain/index.rs`:

```rust
impl SymbolRecord {
    pub fn item_start(&self) -> u32 { ... }
    pub fn item_end(&self) -> u32 { ... }
    pub fn item_range(&self) -> (u32, u32) { ... }
    pub fn core_range(&self) -> (u32, u32) { ... }
}
```

Keep `effective_start()` temporarily for compatibility, but reimplement it as `item_start()`. Then migrate call sites toward the clearer helpers.

## Edit Semantics

Structural edit tools should use `item_range`, not `byte_range`.

### `insert_symbol(..., position="before")`

Insert before the line containing `item_start()`.

Effect:

- new code lands above doc comments and above attribute/decorator blocks
- no insertion between `#[derive(...)]` and the struct/function/class it belongs to

### `insert_symbol(..., position="after")`

Insert after `item_end()`.

Effect:

- insertion occurs after the full item, not inside trailing attribute/decorator or wrapper syntax

### `replace_symbol_body`

Despite the historical name, this tool is already used as "replace the entire definition." Its splice range should become the symbol's full item range.

That gives the most predictable behavior for:

- Rust items with attribute blocks
- decorated definitions
- annotations that semantically belong to the item

If preserving leading trivia becomes important later, add a separate tool or parameter rather than overloading the current one with ambiguous partial behavior.

### `delete_symbol`

Delete the full item range and then run the existing blank-line normalization.

This removes the need for orphaned-doc special cases to carry most of the load. `extend_past_orphaned_docs()` can remain as a compatibility fallback while parsers are upgraded.

### `edit_within_symbol`

Keep this scoped to `byte_range`, not `item_range`.

Reason:

- `edit_within_symbol` is the "surgical" primitive
- it should not unexpectedly rewrite decorators, annotations, or doc blocks
- it remains the safe choice for body-local edits

## Parser Responsibilities

Boundary accuracy must come from parsing, not from increasingly clever editor heuristics.

### General rule

For languages with wrapper nodes that naturally represent the full item, index that wrapper range as `item_byte_range`.

Examples:

- Python: `decorated_definition`
- TypeScript / JavaScript: decorator wrapper or enclosing declaration form
- C# / Java / Kotlin: attribute or annotation-bearing declaration span
- Rust: include outer attributes attached to the item

### Tree-sitter integration

Update the language parsing layer in `src/parsing/languages/` so symbol push helpers can accept an optional outer item node or explicit outer range:

```rust
push_symbol_with_item_range(
    core_node,
    item_node_or_range,
    ...
)
```

This keeps the data model uniform while allowing each language to express its own wrapper-node rules.

### Config extractors

Config extractors should set:

- `doc_byte_range = None`
- `item_byte_range = Some(byte_range)`

They do not need richer trivia semantics.

## Pure Text-Boundary Escape Hatch

Even after boundary-aware symbol edits land, there will still be edits that are:

- adjacent to a symbol
- deterministic
- not naturally expressible as a symbol operation

Examples:

- splitting or merging import groups
- inserting a comment between two sibling items
- adjusting a contiguous modifier block shared across multiple symbols

SymForge should eventually add a low-level, explicitly dangerous but bounded primitive such as:

```rust
splice_file_range(path, start_byte, end_byte, new_text)
```

Constraints:

- exact repo-scoped path only
- explicit byte or line range
- dry-run support
- strong warnings in tool description
- reindex-after-write always

This is not a replacement for symbol-addressed edits. It is the pressure-release valve that prevents shell fallback when the change is fundamentally not a symbol edit.

## Implementation Phases

### Phase 1: Data model and compatibility plumbing

Files:

- `src/domain/index.rs`
- `src/live_index/store.rs`
- `src/live_index/persist.rs`
- `src/parsing/mod.rs`

Tasks:

- add `item_byte_range` to `SymbolRecord`
- preserve it through indexing, serialization, and snapshots
- add helper methods for item/core boundaries
- keep existing tools working by defaulting `item_byte_range` to `byte_range`

### Phase 2: Parser upgrades

Files:

- `src/parsing/languages/mod.rs`
- selected files in `src/parsing/languages/`

Tasks:

- extend symbol push helpers to accept outer item ranges
- migrate Python to the new generic mechanism
- add Rust attribute-aware item range handling
- add decorator/annotation-aware handling for the highest-value languages first

Priority order:

1. Rust
2. Python migration to generic helper
3. TypeScript / JavaScript
4. C# / Java / Kotlin

### Phase 3: Edit engine switch-over

Files:

- `src/protocol/edit.rs`
- `src/protocol/tools.rs`

Tasks:

- make structural edit operations use `item_range`
- keep `edit_within_symbol` on `byte_range`
- reduce ad hoc orphaned-doc logic once parser-backed item ranges are trusted

### Phase 4: Optional range-splice primitive

Files:

- `src/protocol/edit.rs`
- `src/protocol/edit_format.rs`
- `src/protocol/tools.rs`

Tasks:

- add a bounded raw-range edit tool
- keep it explicit and lower-level than symbol edits
- require exact path and exact range

## Testing Strategy

### Unit tests

Add or update tests in `src/protocol/edit.rs` and parser modules for:

- insert-before on Rust `#[derive(...)] struct`
- replace/delete on Rust items with stacked attributes
- insert-before on Python decorated definitions using the generic boundary mechanism
- insert-after on annotated declarations
- unchanged behavior for ordinary functions without leading trivia
- snapshot persistence of `item_byte_range`

### Regression tests

Add exact regressions for the cases that forced raw text edits in real sessions:

- insertion landing between derive block and item
- insertion landing inside import/re-export declaration boundaries
- replacement leaving behind annotations or decorators

### Integration tests

Tool-level tests in `src/protocol/tools.rs`:

- `insert_symbol` before an attributed item
- `replace_symbol_body` on a decorated / attributed definition
- `delete_symbol` removes the full item and reindexes cleanly

### Property-style safety checks

For languages with richer boundary logic:

- `item_range` must contain `byte_range`
- `item_start <= core_start <= core_end <= item_end`
- edits must never produce mixed line endings

## Acceptance Criteria

- Symbol-addressed structural edits no longer require shell fallback for attribute/decorator boundary cases.
- `insert_symbol(position="before")` never lands between an attached attribute/decorator block and its symbol.
- `delete_symbol` and `replace_symbol_body` do not orphan attached attributes, decorators, or doc comments.
- `edit_within_symbol` remains body-local and does not unexpectedly rewrite leading trivia.
- Snapshot persistence preserves the richer boundary model.
- Old files and old indexes remain readable with sensible defaults.

## Risks

- Tree-sitter grammars differ sharply in how they represent decorators and annotations.
- Import-style declarations may still need separate symbol modeling in some languages.
- Over-expanding `item_byte_range` could accidentally swallow neighboring comments that are not truly attached to the symbol.

## Recommended Sequencing

Do not start with the optional raw-range primitive.

The highest-value path is:

1. add `item_byte_range`
2. migrate structural edit tools to it
3. upgrade Rust and Python first
4. only then add the escape-hatch range tool if real sessions still need it

That keeps SymForge's primary editing story symbol-addressed instead of quietly drifting back toward generic text editing.
