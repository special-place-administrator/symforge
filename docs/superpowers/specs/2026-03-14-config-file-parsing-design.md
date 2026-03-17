# Config File Parsing — Design Spec

**Date**: 2026-03-14
**Sprint**: 11 (Tier 1)
**Status**: Draft
**Approach**: Native Rust parsers (Approach A)

## Problem

SymForge only indexes source code files (16 languages via tree-sitter). Config and doc files (JSON, TOML, YAML, Markdown, .env) are silently skipped at discovery time. LLMs must fall back to built-in Read/Edit tools for these files, losing all SymForge benefits (symbol navigation, structured search, targeted edits, token savings).

These files are everywhere in every project — `package.json`, `Cargo.toml`, `docker-compose.yaml`, `README.md`, `.env` — and are read/edited constantly during development sessions.

## Goal

Make JSON, TOML, YAML, Markdown, and .env files first-class citizens in the LiveIndex by producing pseudo-symbols from their structure. All existing read/search/navigation tools work on these files without modification. Edit tools are gated by per-format capability levels to prevent broken-file outcomes.

## Domain Model Changes

### LanguageId (src/domain/index.rs)

Add variants to the existing enum:

```
Json, Toml, Yaml, Markdown, Env
```

Update `from_extension()`:
- `.json` → Json
- `.toml` → Toml
- `.yaml`, `.yml` → Yaml
- `.md` → Markdown
- `.env` → Env (note: `from_extension` extracts text after the last dot, so `.env` yields `"env"`. Dotenv variants like `.env.local`, `.env.production` yield `"local"`/`"production"` and are **out of scope for v1**. A future enhancement could match by filename prefix instead of extension.)

### SymbolKind (src/domain/index.rs)

Add variants:

```
Key       — JSON/TOML/YAML key-value pairs (name = dot-joined path)
Section   — Markdown headers (name = header text)
```

Reuse existing `Variable` kind for `.env` entries.

### FileClassification (src/domain/index.rs)

`FileClassification` is a struct with fields `class: FileClass`, `is_generated: bool`, `is_test: bool`, `is_vendor: bool`. `FileClass` is an enum with `Code`, `Text`, `Binary`. Add `is_config: bool` field to the struct to distinguish config files from source when needed. Config files get `FileClass::Text` with `is_config: true`.

## Edit Capability Levels

Each config file type declares its edit safety level. Edit tools check this before operating.

```rust
pub enum EditCapability {
    IndexOnly,            // read/search/navigation only, no edits
    TextEditSafe,         // edit_within_symbol (scoped find-replace) is safe
    StructuralEditSafe,   // replace_symbol_body, delete_symbol are safe
}
```

| Format | Capability | Rationale |
|--------|-----------|-----------|
| TOML | `StructuralEditSafe` | `toml_edit` spans are reliable |
| .env | `StructuralEditSafe` | Full-line spans, no structural dependencies |
| Markdown | `TextEditSafe` | Section boundaries are heuristic |
| JSON | `TextEditSafe` | Byte ranges are accurate, but delete/replace may break comma/bracket syntax |
| YAML | `TextEditSafe` | Line-based offsets are heuristic; indentation-sensitive edits are risky |

Edit tools check the file's capability:
- `edit_within_symbol` — requires `TextEditSafe` or higher
- `replace_symbol_body`, `delete_symbol` — requires `StructuralEditSafe`
- Below-threshold operations return a warning: "This file type does not support structural edits. Use edit_within_symbol or raw Edit instead."

The capability is stored as a method on the `ConfigExtractor` trait (see below), not on individual symbols.

## Extractor Architecture

### ConfigExtractor trait (src/parsing/config_extractors.rs)

Extensible trait for config file parsers. Future formats (INI, XML, .properties, .dockerignore) implement this trait without touching dispatch logic.

```rust
pub trait ConfigExtractor {
    fn extract(&self, content: &[u8]) -> Vec<SymbolRecord>;
    fn edit_capability(&self) -> EditCapability;
}
```

Registry dispatches by `LanguageId`:

```rust
pub fn extractor_for(language: LanguageId) -> Option<Box<dyn ConfigExtractor>> {
    match language {
        LanguageId::Json => Some(Box::new(JsonExtractor)),
        LanguageId::Toml => Some(Box::new(TomlExtractor)),
        LanguageId::Yaml => Some(Box::new(YamlExtractor)),
        LanguageId::Markdown => Some(Box::new(MarkdownExtractor)),
        LanguageId::Env => Some(Box::new(EnvExtractor)),
        _ => None,
    }
}
```

### Integration point: src/parsing/mod.rs

