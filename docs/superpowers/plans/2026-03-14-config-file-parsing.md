# Config File Parsing Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make JSON, TOML, YAML, Markdown, and .env files first-class in the LiveIndex via native Rust parsers, with capability-gated edits.

**Architecture:** Add 5 new `LanguageId` variants, 2 new `SymbolKind` variants, a `ConfigExtractor` trait with 5 implementations, and an `EditCapability` enum. Config files branch before the tree-sitter pipeline in `process_file_with_classification`. Edit tools check capability before operating.

**Tech Stack:** `serde_json` (existing), `toml_edit` (existing), `serde_yml` (new), regex for Markdown, line scan for .env.

**Spec:** `docs/superpowers/specs/2026-03-14-config-file-parsing-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/domain/index.rs` | Modify | Add LanguageId variants, SymbolKind variants, FileClassification.is_config |
| `src/parsing/config_extractors/mod.rs` | Create | ConfigExtractor trait, EditCapability enum, key escaping utils, registry |
| `src/parsing/config_extractors/env.rs` | Create | EnvExtractor |
| `src/parsing/config_extractors/markdown.rs` | Create | MarkdownExtractor |
| `src/parsing/config_extractors/json.rs` | Create | JsonExtractor |
| `src/parsing/config_extractors/toml.rs` | Create | TomlExtractor |
| `src/parsing/config_extractors/yaml.rs` | Create | YamlExtractor |
| `src/parsing/mod.rs` | Modify | Branch config types before tree-sitter |
| `src/protocol/tools.rs` | Modify | Edit capability gating |
| `src/cli/hook.rs` | Modify | Remove config extensions from is_non_source_path |
| `src/discovery/mod.rs` | Modify | Update existing test |
| `Cargo.toml` | Modify | Add serde_yml dependency |
| `tests/config_files.rs` | Create | Integration + regression tests |

---

## Chunk 1: Domain Model + Framework

### Task 1: Add LanguageId variants and extension mapping

**Files:**
- Modify: `src/domain/index.rs:7-24` (LanguageId enum)
- Modify: `src/domain/index.rs:27-47` (from_extension)
- Modify: `src/domain/index.rs:49-68` (extensions)
- Modify: `src/domain/index.rs:70-88` (support_tier)
- Modify: `src/domain/index.rs:90-112` (Display impl)

- [ ] **Step 1: Add 5 variants to LanguageId enum**

In `src/domain/index.rs`, add after `Elixir` (line 23):

```rust
    Json,
    Toml,
    Yaml,
    Markdown,
    Env,
```

- [ ] **Step 2: Add extension mapping in from_extension()**

Add these arms before the `_ => None` line (line 46):

```rust
            "json" => Some(Self::Json),
            "toml" => Some(Self::Toml),
            "yaml" | "yml" => Some(Self::Yaml),
            "md" => Some(Self::Markdown),
            "env" => Some(Self::Env),
```

- [ ] **Step 3: Add extensions() arms**

Add in the `extensions()` match:

```rust
            Self::Json => &["json"],
            Self::Toml => &["toml"],
            Self::Yaml => &["yaml", "yml"],
            Self::Markdown => &["md"],
            Self::Env => &["env"],
```

- [ ] **Step 4: Add support_tier() arms**

