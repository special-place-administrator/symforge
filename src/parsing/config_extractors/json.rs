use super::{
    ConfigExtractor, EditCapability, ExtractionOutcome, ExtractionResult, MAX_ARRAY_ITEMS,
    MAX_DEPTH, join_array_index, join_key_path,
};
use crate::domain::{SymbolKind, SymbolRecord};

use super::{optional_u32, parse_diagnostic};

pub struct JsonExtractor;

/// Strip `//` line comments and `/* … */` block comments from JSON bytes,
/// producing valid JSON that `serde_json` can parse. String literals are
/// respected — comments inside `"…"` are left untouched. Newlines inside
/// block comments are preserved so that line numbers stay accurate.
fn strip_json_comments(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let len = input.len();
    let mut i = 0;

    while i < len {
        let b = input[i];

        // --- string literal: copy verbatim until closing quote ---
        if b == b'"' {
            out.push(b);
            i += 1;
            while i < len {
                let c = input[i];
                out.push(c);
                i += 1;
                if c == b'"' {
                    break;
                }
                if c == b'\\' && i < len {
                    // escaped char — copy next byte unconditionally
                    out.push(input[i]);
                    i += 1;
                }
            }
            continue;
        }

        // --- possible comment start ---
        if b == b'/' && i + 1 < len {
            let next = input[i + 1];

            // line comment: replace with spaces until newline
            if next == b'/' {
                i += 2; // skip "//"
                out.push(b' ');
                out.push(b' ');
                while i < len && input[i] != b'\n' {
                    out.push(b' ');
                    i += 1;
                }
                continue;
            }

            // block comment: replace with spaces, preserve newlines
            if next == b'*' {
                i += 2; // skip "/*"
                out.push(b' ');
                out.push(b' ');
                while i < len {
                    if input[i] == b'*' && i + 1 < len && input[i + 1] == b'/' {
                        out.push(b' ');
                        out.push(b' ');
                        i += 2;
                        break;
                    }
                    if input[i] == b'\n' {
                        out.push(b'\n');
                    } else {
                        out.push(b' ');
                    }
                    i += 1;
                }
                continue;
            }
        }

        // --- ordinary byte ---
        out.push(b);
        i += 1;
    }

    out
}
impl ConfigExtractor for JsonExtractor {
    fn extract(&self, content: &[u8]) -> ExtractionResult {
        let stripped = strip_json_comments(content);
        let value: serde_json::Value = match serde_json::from_slice(&stripped) {
            Ok(v) => v,
            Err(e) => {
                return ExtractionResult {
                    symbols: vec![],
                    outcome: ExtractionOutcome::Failed(parse_diagnostic(
                        "serde_json",
                        e.to_string(),
                        optional_u32(e.line()),
                        optional_u32(e.column()),
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

        // Only walk into the root if it is an object or array.
        match &value {
            serde_json::Value::Object(map) => {
                walk_object(
                    content,
                    &line_starts,
                    map,
                    "",
                    0,
                    &mut symbols,
                    &mut sort_order,
                );
            }
            serde_json::Value::Array(arr) => {
                walk_array(
                    content,
                    &line_starts,
                    arr,
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

fn walk_object(
    content: &[u8],
    line_starts: &[u32],
    map: &serde_json::Map<String, serde_json::Value>,
    parent_path: &str,
    depth: u32,
    symbols: &mut Vec<SymbolRecord>,
    sort_order: &mut u32,
) {
    // We need a search cursor so we scan forward through the raw bytes.
    // Start after the opening `{` of this object.
    let mut search_from: usize = 0;

    for (key, value) in map.iter() {
        let key_path = join_key_path(parent_path, key);

        // Find the byte range for this key-value pair in the raw content.
        let (byte_start, byte_end) = find_key_value_range(content, key, &mut search_from);
        let byte_range = (byte_start as u32, byte_end as u32);

        let start_line = byte_to_line(line_starts, byte_start as u32);
        let end_line = byte_to_line(
            line_starts,
            byte_end.saturating_sub(1).max(byte_start) as u32,
        );

        symbols.push(SymbolRecord {
            name: key_path.clone(),
            kind: SymbolKind::Key,
            depth,
            sort_order: *sort_order,
            byte_range,
            line_range: (start_line, end_line),
            doc_byte_range: None,
            item_byte_range: Some(byte_range),
        });
        *sort_order += 1;

        // Recurse if we haven't hit the depth limit.
        if depth + 1 < MAX_DEPTH {
            match value {
                serde_json::Value::Object(child_map) => {
                    walk_object(
                        content,
                        line_starts,
                        child_map,
                        &key_path,
                        depth + 1,
                        symbols,
                        sort_order,
                    );
                }
                serde_json::Value::Array(child_arr) => {
                    walk_array(
                        content,
                        line_starts,
                        child_arr,
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

fn walk_array(
    content: &[u8],
    line_starts: &[u32],
    arr: &[serde_json::Value],
    parent_path: &str,
    depth: u32,
    symbols: &mut Vec<SymbolRecord>,
    sort_order: &mut u32,
) {
    for (i, value) in arr.iter().enumerate() {
        if i >= MAX_ARRAY_ITEMS {
            break;
        }

        let elem_path = join_array_index(parent_path, i);

        // For array elements we don't have a key to search for, so we use a
        // simple (0,0) placeholder and then try to refine below.
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
            line_range: (start_line, end_line),
            doc_byte_range: None,
            item_byte_range: Some(byte_range),
        });
        *sort_order += 1;

        if depth + 1 < MAX_DEPTH {
            match value {
                serde_json::Value::Object(child_map) => {
                    walk_object(
                        content,
                        line_starts,
                        child_map,
                        &elem_path,
                        depth + 1,
                        symbols,
                        sort_order,
                    );
                }
                serde_json::Value::Array(child_arr) => {
                    walk_array(
                        content,
                        line_starts,
                        child_arr,
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

/// Search the raw bytes for `"key":` starting from `*search_from`, returning
/// the byte range covering the key and its associated value.
///
/// The start is the opening `"` of the key. The end is determined by scanning
/// past the value (tracking braces, brackets, and strings).
fn find_key_value_range(content: &[u8], key: &str, search_from: &mut usize) -> (usize, usize) {
    // Build the needle: `"key"` (we search for the quoted key).
    let needle = format!("\"{}\"", key);
    let needle_bytes = needle.as_bytes();

    // Search forward from the current cursor.
    let hay = &content[*search_from..];
    if let Some(rel_pos) = find_substring(hay, needle_bytes) {
        let abs_key_start = *search_from + rel_pos;

        // Find the colon after the key.
        let after_key = abs_key_start + needle_bytes.len();
        let colon_pos = match content[after_key..].iter().position(|&b| b == b':') {
            Some(p) => after_key + p,
            None => {
                // Fallback: return just the key span.
                let end = abs_key_start + needle_bytes.len();
                *search_from = end;
                return (abs_key_start, end);
            }
        };

        // Skip whitespace after the colon to find the value start.
        let value_start = skip_whitespace(content, colon_pos + 1);

        // Determine the value end.
        let value_end = scan_value_end(content, value_start);

        *search_from = value_end;
        (abs_key_start, value_end)
    } else {
        // Key not found (shouldn't happen for valid JSON). Return file bounds.
        (0, content.len())
    }
}

/// Find the first occurrence of `needle` in `haystack` (simple byte search).
fn find_substring(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Skip ASCII whitespace bytes, returning the index of the first non-WS byte.
fn skip_whitespace(content: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < content.len() && matches!(content[i], b' ' | b'\t' | b'\r' | b'\n') {
        i += 1;
    }
    i
}

/// Scan past a single JSON value (string, number, bool, null, object, array),
/// returning the byte position just past the value.
fn scan_value_end(content: &[u8], start: usize) -> usize {
    if start >= content.len() {
        return content.len();
    }

    match content[start] {
        b'"' => scan_string_end(content, start),
        b'{' | b'[' => scan_container_end(content, start),
        _ => {
            // Primitive: number, bool, null — ends at comma, `}`, `]`, or whitespace.
            let mut i = start;
            while i < content.len()
                && !matches!(
                    content[i],
                    b',' | b'}' | b']' | b' ' | b'\t' | b'\r' | b'\n'
                )
            {
                i += 1;
            }
            i
        }
    }
}

/// Scan past a JSON string (handling escape sequences).
fn scan_string_end(content: &[u8], start: usize) -> usize {
    // start points at the opening `"`.
    let mut i = start + 1;
    while i < content.len() {
        if content[i] == b'\\' {
            i += 2; // skip escaped char
        } else if content[i] == b'"' {
            return i + 1; // past the closing quote
        } else {
            i += 1;
        }
    }
    content.len()
}

/// Scan past a JSON object `{…}` or array `[…]`, tracking nesting and strings.
fn scan_container_end(content: &[u8], start: usize) -> usize {
    let open = content[start];
    let close = if open == b'{' { b'}' } else { b']' };

    let mut depth: u32 = 0;
    let mut i = start;
    while i < content.len() {
        match content[i] {
            b'"' => {
                // Skip string contents.
                i = scan_string_end(content, i);
                continue;
            }
            b if b == open => depth += 1,
            b if b == close => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    content.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_top_level_keys() {
        let content = br#"{"name": "test", "version": "1.0"}"#;
        let result = JsonExtractor.extract(content);
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "name" && s.kind == SymbolKind::Key)
        );
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
        let arr_items: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.name.starts_with("arr["))
            .collect();
        assert_eq!(arr_items.len(), 20);
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
    fn test_byte_range_within_bounds() {
        let content = b"{\n  \"name\": \"test\",\n  \"version\": \"1.0\"\n}";
        let result = JsonExtractor.extract(content);
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

    #[test]
    fn test_edit_capability() {
        assert_eq!(
            JsonExtractor.edit_capability(),
            EditCapability::TextEditSafe
        );
    }

    #[test]
    fn test_jsonc_line_comments() {
        let content = b"{\n  // This is a comment\n  \"name\": \"test\"\n}";
        let result = JsonExtractor.extract(content);
        assert!(
            matches!(result.outcome, ExtractionOutcome::Ok),
            "JSONC with line comments should parse OK"
        );
        assert!(result.symbols.iter().any(|s| s.name == "name"));
    }

    #[test]
    fn test_jsonc_block_comments() {
        let content = b"{\n  /* block comment */\n  \"name\": \"test\"\n}";
        let result = JsonExtractor.extract(content);
        assert!(
            matches!(result.outcome, ExtractionOutcome::Ok),
            "JSONC with block comments should parse OK"
        );
        assert!(result.symbols.iter().any(|s| s.name == "name"));
    }

    #[test]
    fn test_jsonc_trailing_commas_still_fail() {
        let content = br#"{"a": 1,}"#;
        let result = JsonExtractor.extract(content);
        assert!(
            matches!(result.outcome, ExtractionOutcome::Failed(_)),
            "Trailing commas should still fail"
        );
    }

    #[test]
    fn test_jsonc_comments_inside_strings_preserved() {
        let content = br#"{"url": "https://example.com", "pattern": "// not a comment"}"#;
        let result = JsonExtractor.extract(content);
        assert!(
            matches!(result.outcome, ExtractionOutcome::Ok),
            "Comments inside strings should not be stripped"
        );
        assert!(result.symbols.iter().any(|s| s.name == "url"));
        assert!(result.symbols.iter().any(|s| s.name == "pattern"));
    }
}