In the public entry points `process_file` / `process_file_with_classification` (not the private `parse_source`): if the language is a config type, call `extract_config_symbols` instead of entering the tree-sitter pipeline. This branch must happen before `parse_source` is called, since `parse_source` immediately creates a tree-sitter `Parser`.

### Byte Range Strategy

| Format | Parser | Byte range covers |
|--------|--------|-------------------|
| JSON | `serde_json` + manual byte offset tracking | Key start to value end (including quotes/braces) |
| TOML | `toml_edit` (spans via `Item::span()` → `Option<Range<usize>>`, handle `None`) | Key-value pair including inline comment |
| YAML | `serde_yml` + line-based offset calculation | Key-value line(s) |
| Markdown | Regex line scan for `^#{1,6} ` | Header line to next same-or-higher-level header |
| .env | Line scan for `KEY=value` | Full line |

### Symbol Naming Convention

Dot-joined key paths for structured formats. To avoid ambiguity when raw keys contain dots, brackets, or tildes, symbol names use a JSON-Pointer-style escape scheme:

**Escape rules (applied to each individual key segment before joining):**
- `~` → `~0`
- `.` → `~1`
- `[` → `~2`
- `]` → `~3`

**Examples:**

```
# JSON/TOML/YAML — normal keys:
scripts.test          → kind: Key   (two segments: "scripts", "test")
dependencies.serde    → kind: Key
services.api.ports    → kind: Key   (three segments)

# Raw key containing a literal dot:
raw key "a.b"         → symbol name: a~1b     (one segment, escaped)
nested under "x"      → symbol name: x.a~1b   (two segments, second escaped)

# Arrays — structural index, not escaped:
items[0]              → kind: Key   (array access, NOT a raw key)
items[1]              → kind: Key

# Raw key containing literal brackets:
raw key "items[0]"    → symbol name: items~20~3 (one segment, escaped)

# Markdown:
Installation          → kind: Section
Installation.Prerequisites → kind: Section (nested via header level)
Installation#2        → kind: Section (duplicate header disambiguation)

# .env:
DATABASE_URL          → kind: Variable
```

**Lookup**: `get_symbol(name="x.a~1b")` returns the value at key `"a.b"` nested under `"x"`. Unescaped names work for the common case (no special characters in keys). The escaping is transparent — users only need it when keys literally contain `.`, `[`, `]`, or `~`.

**Markdown duplicate headers**: When multiple headers have the same text at the same level, append `#N` (1-indexed, starting from the second occurrence): `Installation`, `Installation#2`, `Installation#3`.

### Depth and Size Limits

- **Depth cap**: 6 levels for JSON/TOML/YAML (prevents explosion on pathological files)
- **Array cap**: 20 items per array (emit `key[0]` through `key[19]`, skip rest)
- **No cross-references**: Config files produce no `ReferenceRecord` entries. The `references` field on `IndexedFile` is empty for config files.

## Discovery and Watcher

**Zero changes needed.** Both `discover_files` (src/discovery/mod.rs) and `supported_language` (src/watcher/mod.rs) gate on `LanguageId::from_extension()`. Adding the new enum variants and extension mappings is sufficient — config files will be discovered, indexed, and watched automatically.

## Tool Impact

### Read/Search/Navigation Tools — No Changes Needed

Query tools are extension-agnostic and work with any `SymbolRecord`:

- `search_symbols` — filter by `kind="key"` or `kind="section"`.
- `get_symbol` / `get_symbol_context` — resolves by name + path. `get_symbol(path="Cargo.toml", name="dependencies.serde")` works.
- `get_file_context` — produces outline from indexed symbols. TOML files show key hierarchy.
- `search_text` — searches raw content with enclosing symbol context.
- `get_file_content` — `around_symbol` resolves to byte range.

### Edit Tools — Capability-Gated

Edit tools require a small change: check the file's `EditCapability` before operating.

- `edit_within_symbol` — allowed for `TextEditSafe` and above. Scoped find-and-replace within byte range. Works for all config formats.
- `replace_symbol_body` — allowed for `StructuralEditSafe` only (TOML, .env). For other formats, returns a warning suggesting `edit_within_symbol` or raw Edit instead. LLM is responsible for valid replacement content.
- `delete_symbol` — allowed for `StructuralEditSafe` only. **Known limitation for future JSON support**: deleting a JSON key may leave trailing commas. A future Phase 2 enhancement could add JSON-aware comma cleanup and upgrade JSON to `StructuralEditSafe`.

### YAML Edit Behavior

YAML edits are explicitly best-effort in v1. Line-based offset calculation works for simple key-value pairs but may produce incorrect ranges for:
- Block/folded scalars
- Anchors and aliases
- Indentation-sensitive multi-line values