`SupportTier` currently has `QualityFocus`, `Broader`, `Unsupported`. Config languages should use `Broader` (they're supported but not tree-sitter-quality):

```rust
            Self::Json | Self::Toml | Self::Yaml | Self::Markdown | Self::Env => SupportTier::Broader,
```

- [ ] **Step 5: Add Display impl arms**

```rust
            Self::Json => write!(f, "JSON"),
            Self::Toml => write!(f, "TOML"),
            Self::Yaml => write!(f, "YAML"),
            Self::Markdown => write!(f, "Markdown"),
            Self::Env => write!(f, "Env"),
```

- [ ] **Step 6: Handle all other match arms on LanguageId**

Search for `match.*language` and `match.*lang` across the codebase. Every exhaustive match on `LanguageId` will fail to compile until the new variants are handled. Fix each one — typically config types should take the default/fallback branch.

- [ ] **Step 7: Compile check**

Run: `cargo check`
Expected: Compiler guides you to any remaining unhandled match arms.

- [ ] **Step 8: Commit**

```bash
git add src/domain/index.rs
git commit -m "feat: add Json/Toml/Yaml/Markdown/Env to LanguageId"
```

### Task 2: Add SymbolKind variants

**Files:**
- Modify: `src/domain/index.rs:248-260` (SymbolKind enum)
- Modify: `src/domain/index.rs:264-283` (SymbolKind Display impl)

- [ ] **Step 1: Add Key and Section variants**

After `Other` in the SymbolKind enum:

```rust
    Key,
    Section,
```

- [ ] **Step 2: Add Display arms**

```rust
            Self::Key => write!(f, "key"),
            Self::Section => write!(f, "section"),
```

- [ ] **Step 3: Handle all other match arms on SymbolKind**

Search for exhaustive matches on `SymbolKind` across the codebase. Add the new variants to each — they typically belong in the same branch as `Other`.

- [ ] **Step 4: Compile check**

Run: `cargo check`

- [ ] **Step 5: Commit**

```bash
git add src/domain/index.rs
git commit -m "feat: add Key and Section to SymbolKind"
```

### Task 3: Add is_config to FileClassification

**Files:**
- Modify: `src/domain/index.rs:128-134` (FileClassification struct)
- Modify: `src/domain/index.rs:137-190` (for_code_path)

- [ ] **Step 1: Add field**

Add after `is_vendor` in the struct:

```rust
    pub is_config: bool,
```

- [ ] **Step 2: Fix all struct construction sites**

Search for `FileClassification {` across the codebase. Add `is_config: false` to every existing construction. Then in `for_code_path`, add detection for config extensions:

```rust
let is_config = matches!(
    ext,
    "json" | "toml" | "yaml" | "yml" | "md" | "env"
);
```

And set `is_config` in the returned struct.

- [ ] **Step 3: Compile check and run tests**

Run: `cargo check && cargo test --lib domain -- --test-threads=1`

- [ ] **Step 4: Commit**

```bash
git add src/domain/index.rs
git commit -m "feat: add is_config field to FileClassification"
```

### Task 4: Create ConfigExtractor trait and key escaping

**Files:**
- Create: `src/parsing/config_extractors/mod.rs` (module root — must be directory-based since sub-modules live alongside)

- [ ] **Step 1: Write the module with trait, enum, escaping, and registry**

Create `src/parsing/config_extractors/mod.rs`:

```rust
//! Config file extractors — produce SymbolRecord from structured non-code files.

pub mod env;
pub mod json;
pub mod markdown;
pub mod toml;
pub mod yaml;

use crate::domain::{LanguageId, SymbolRecord};

/// Edit safety level for config file types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditCapability {
    /// Read/search/navigation only, no edits via Tokenizor.
    IndexOnly,
    /// edit_within_symbol (scoped find-replace) is safe.
    TextEditSafe,
    /// replace_symbol_body, delete_symbol are safe.
    StructuralEditSafe,
}

/// Result of config extraction — separates success from parse failure.
pub struct ExtractionResult {
    pub symbols: Vec<SymbolRecord>,
    pub outcome: ExtractionOutcome,
}

pub enum ExtractionOutcome {
    /// Parsed successfully.
    Ok,
    /// Parse failed — error message for diagnostics.
    Failed(String),
}

/// Trait for config file parsers. Each format implements this.
pub trait ConfigExtractor: Send + Sync {
    fn extract(&self, content: &[u8]) -> ExtractionResult;
    fn edit_capability(&self) -> EditCapability;
}

/// Returns true if the language is a config type handled by config extractors.
pub fn is_config_language(language: &LanguageId) -> bool {
    matches!(
        language,
        LanguageId::Json
            | LanguageId::Toml
            | LanguageId::Yaml
            | LanguageId::Markdown
            | LanguageId::Env
    )
}

/// Get the extractor for a config language.
pub fn extractor_for(language: &LanguageId) -> Option<Box<dyn ConfigExtractor>> {
    match language {
        LanguageId::Json => Some(Box::new(json::JsonExtractor)),
        LanguageId::Toml => Some(Box::new(toml::TomlExtractor)),
        LanguageId::Yaml => Some(Box::new(yaml::YamlExtractor)),
        LanguageId::Markdown => Some(Box::new(markdown::MarkdownExtractor)),
        LanguageId::Env => Some(Box::new(env::EnvExtractor)),
        _ => None,
    }
}

/// Get the edit capability for a language. Returns None for non-config languages.
pub fn edit_capability_for(language: &LanguageId) -> Option<EditCapability> {
    extractor_for(language).map(|e| e.edit_capability())
}

// ---------------------------------------------------------------------------
// Key escaping (JSON-Pointer-style)
// ---------------------------------------------------------------------------

/// Escape a single key segment for use in dot-joined symbol names.
///
/// Rules:
/// - `~` → `~0`
/// - `.` → `~1`
/// - `[` → `~2`
/// - `]` → `~3`
pub fn escape_key_segment(raw: &str) -> String {
    if !raw.contains(['~', '.', '[', ']']) {
        return raw.to_string();
    }
    raw.replace('~', "~0")
        .replace('.', "~1")
        .replace('[', "~2")
        .replace(']', "~3")
}

/// Join parent path and child key segment into a dot-joined symbol name.
pub fn join_key_path(parent: &str, child: &str) -> String {
    let escaped = escape_key_segment(child);
    if parent.is_empty() {
        escaped
    } else {
        format!("{parent}.{escaped}")
    }
}

/// Format an array index path: `parent[index]`.
pub fn join_array_index(parent: &str, index: usize) -> String {
    format!("{parent}[{index}]")
}

/// Maximum nesting depth for structured config files.
pub const MAX_DEPTH: u32 = 6;

/// Maximum array items to index.
pub const MAX_ARRAY_ITEMS: usize = 20;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_no_special_chars() {
        assert_eq!(escape_key_segment("name"), "name");
    }

    #[test]
    fn test_escape_dot() {
        assert_eq!(escape_key_segment("a.b"), "a~1b");
    }

    #[test]
    fn test_escape_tilde() {
        assert_eq!(escape_key_segment("a~b"), "a~0b");
    }

    #[test]
    fn test_escape_brackets() {
        assert_eq!(escape_key_segment("items[0]"), "items~20~3");
    }

    #[test]
    fn test_escape_all_special() {
        assert_eq!(escape_key_segment("a.b[~]"), "a~1b~2~0~3");
    }

    #[test]
    fn test_join_key_path_root() {
        assert_eq!(join_key_path("", "name"), "name");
    }

    #[test]
    fn test_join_key_path_nested() {
        assert_eq!(join_key_path("scripts", "test"), "scripts.test");
    }

    #[test]
    fn test_join_key_path_escaped() {
        assert_eq!(join_key_path("x", "a.b"), "x.a~1b");
    }

    #[test]
    fn test_join_array_index() {
        assert_eq!(join_array_index("items", 3), "items[3]");
    }
}
```

- [ ] **Step 2: Register module in src/parsing/mod.rs**

Add at the top of `src/parsing/mod.rs`:

```rust
pub mod config_extractors;
```

- [ ] **Step 3: Compile and run tests**

Run: `cargo test --lib parsing::config_extractors -- --test-threads=1`
Expected: All escaping tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/parsing/config_extractors/ src/parsing/mod.rs
git commit -m "feat: add ConfigExtractor trait, EditCapability enum, key escaping"
```

---

## Chunk 2: Extractors (.env, Markdown, JSON)

### Task 5: .env Extractor

**Files:**
- Create: `src/parsing/config_extractors/env.rs`

- [ ] **Step 1: Write failing test**

At the bottom of `env.rs`, add tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SymbolKind;

    #[test]
    fn test_basic_key_value() {
        let content = b"DATABASE_URL=postgres://localhost/db\nPORT=3000\n";
        let extractor = EnvExtractor;
        let result = extractor.extract(content);
        assert_eq!(result.symbols.len(), 2);
        assert_eq!(result.symbols[0].name, "DATABASE_URL");
        assert_eq!(result.symbols[0].kind, SymbolKind::Variable);
        assert_eq!(result.symbols[1].name, "PORT");
    }

    #[test]
    fn test_comments_and_blanks_skipped() {
        let content = b"# comment\n\nKEY=value\n";
        let result = EnvExtractor.extract(content);
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "KEY");
    }

    #[test]
    fn test_no_value_key() {
        let content = b"EMPTY_KEY=\n";
        let result = EnvExtractor.extract(content);
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "EMPTY_KEY");
    }

    #[test]
    fn test_quoted_value() {
        let content = b"SECRET=\"hello world\"\n";
        let result = EnvExtractor.extract(content);
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "SECRET");
    }

    #[test]
    fn test_empty_file() {
        let result = EnvExtractor.extract(b"");
        assert!(result.symbols.is_empty());
    }

    #[test]
    fn test_byte_ranges_cover_full_line() {
        let content = b"A=1\nB=2\n";
        let result = EnvExtractor.extract(content);
        let a_range = result.symbols[0].byte_range;
        assert_eq!(&content[a_range.0 as usize..a_range.1 as usize], b"A=1");
    }

    #[test]
    fn test_edit_capability() {
        assert_eq!(EnvExtractor.edit_capability(), EditCapability::StructuralEditSafe);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib parsing::config_extractors::env -- --test-threads=1`
Expected: FAIL — EnvExtractor not defined.

- [ ] **Step 3: Write implementation**

```rust
//! .env file extractor — KEY=value lines as Variable symbols.

use crate::domain::{SymbolKind, SymbolRecord};
use super::{ConfigExtractor, EditCapability, ExtractionResult, ExtractionOutcome};

pub struct EnvExtractor;

impl ConfigExtractor for EnvExtractor {
    fn extract(&self, content: &[u8]) -> ExtractionResult {
        let text = String::from_utf8_lossy(content);
        let mut symbols = Vec::new();
        let mut sort_order: u32 = 0;

        // Build a line-start index table for accurate byte ranges.
        // This handles both LF and CRLF line endings correctly.
        let mut line_starts: Vec<u32> = vec![0];
        for (i, byte) in content.iter().enumerate() {
            if *byte == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }

        for (line_num, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                if !key.is_empty() {
                    let line_start = line_starts[line_num];
                    // Line content length (without line ending)
                    let line_content_len = line.trim_end_matches(['\r', '\n']).len() as u32;
                    symbols.push(SymbolRecord {
                        name: key.to_string(),
                        kind: SymbolKind::Variable,
                        depth: 0,
                        sort_order,
                        byte_range: (line_start, line_start + line_content_len),
                        line_range: (line_num as u32, line_num as u32),
                        doc_byte_range: None,
                    });
                    sort_order += 1;
                }
            }
        }

        ExtractionResult {
            symbols,
            outcome: ExtractionOutcome::Ok,
        }
    }

    fn edit_capability(&self) -> EditCapability {
        EditCapability::StructuralEditSafe
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib parsing::config_extractors::env -- --test-threads=1`
Expected: All pass. If byte range test fails, adjust offset calculation.

- [ ] **Step 5: Commit**

```bash
git add src/parsing/config_extractors/env.rs
git commit -m "feat: add .env file extractor"
```

### Task 6: Markdown Extractor

**Files:**
- Create: `src/parsing/config_extractors/markdown.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SymbolKind;

    #[test]
    fn test_single_header() {
        let content = b"# Title\nSome text\n";
        let result = MarkdownExtractor.extract(content);
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "Title");
        assert_eq!(result.symbols[0].kind, SymbolKind::Section);
        assert_eq!(result.symbols[0].depth, 0);
    }

    #[test]
    fn test_nested_headers() {
        let content = b"# Top\n## Sub\n### Deep\n";
        let result = MarkdownExtractor.extract(content);
        assert_eq!(result.symbols.len(), 3);
        assert_eq!(result.symbols[0].name, "Top");
        assert_eq!(result.symbols[1].name, "Top.Sub");
        assert_eq!(result.symbols[2].name, "Top.Sub.Deep");
    }

    #[test]
    fn test_section_byte_range_spans_to_next_header() {
        let content = b"# A\nline1\nline2\n# B\nline3\n";
        let result = MarkdownExtractor.extract(content);
        assert_eq!(result.symbols.len(), 2);
        // Section A spans from "# A" to just before "# B"
        let a_text = &content[result.symbols[0].byte_range.0 as usize..result.symbols[0].byte_range.1 as usize];
        assert!(a_text.starts_with(b"# A"));
        assert!(!a_text.contains(&b'B'));
    }

    #[test]
    fn test_duplicate_headers_disambiguated() {
        let content = b"## Install\ntext\n## Install\ntext\n";
        let result = MarkdownExtractor.extract(content);
        assert_eq!(result.symbols[0].name, "Install");
        assert_eq!(result.symbols[1].name, "Install#2");
    }

    #[test]
    fn test_frontmatter_skipped() {
        let content = b"---\ntitle: Hello\n---\n# Real Header\n";
        let result = MarkdownExtractor.extract(content);
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "Real Header");
    }

    #[test]
    fn test_empty_file() {
        assert!(MarkdownExtractor.extract(b"").symbols.is_empty());
    }

    #[test]
    fn test_edit_capability() {
        assert_eq!(MarkdownExtractor.edit_capability(), EditCapability::TextEditSafe);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib parsing::config_extractors::markdown -- --test-threads=1`

- [ ] **Step 3: Write implementation**

Implement `MarkdownExtractor`:
- Scan lines for ATX headers (`^#{1,6} `)
- Skip YAML frontmatter (between opening `---` line 0 and closing `---`)
- Track header stack for nesting (dot-joined parent path based on header level)
- **Escape header text segments** using `escape_key_segment()` before joining — header text like `A.B` or `C[1]` would otherwise create ambiguous paths
- Detect duplicate headers at same level, append `#2`, `#3`
- Byte range: from header line start to byte before next same-or-higher-level header (or EOF)
- `edit_capability()` returns `TextEditSafe`
- Return `ExtractionResult` with `ExtractionOutcome::Ok`

- [ ] **Step 4: Run tests**

Run: `cargo test --lib parsing::config_extractors::markdown -- --test-threads=1`

- [ ] **Step 5: Commit**

```bash
git add src/parsing/config_extractors/markdown.rs
git commit -m "feat: add Markdown section extractor"
```

### Task 7: JSON Extractor

**Files:**
- Create: `src/parsing/config_extractors/json.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SymbolKind;

    #[test]
    fn test_top_level_keys() {
        let content = br#"{"name": "test", "version": "1.0"}"#;
        let result = JsonExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "name" && s.kind == SymbolKind::Key));
        assert!(result.symbols.iter().any(|s| s.name == "version"));
    }

    #[test]
    fn test_nested_keys() {
        let content = br#"{"scripts": {"test": "jest", "build": "tsc"}}"#;
        let result = JsonExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "scripts"));
        assert!(result.symbols.iter().any(|s| s.name == "scripts.test"));
        assert!(result.symbols.iter().any(|s| s.name == "scripts.build"));
    }

    #[test]
    fn test_array_indexing() {
        let content = br#"{"items": ["a", "b", "c"]}"#;
        let result = JsonExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "items[0]"));
        assert!(result.symbols.iter().any(|s| s.name == "items[2]"));
    }

    #[test]
    fn test_depth_limit() {
        // 7 levels deep — should stop at 6
        let content = br#"{"a":{"b":{"c":{"d":{"e":{"f":{"g":"deep"}}}}}}}"#;
        let result = JsonExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "a.b.c.d.e.f"));
        assert!(!result.symbols.iter().any(|s| s.name == "a.b.c.d.e.f.g"));
    }

    #[test]
    fn test_array_cap() {
        let items: Vec<String> = (0..25).map(|i| format!("{i}")).collect();
        let content = format!(r#"{{"arr": [{}]}}"#, items.join(","));
        let result = JsonExtractor.extract(content.as_bytes());
        let arr_items: Vec<_> = result.symbols.iter().filter(|s| s.name.starts_with("arr[")).collect();
        assert_eq!(arr_items.len(), 20); // capped at 20
    }

    #[test]
    fn test_literal_dot_key_escaped() {
        let content = br#"{"a.b": "value"}"#;
        let result = JsonExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "a~1b"));
    }

    #[test]
    fn test_literal_bracket_key_escaped() {
        let content = br#"{"items[0]": "literal"}"#;
        let result = JsonExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "items~20~3"));
    }

    #[test]
    fn test_empty_object() {
        let result = JsonExtractor.extract(b"{}");
        assert!(result.symbols.is_empty());
    }

    #[test]
    fn test_malformed_json() {
        let result = JsonExtractor.extract(b"{invalid json");
        assert!(result.symbols.is_empty());
        assert!(matches!(result.outcome, ExtractionOutcome::Failed(_)));
    }

    #[test]
    fn test_byte_range_with_escaped_quotes_and_whitespace() {
        // Dense formatting with escaped quotes — byte range finder must handle this
        let content = br#"{ "msg" : "hello \"world\"" , "num" : 42 }"#;
        let symbols = JsonExtractor.extract(content);
        let msg = symbols.symbols.iter().find(|s| s.name == "msg").expect("should find msg");
        let range_text = &content[msg.byte_range.0 as usize..msg.byte_range.1 as usize];
        // Range should contain the key and its value
        assert!(std::str::from_utf8(range_text).unwrap().contains("hello"));
    }

    #[test]
    fn test_byte_range_multiline_formatted() {
        let content = b"{\n  \"name\": \"test\",\n  \"version\": \"1.0\"\n}";
        let symbols = JsonExtractor.extract(content);
        let name = symbols.symbols.iter().find(|s| s.name == "name").expect("should find name");
        // Byte range must be within file bounds
        assert!(name.byte_range.1 <= content.len() as u32);
    }

    #[test]
    fn test_edit_capability() {
        assert_eq!(JsonExtractor.edit_capability(), EditCapability::TextEditSafe);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib parsing::config_extractors::json -- --test-threads=1`

- [ ] **Step 3: Write implementation**

Implement `JsonExtractor`:
- Parse with `serde_json::from_slice` → `serde_json::Value`
- Walk the Value tree recursively, tracking byte offsets by scanning raw content for key positions
- Emit `SymbolRecord` for each key with dot-joined path using `join_key_path`/`join_array_index`
- Respect `MAX_DEPTH` (6) and `MAX_ARRAY_ITEMS` (20)
- Malformed JSON → return empty vec (fail-open)
- `edit_capability()` returns `TextEditSafe`

**Byte range strategy**: After parsing the Value tree, scan the raw bytes to find the byte offset of each key string. The range covers from the key's opening quote to the end of its value (closing quote/brace/bracket).

- [ ] **Step 4: Run tests**

Run: `cargo test --lib parsing::config_extractors::json -- --test-threads=1`

- [ ] **Step 5: Commit**

```bash
git add src/parsing/config_extractors/json.rs
git commit -m "feat: add JSON key-path extractor"
```

---

## Chunk 3: Extractors (TOML, YAML)

### Task 8: TOML Extractor

**Files:**
- Create: `src/parsing/config_extractors/toml.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SymbolKind;

    #[test]
    fn test_top_level_keys() {
        let content = b"name = \"test\"\nversion = \"1.0\"\n";
        let result = TomlExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "name" && s.kind == SymbolKind::Key));
        assert!(result.symbols.iter().any(|s| s.name == "version"));
    }

    #[test]
    fn test_table_keys() {
        let content = b"[package]\nname = \"test\"\nversion = \"1.0\"\n";
        let result = TomlExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "package"));
        assert!(result.symbols.iter().any(|s| s.name == "package.name"));
        assert!(result.symbols.iter().any(|s| s.name == "package.version"));
    }

    #[test]
    fn test_nested_tables() {
        let content = b"[dependencies]\nserde = \"1.0\"\n\n[dependencies.serde]\nfeatures = [\"derive\"]\n";
        let result = TomlExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "dependencies.serde"));
    }

    #[test]
    fn test_inline_table() {
        let content = b"[package]\nmetadata = { key = \"value\" }\n";
        let result = TomlExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "package.metadata"));
        assert!(result.symbols.iter().any(|s| s.name == "package.metadata.key"));
    }

    #[test]
    fn test_empty_file() {
        assert!(TomlExtractor.extract(b"").symbols.is_empty());
    }

    #[test]
    fn test_malformed_toml() {
        assert!(TomlExtractor.extract(b"[invalid\nno closing").symbols.is_empty());
    }

    #[test]
    fn test_edit_capability() {
        assert_eq!(TomlExtractor.edit_capability(), EditCapability::StructuralEditSafe);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib parsing::config_extractors::toml -- --test-threads=1`

- [ ] **Step 3: Write implementation**

Implement `TomlExtractor`:
- Parse with `toml_edit::DocumentMut::from_str()`
- Walk the document tree recursively
- Use `Item::span()` (`Option<Range<usize>>`) for byte ranges — skip items where `span()` returns `None`
- Emit `SymbolRecord` for each key with dot-joined path
- Respect `MAX_DEPTH` (6)
- `edit_capability()` returns `StructuralEditSafe`

- [ ] **Step 4: Run tests**

Run: `cargo test --lib parsing::config_extractors::toml -- --test-threads=1`

- [ ] **Step 5: Commit**

```bash
git add src/parsing/config_extractors/toml.rs
git commit -m "feat: add TOML key-path extractor"
```

### Task 9: YAML Extractor + serde_yml dependency

**Files:**
- Modify: `Cargo.toml` (add serde_yml)
- Create: `src/parsing/config_extractors/yaml.rs`

- [ ] **Step 1: Add serde_yml to Cargo.toml**

Add to `[dependencies]`:

```toml
serde_yml = "0.0.12"
```

Run: `cargo check` to verify the dependency resolves.

- [ ] **Step 2: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SymbolKind;

    #[test]
    fn test_simple_mapping() {
        let content = b"name: test\nversion: 1.0\n";
        let result = YamlExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "name" && s.kind == SymbolKind::Key));
        assert!(result.symbols.iter().any(|s| s.name == "version"));
    }

    #[test]
    fn test_nested_mapping() {
        let content = b"server:\n  host: localhost\n  port: 8080\n";
        let result = YamlExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "server"));
        assert!(result.symbols.iter().any(|s| s.name == "server.host"));
        assert!(result.symbols.iter().any(|s| s.name == "server.port"));
    }

    #[test]
    fn test_sequence() {
        let content = b"items:\n  - a\n  - b\n";
        let result = YamlExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "items[0]"));
        assert!(result.symbols.iter().any(|s| s.name == "items[1]"));
    }

    #[test]
    fn test_empty_file() {
        assert!(YamlExtractor.extract(b"").symbols.is_empty());
    }

    #[test]
    fn test_malformed_yaml() {
        assert!(YamlExtractor.extract(b":\n  :\n  - [invalid").symbols.is_empty());
    }

    #[test]
    fn test_edit_capability() {
        assert_eq!(YamlExtractor.edit_capability(), EditCapability::TextEditSafe);
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib parsing::config_extractors::yaml -- --test-threads=1`

- [ ] **Step 4: Write implementation**

Implement `YamlExtractor`:
- Parse with `serde_yml::from_slice` → `serde_yml::Value`
- Walk the Value tree recursively with line-based byte offset calculation
- For byte ranges: track line starts in the raw content, map YAML keys to their line positions
- Respect `MAX_DEPTH` (6) and `MAX_ARRAY_ITEMS` (20)
- Malformed YAML → return empty vec
- `edit_capability()` returns `TextEditSafe`

- [ ] **Step 5: Run tests**

Run: `cargo test --lib parsing::config_extractors::yaml -- --test-threads=1`

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/parsing/config_extractors/yaml.rs
git commit -m "feat: add YAML key-path extractor with serde_yml"
```

---

## Chunk 4: Pipeline Integration + Edit Gating

### Task 10: Integrate config extractors into parsing pipeline

**Files:**
- Modify: `src/parsing/mod.rs:26-44` (process_file_with_classification)

- [ ] **Step 1: Add config branch before tree-sitter**

In `process_file_with_classification`, before the `parse_source` call, add:

```rust
    // Config files use native parsers, not tree-sitter.
    if config_extractors::is_config_language(&language) {
        let result = config_extractors::extractor_for(&language)
            .map(|e| e.extract(bytes));
        let (symbols, outcome) = match result {
            Some(r) => {
                let outcome = match r.outcome {
                    config_extractors::ExtractionOutcome::Ok => FileOutcome::Processed,
                    config_extractors::ExtractionOutcome::Failed(err) => {
                        FileOutcome::Failed { error: err }
                    }
                };
                (r.symbols, outcome)
            }
            None => (vec![], FileOutcome::Processed),
        };
        // Match the exact fields of FileProcessingResult — check the struct definition
        // in src/domain/index.rs and mirror the same fields the tree-sitter path returns.
        // Do NOT include a `content` field — FileProcessingResult has no such field.
        return FileProcessingResult {
            relative_path: relative_path.to_string(),
            language,
            classification,
            symbols,
            outcome,
            byte_len,
            content_hash,
            references: vec![],
            alias_map: Default::default(),
        };
    }
```

**Malformed file contract**: Extractors return `ExtractionResult` with `ExtractionOutcome::Failed(error)` and empty symbols on parse failure. The pipeline maps this to `FileOutcome::Failed { error }`. The file is still indexed (watcher tracks it, `search_text` works on raw content) but has no navigable symbols — consistent with the spec.

- [ ] **Step 2: Add unreachable arms in parse_source**

The private `parse_source` function (in the same file) has an exhaustive match on `LanguageId` for tree-sitter parser selection. Since config types are branched away before `parse_source` is ever called, add a catch-all arm:

```rust
            LanguageId::Json | LanguageId::Toml | LanguageId::Yaml
            | LanguageId::Markdown | LanguageId::Env => {
                unreachable!("config languages are handled before parse_source")
            }
```

- [ ] **Step 3: Compile check**

Run: `cargo check`
Expected: Clean compilation. If `FileProcessingResult` fields don't match, check the actual struct definition and adjust.

- [ ] **Step 4: Run full test suite**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: Existing tests pass. The discovery test `test_discover_files_ignores_json_md_toml` will likely FAIL — that's expected and fixed in Task 13.

- [ ] **Step 4: Commit**

```bash
git add src/parsing/mod.rs
git commit -m "feat: branch config files before tree-sitter in parsing pipeline"
```

### Task 11: Add edit capability gating to tools

**Files:**
- Modify: `src/protocol/tools.rs` (replace_symbol_body, delete_symbol, edit_within_symbol handlers)

- [ ] **Step 1: Add capability check helper**

Add a helper function near the edit tool handlers:

```rust
fn check_edit_capability(
    language: &LanguageId,
    required: EditCapability,
    tool_name: &str,
) -> Option<String> {
    use crate::parsing::config_extractors::{edit_capability_for, EditCapability};
    if let Some(cap) = edit_capability_for(language) {
        let allowed = match required {
            EditCapability::IndexOnly => false,
            EditCapability::TextEditSafe => matches!(cap, EditCapability::TextEditSafe | EditCapability::StructuralEditSafe),
            EditCapability::StructuralEditSafe => matches!(cap, EditCapability::StructuralEditSafe),
        };
        if !allowed {
            return Some(format!(
                "This file type ({language}) does not support {tool_name}. Use edit_within_symbol or raw Edit instead."
            ));
        }
    }
    None // Non-config files (source code) → no restriction
}
```

- [ ] **Step 2: Gate replace_symbol_body**

At the start of the `replace_symbol_body` handler, after resolving the file, add:

```rust
if let Some(warning) = check_edit_capability(&file.language, EditCapability::StructuralEditSafe, "replace_symbol_body") {
    return Ok(warning);
}
```

- [ ] **Step 3: Gate delete_symbol**

Same pattern for `delete_symbol`.

- [ ] **Step 4: Gate edit_within_symbol**

Same pattern but with `EditCapability::TextEditSafe`.

- [ ] **Step 5: Compile and test**

Run: `cargo check && cargo test --lib protocol -- --test-threads=1`

- [ ] **Step 6: Commit**

```bash
git add src/protocol/tools.rs
git commit -m "feat: gate edit tools by config file EditCapability"
```

### Task 12: Update PreToolUse hook

**Files:**
- Modify: `src/cli/hook.rs:210-234` (is_non_source_path)

- [ ] **Step 1: Remove config extensions from skip list**

In `is_non_source_path`, remove these from the `non_source_exts` array:
`.md`, `.json`, `.toml`, `.yaml`, `.yml`, `.env`

Keep all other non-source extensions (`.txt`, `.xml`, `.csv`, `.lock`, etc.).

- [ ] **Step 2: Run hook tests**

Run: `cargo test --lib cli::hook -- --test-threads=1`
Expected: `test_is_non_source_path_detects_config_files` will FAIL — update it.

- [ ] **Step 3: Update test**

Update `test_is_non_source_path_detects_config_files` to no longer expect `.json`, `.toml`, `.md` as non-source. Add new test:

```rust
#[test]
fn test_is_non_source_path_allows_config_files() {
    assert!(!is_non_source_path("package.json"));
    assert!(!is_non_source_path("Cargo.toml"));
    assert!(!is_non_source_path("README.md"));
    assert!(!is_non_source_path(".env"));
    assert!(!is_non_source_path("config.yaml"));
}
```

- [ ] **Step 4: Commit**

```bash
git add src/cli/hook.rs
git commit -m "feat: PreToolUse hook now intercepts config files for Tokenizor"
```

---

## Chunk 5: Tests + Cleanup

### Task 13: Update existing discovery test

**Files:**
- Modify: `src/discovery/mod.rs:218-228`

- [ ] **Step 1: Find and update the test**

Rename `test_discover_files_ignores_json_md_toml` to `test_discover_files_includes_config_files`.

Change assertion: instead of asserting config files are NOT in the result, assert they ARE present with correct `LanguageId` and `is_config: true`.

- [ ] **Step 2: Run discovery tests**

Run: `cargo test --lib discovery -- --test-threads=1`

- [ ] **Step 3: Commit**

```bash
git add src/discovery/mod.rs
git commit -m "test: update discovery test to expect config file inclusion"
```

### Task 14: Integration and regression tests

**Files:**
- Create: `tests/config_files.rs`

- [ ] **Step 1: Write integration tests**

Create `tests/config_files.rs` with:

1. **Index config files test**: Create temp dir with `.json`, `.toml`, `.yaml`, `.md`, `.env` files. Index them. Verify `search_symbols` finds keys.
2. **get_symbol on JSON key path**: Verify `get_symbol(path="test.json", name="scripts.build")` returns correct content.
3. **get_file_context on TOML**: Verify structured outline returned.
4. **Edit capability gating**: Verify `replace_symbol_body` on JSON returns warning, on TOML succeeds.
5. **get_file_content around_symbol test**: `get_file_content(path="README.md", around_symbol="Installation")` returns the section content.
6. **Duplicate Markdown headers regression**: File with two `## Installation` → `Installation` and `Installation#2`, both resolvable.
6. **Literal-dot key regression**: JSON key `"a.b"` → symbol `a~1b`, round-trips through lookup.
7. **Literal-bracket key regression**: JSON key `"items[0]"` → symbol `items~20~3`, distinct from array index.
8. **File watcher picks up config changes**: Modify a `.toml` file after initial indexing, verify symbols update.
9. **YAML heuristic warning**: `edit_within_symbol` on a YAML file with multi-line block scalar includes a warning about heuristic ranges in the response.
10. **Malformed JSON produces Failed outcome**: Index a file with invalid JSON, verify `FileOutcome::Failed` with error message and zero symbols.

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test config_files -- --test-threads=1`

- [ ] **Step 3: Commit**

```bash
git add tests/config_files.rs
git commit -m "test: add config file integration and regression tests"
```

### Task 15: Full test suite + cleanup

- [ ] **Step 1: Run full test suite**

Run: `cargo test --all-targets -- --test-threads=1`
Expected: All tests pass.

- [ ] **Step 2: Run cargo fmt**

Run: `cargo fmt -- --check`
Fix any formatting issues.

- [ ] **Step 3: Final commit**

```bash
git add -A
git commit -m "style: fix formatting for config file parsing"
```

- [ ] **Step 4: Push**

```bash
git push origin main
```
