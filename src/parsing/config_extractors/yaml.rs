use super::{
    ConfigExtractor, EditCapability, ExtractionOutcome, ExtractionResult, MAX_ARRAY_ITEMS,
    MAX_DEPTH, join_array_index, join_key_path,
};
use crate::domain::{SymbolKind, SymbolRecord};

use super::{optional_u32, parse_diagnostic};

pub struct YamlExtractor;

impl ConfigExtractor for YamlExtractor {
    fn extract(&self, content: &[u8]) -> ExtractionResult {
        if content.is_empty() {
            return ExtractionResult {
                symbols: vec![],
                outcome: ExtractionOutcome::Ok,
            };
        }

        let value: serde_yml::Value = match serde_yml::from_slice(content) {
            Ok(v) => v,
            Err(e) => {
                let location = e.location();
                return ExtractionResult {
                    symbols: vec![],
                    outcome: ExtractionOutcome::Failed(parse_diagnostic(
                        "serde_yml",
                        e.to_string(),
                        location.as_ref().and_then(|loc| optional_u32(loc.line())),
                        location.as_ref().and_then(|loc| optional_u32(loc.column())),
                        None,
                        false,
                    )),
                };
            }
        };

        // Build a line-start offset table for line_range computation.
        let line_starts = build_line_starts(content);

        let mut symbols = Vec::new();
        let mut sort_order: u32 = 0;

        match &value {
            serde_yml::Value::Mapping(map) => {
                walk_mapping(
                    content,
                    &line_starts,
                    map,
                    "",
                    0,
                    &mut symbols,
                    &mut sort_order,
                );
            }
            serde_yml::Value::Sequence(seq) => {
                walk_sequence(
                    content,
                    &line_starts,
                    seq,
                    "",
                    0,
                    &mut symbols,
                    &mut sort_order,
                );
            }
            _ => {}
        }

        ExtractionResult {
            symbols,
            outcome: ExtractionOutcome::Ok,
        }
    }

