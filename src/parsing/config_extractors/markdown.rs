use super::{
    ConfigExtractor, EditCapability, ExtractionOutcome, ExtractionResult, escape_key_segment,
};
use crate::domain::{SymbolKind, SymbolRecord};
use std::collections::HashMap;

use super::parse_diagnostic;

pub struct MarkdownExtractor;

impl ConfigExtractor for MarkdownExtractor {
    fn extract(&self, content: &[u8]) -> ExtractionResult {
            let text = match std::str::from_utf8(content) {
                Ok(s) => s,
                Err(_) => {
                    return ExtractionResult {
                        symbols: vec![],
                        outcome: ExtractionOutcome::Failed(parse_diagnostic(
                            "utf-8",
                            "Invalid UTF-8",
                            None,
                            None,
                            None,
                            false,
                        )),
                    };
                }
            };

            if text.is_empty() {
                return ExtractionResult {
                    symbols: vec![],
                    outcome: ExtractionOutcome::Ok,
                };
            }

            // Collect (byte_offset, line_text) pairs, skipping YAML frontmatter.
            let mut lines: Vec<(usize, &str)> = Vec::new();
            {
                let raw: Vec<&str> = text.split('\n').collect();
                let mut i = 0usize;
                let mut byte_offset = 0usize;

                // Check for frontmatter
                if raw.first().copied() == Some("---") {
                    byte_offset += raw[0].len() + 1; // skip opening ---\n
                    i = 1;
                    let mut closed = false;
                    while i < raw.len() {
                        let line_bytes = raw[i].len() + 1;
                        if raw[i] == "---" {
                            byte_offset += line_bytes;
                            i += 1;
                            closed = true;
                            break;
                        }
                        byte_offset += line_bytes;
                        i += 1;
                    }
                    if !closed {
                        return ExtractionResult {
                            symbols: vec![],
                            outcome: ExtractionOutcome::Ok,
                        };
                    }
                }

                while i < raw.len() {
                    lines.push((byte_offset, raw[i]));
                    byte_offset += raw[i].len() + 1;
                    i += 1;
                }
            }

            // Parse ATX headers from collected lines.
            struct HeaderInfo {
                level: u32,
                text: String,
                byte_start: usize,
                line_index: usize,
            }

            let mut headers: Vec<HeaderInfo> = Vec::new();
            for (li, &(byte_off, line)) in lines.iter().enumerate() {
                if !line.starts_with('#') {
                    continue;
                }
                let hashes = line.bytes().take_while(|&b| b == b'#').count();
                if hashes > 6 {
                    continue;
                }
                let rest = &line[hashes..];
                if let Some(title) = rest.strip_prefix(' ') {
                    headers.push(HeaderInfo {
                        level: hashes as u32,
                        text: title.trim_end().to_string(),
                        byte_start: byte_off,
                        line_index: li,
                    });
                }
            }

            if headers.is_empty() {
                return ExtractionResult {
                    symbols: vec![],
                    outcome: ExtractionOutcome::Ok,
                };
            }

            let total_bytes = content.len() as u32;

            // Build symbols. Stack holds (level, escaped_segment) for path building.
            let mut stack: Vec<(u32, String)> = Vec::new();
            // Duplicate counter keyed by base path (before disambiguation suffix).
            let mut seen_paths: HashMap<String, u32> = HashMap::new();
            let mut symbols: Vec<SymbolRecord> = Vec::new();

            for (hi, header) in headers.iter().enumerate() {
                let level = header.level;

                // Pop entries at same or deeper level.
                while stack.last().is_some_and(|&(l, _)| l >= level) {
                    stack.pop();
                }

                // Build dot-joined path.
                let escaped = escape_key_segment(&header.text);
                let base_path = if stack.is_empty() {
                    escaped.clone()
                } else {
                    let parent: String = stack
                        .iter()
                        .map(|(_, n)| n.as_str())
                        .collect::<Vec<_>>()
                        .join(".");
                    format!("{}.{}", parent, escaped)
                };

                // Disambiguate duplicates.
                let count = seen_paths.entry(base_path.clone()).or_insert(0);
                *count += 1;
                let name = if *count == 1 {
                    base_path.clone()
                } else {
                    format!("{}#{}", base_path, count)
                };

                stack.push((level, escaped));

                // Byte range: this header's start → byte before next header at same or higher level (or EOF).
                let byte_end: u32 = headers[hi + 1..]
                    .iter()
                    .find(|h| h.level <= level)
                    .map_or(total_bytes, |h| h.byte_start as u32);

                // Line range: same logic using line indices.
                let line_start = header.line_index as u32;
                let line_end: u32 = headers[hi + 1..]
                    .iter()
                    .find(|h| h.level <= level)
                    .map_or(lines.len() as u32, |h| h.line_index as u32);

                symbols.push(SymbolRecord {
                    name,
                    kind: SymbolKind::Section,
                    depth: level - 1,
                    sort_order: hi as u32,
                    byte_range: (header.byte_start as u32, byte_end),
                    line_range: (line_start, line_end),
                    doc_byte_range: None,
                });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(content: &[u8]) -> Vec<SymbolRecord> {
        MarkdownExtractor.extract(content).symbols
    }

    #[test]
    fn test_single_header() {
        let syms = extract(b"# Title\nSome text\n");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Title");
        assert_eq!(syms[0].kind, SymbolKind::Section);
        assert_eq!(syms[0].depth, 0);
    }

    #[test]
    fn test_nested_headers() {
        let syms = extract(b"# Top\n## Sub\n### Deep\n");
        assert_eq!(syms.len(), 3);
        assert_eq!(syms[0].name, "Top");
        assert_eq!(syms[1].name, "Top.Sub");
        assert_eq!(syms[2].name, "Top.Sub.Deep");
    }

    #[test]
    fn test_section_byte_range_spans_to_next_header() {
        // "# A\n" = 4, "line1\n" = 6, "line2\n" = 6  → "# B" starts at byte 16
        let content = b"# A\nline1\nline2\n# B\nline3\n";
        let syms = extract(content);
        assert_eq!(syms.len(), 2);
        assert_eq!(syms[0].byte_range.0, 0);
        assert_eq!(syms[0].byte_range.1, 16);
        assert_eq!(syms[1].byte_range.0, 16);
        assert_eq!(syms[1].byte_range.1, content.len() as u32);
    }

    #[test]
    fn test_duplicate_headers_disambiguated() {
        let syms = extract(b"## Install\ntext\n## Install\ntext\n");
        assert_eq!(syms.len(), 2);
        assert_eq!(syms[0].name, "Install");
        assert_eq!(syms[1].name, "Install#2");
    }

    #[test]
    fn test_frontmatter_skipped() {
        let syms = extract(b"---\ntitle: Hello\n---\n# Real Header\n");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Real Header");
    }

    #[test]
    fn test_empty_file() {
        let syms = extract(b"");
        assert_eq!(syms.len(), 0);
    }

    #[test]
    fn test_edit_capability() {
        assert_eq!(
            MarkdownExtractor.edit_capability(),
            EditCapability::TextEditSafe
        );
    }
}