Tools warn when operating on YAML heuristic ranges. For complex YAML structures, users should fall back to raw text edits.

### PreToolUse Hook Update

After shipping, update `is_non_source_path` in `src/cli/hook.rs` to remove `.json`, `.toml`, `.yaml`, `.yml`, `.md`, `.env` from the skip list so the PreToolUse hook starts suggesting SymForge for these files.

## Dependencies

| Crate | Status | Purpose |
|-------|--------|---------|
| `serde_json` | Already in deps | JSON parsing |
| `toml_edit` | Already in deps | TOML parsing with span preservation |
| `serde_yml` | **New** (~50KB) | YAML parsing (`serde_yaml` is deprecated; `serde_yml` is the maintained successor) |

No new deps for Markdown or .env (regex/line scan).

## Testing Strategy

### Unit tests (in config_extractors.rs)

Per extractor:
- **JSON**: nested objects → correct dot-paths, byte ranges. Depth limit at 6. Array indexing `[0]`..`[19]`, cap at 20.
- **TOML**: tables, inline tables, arrays of tables.
- **YAML**: mappings, sequences, multi-line values.
- **Markdown**: ATX headers levels 1-6, nesting, consecutive headers. Frontmatter (lines between opening and closing `---`) is ignored entirely — not parsed as YAML, not emitted as symbols.
- **.env**: KEY=value, quoted values, comments, blank lines, no-value keys.

### Integration tests (tests/config_files.rs)

- Index temp directory with config files, verify `search_symbols` finds keys.
- `get_symbol` on JSON key path returns correct content.
- `get_file_context` on TOML file returns structured outline.
- Simple YAML key replacement succeeds on representative cases (flat key-value pairs).
- Tools warn when operating on YAML heuristic ranges for complex structures.
- `replace_symbol_body` on TOML key (StructuralEditSafe) writes correct file.
- `replace_symbol_body` on JSON key is rejected with capability warning.
- File watcher picks up config file changes.
- **Update existing test**: `test_discover_files_ignores_json_md_toml` in `src/discovery/mod.rs` explicitly asserts these files are NOT discovered. This test must be updated to expect discovery of config files.

### Required regression tests

These must be named explicitly and cannot be skipped:

- **Duplicate Markdown headers disambiguate deterministically**: file with two `## Installation` headers produces `Installation` and `Installation#2`, both resolvable via `get_symbol`.
- **Literal-dot keys round-trip correctly**: JSON key `"a.b"` produces symbol `a~1b`. `get_symbol(name="a~1b")` returns correct content. `edit_within_symbol(name="a~1b", ...)` scopes to the correct byte range.
- **Literal-bracket keys round-trip correctly**: JSON key `"items[0]"` produces symbol `items~20~3`, distinct from structural array index `items[0]`.

### Edge cases

- Empty files → zero symbols, no crash.
- Malformed JSON/TOML/YAML → `FileOutcome::Failed { error }`, zero symbols, fail-open.
- Deeply nested (>6 levels) → symbols stop at depth 6.
- Large arrays (>20 items) → capped.
- Binary files with `.json` extension → detect and skip.

## Performance

No concern. Config files are tiny compared to source code. A project with 50 config files adds ~500 symbols to an index already containing 3000+. Parsing is sub-millisecond per file.

## Acceptance Criteria

### Read/Search/Navigation
- [ ] `search_symbols(name="dependencies")` finds TOML/JSON dependency keys
- [ ] `get_file_context(path="Cargo.toml")` returns structured outline of keys
- [ ] `get_file_content(path="README.md", around_symbol="Installation")` works
- [ ] `get_symbol(path="package.json", name="scripts.build")` returns the value
- [ ] File watcher re-indexes config files on change
- [ ] PreToolUse hook intercepts config files after this ships

### Edit Safety
- [ ] `edit_within_symbol` works on all TextEditSafe+ formats (JSON, YAML, Markdown, TOML, .env)
- [ ] `replace_symbol_body` works on StructuralEditSafe formats (TOML, .env)
- [ ] `replace_symbol_body` on TextEditSafe-only formats (JSON, YAML, Markdown) returns capability warning
- [ ] YAML edit tools warn when operating on heuristic ranges

### Correctness
- [ ] Malformed files fail-open with FileOutcome::Failed, zero symbols
- [ ] Existing discovery test updated to expect config file discovery
- [ ] Duplicate Markdown headers disambiguate deterministically (`Installation`, `Installation#2`)
- [ ] Literal-dot keys escape correctly (`"a.b"` → `a~1b`) and round-trip through symbol lookup and edits
- [ ] Literal-bracket keys escape correctly (`"items[0]"` → `items~20~3`), distinct from structural array index `items[0]`