    fn edit_capability(&self) -> EditCapability {
        EditCapability::TextEditSafe
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a table mapping line index → byte offset of line start.
fn build_line_starts(content: &[u8]) -> Vec<u32> {
    let mut starts: Vec<u32> = vec![0];
    for (i, &b) in content.iter().enumerate() {
        if b == b'\n' {
            starts.push((i + 1) as u32);
        }
    }
    starts
}

/// Convert a byte offset into a 0-based line number.
fn byte_to_line(line_starts: &[u32], offset: u32) -> u32 {
    match line_starts.binary_search(&offset) {
        Ok(idx) => idx as u32,
        Err(idx) => (idx.saturating_sub(1)) as u32,
    }
}

/// Extract a plain string from a YAML Value if it is a string/number/bool scalar.
fn value_as_key_str(v: &serde_yml::Value) -> Option<String> {
    match v {
        serde_yml::Value::String(s) => Some(s.clone()),
        serde_yml::Value::Number(n) => Some(n.to_string()),
        serde_yml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Find the byte range in `content` for the YAML mapping key `key`.
///
/// Searches for `key:` pattern starting from `search_from`, requiring the
/// match to appear at the start of a line (preceded only by spaces/tabs).
/// The range extends from the key start to just before the next sibling key
/// at the same or lesser indentation, or to end of content.
fn find_yaml_key_range(content: &[u8], key: &str, search_from: &mut usize) -> (usize, usize) {
    let needle = format!("{}:", key);
    let needle_bytes = needle.as_bytes();

    let mut pos = *search_from;
    while pos + needle_bytes.len() <= content.len() {
        if let Some(rel) = find_substring(&content[pos..], needle_bytes) {
            let abs_start = pos + rel;

            // Scan back to find the line start; only whitespace should precede the key.
            let line_start = content[..abs_start]
                .iter()
                .rposition(|&b| b == b'\n')
                .map(|p| p + 1)
                .unwrap_or(0);

            let prefix = &content[line_start..abs_start];
            let is_line_start = prefix.iter().all(|&b| b == b' ' || b == b'\t');

            if is_line_start {
                let indent = prefix.len();
                let key_end = abs_start + needle_bytes.len();
                let range_end = find_block_end(content, key_end, indent);

                *search_from = key_end;
                return (abs_start, range_end);
            }

            pos = abs_start + 1;
        } else {
            break;
        }
    }

    // Fallback: return whole remaining content
    (*search_from, content.len())
}

/// Given that a key was found ending at `after_key`, scan forward line-by-line
/// to find where its block ends — i.e., the start of the next line at
/// indentation <= `key_indent` that contains non-whitespace, non-comment content.
fn find_block_end(content: &[u8], after_key: usize, key_indent: usize) -> usize {
    let mut i = after_key;

    // Skip to end of the current key line.
    while i < content.len() && content[i] != b'\n' {
        i += 1;
    }
    if i < content.len() {
        i += 1; // consume the newline
    }

    while i < content.len() {
        let line_start = i;
        let mut indent = 0usize;
        while i < content.len() && (content[i] == b' ' || content[i] == b'\t') {
            indent += 1;
            i += 1;
        }

        if i >= content.len() {
            return content.len();
        }

        let ch = content[i];

        // Blank lines — continue scanning.
        if ch == b'\n' || ch == b'\r' {
            i += 1;
            continue;
        }

        // Comment lines — continue scanning.
        if ch == b'#' {
            while i < content.len() && content[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        // If this line's indentation is <= the key's indentation,
        // this is a sibling or parent — the block ends here.
        if indent <= key_indent {
            return line_start;
        }

        // Child line — skip it.
        while i < content.len() && content[i] != b'\n' {
            i += 1;
        }
        if i < content.len() {
            i += 1;
        }
    }

    content.len()
}

/// Find the first occurrence of `needle` in `haystack`.
fn find_substring(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Walk a YAML mapping, emitting a SymbolRecord per key and recursing.
fn walk_mapping(
    content: &[u8],
    line_starts: &[u32],
    map: &serde_yml::Mapping,
    parent_path: &str,
    depth: u32,
    symbols: &mut Vec<SymbolRecord>,
    sort_order: &mut u32,
) {
    let mut search_from: usize = 0;

    for (k, v) in map.iter() {
        let key_str = match value_as_key_str(k) {
            Some(s) => s,
            None => continue, // skip non-scalar keys
        };

        let key_path = join_key_path(parent_path, &key_str);

        let (byte_start, byte_end) = find_yaml_key_range(content, &key_str, &mut search_from);
        let byte_range = (byte_start as u32, byte_end as u32);

        let start_line = byte_to_line(line_starts, byte_start as u32);
        let end_line = byte_to_line(
            line_starts,
            (byte_end.saturating_sub(1).max(byte_start)) as u32,
        );

        symbols.push(SymbolRecord {
            name: key_path.clone(),
            kind: SymbolKind::Key,
            depth,
            sort_order: *sort_order,
            byte_range,
            item_byte_range: Some(byte_range),
            line_range: (start_line, end_line),
            doc_byte_range: None,
        });
        *sort_order += 1;

        if depth + 1 < MAX_DEPTH {
            match v {
                serde_yml::Value::Mapping(child_map) => {
                    walk_mapping(
                        content,
                        line_starts,
                        child_map,
                        &key_path,
                        depth + 1,
                        symbols,
                        sort_order,
                    );
                }
                serde_yml::Value::Sequence(child_seq) => {
                    walk_sequence(
                        content,
                        line_starts,
                        child_seq,
                        &key_path,
                        depth + 1,
                        symbols,
                        sort_order,
                    );
                }
                _ => {}
            }
        }
    }
}

/// Walk a YAML sequence, emitting a SymbolRecord per element and recursing.
fn walk_sequence(
    content: &[u8],
    line_starts: &[u32],
    seq: &[serde_yml::Value],
    parent_path: &str,
    depth: u32,
    symbols: &mut Vec<SymbolRecord>,
    sort_order: &mut u32,
) {
    for (i, v) in seq.iter().enumerate() {
        if i >= MAX_ARRAY_ITEMS {
            break;
        }

        let elem_path = join_array_index(parent_path, i);

        // Array elements in YAML don't have a searchable key pattern;
        // use the whole content span as a conservative range.
        let byte_start = 0u32;
        let byte_end = content.len() as u32;
        let byte_range = (byte_start, byte_end);
        let start_line = byte_to_line(line_starts, byte_start);
        let end_line = byte_to_line(line_starts, byte_end.saturating_sub(1).max(byte_start));

        symbols.push(SymbolRecord {
            name: elem_path.clone(),
            kind: SymbolKind::Key,
            depth,
            sort_order: *sort_order,
            byte_range,
            item_byte_range: Some(byte_range),
            line_range: (start_line, end_line),
            doc_byte_range: None,
        });
        *sort_order += 1;

        if depth + 1 < MAX_DEPTH {
            match v {
                serde_yml::Value::Mapping(child_map) => {
                    walk_mapping(
                        content,
                        line_starts,
                        child_map,
                        &elem_path,
                        depth + 1,
                        symbols,
                        sort_order,
                    );
                }
                serde_yml::Value::Sequence(child_seq) => {
                    walk_sequence(
                        content,
                        line_starts,
                        child_seq,
                        &elem_path,
                        depth + 1,
                        symbols,
                        sort_order,
                    );
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_mapping() {
        let content = b"name: test\nversion: 1.0\n";
        let result = YamlExtractor.extract(content);
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "name" && s.kind == SymbolKind::Key)
        );
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
        let result = YamlExtractor.extract(b":\n  :\n  - [invalid");
        assert!(result.symbols.is_empty());
        assert!(matches!(result.outcome, ExtractionOutcome::Failed(_)));
    }

    #[test]
    fn test_edit_capability() {
        assert_eq!(
            YamlExtractor.edit_capability(),
            EditCapability::TextEditSafe
        );
    }

    #[test]
    fn test_depth_limit() {
        let content =
            b"a:\n  b:\n    c:\n      d:\n        e:\n          f:\n            g: deep\n";
        let result = YamlExtractor.extract(content);
        assert!(result.symbols.iter().any(|s| s.name == "a.b.c.d.e.f"));
        assert!(!result.symbols.iter().any(|s| s.name == "a.b.c.d.e.f.g"));
    }

    #[test]
    fn test_array_cap() {
        let mut content = String::from("arr:\n");
        for i in 0..25 {
            content.push_str(&format!("  - {}\n", i));
        }
        let result = YamlExtractor.extract(content.as_bytes());
        let arr_items: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.name.starts_with("arr["))
            .collect();
        assert_eq!(arr_items.len(), 20);
    }

    #[test]
    fn test_byte_range_within_bounds() {
        let content = b"name: test\nversion: 1.0\n";
        let result = YamlExtractor.extract(content);
        for sym in &result.symbols {
            assert!(
                sym.byte_range.1 <= content.len() as u32,
                "symbol {} byte_range end {} exceeds file length {}",
                sym.name,
                sym.byte_range.1,
                content.len()
            );
        }
    }
}
