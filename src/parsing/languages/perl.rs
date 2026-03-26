use tree_sitter::Node;

use super::{DocCommentSpec, SymbolSink, collect_symbols, push_named_symbol, walk_children};

pub(super) const DOC_SPEC: DocCommentSpec = DocCommentSpec {
    comment_node_types: &["comment"],
    doc_prefixes: None,
    custom_doc_check: None,
};
use crate::domain::{SymbolKind, SymbolRecord};

pub fn extract_symbols(node: &Node, source: &str) -> Vec<SymbolRecord> {
    collect_symbols(node, source, walk_node)
}

fn walk_node(
    node: &Node,
    source: &str,
    depth: u32,
    sort_order: &mut u32,
    symbols: &mut Vec<SymbolRecord>,
) {
    let kind = match node.kind() {
        "function_definition" | "function_definition_without_sub" => Some(SymbolKind::Function),
        "package_statement" => Some(SymbolKind::Module),
        _ => None,
    };

    {
        let mut sink = SymbolSink::new(source, sort_order, symbols, &DOC_SPEC);
        push_named_symbol(node, depth, kind, find_name, &mut sink);
    }
    walk_children(node, source, depth, sort_order, symbols, kind, walk_node);
}

fn find_name(node: &Node, source: &str, kind: SymbolKind) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // For subroutine_declaration_statement, look for identifier after 'sub'
        // For package_statement, look for package_name or identifier
        if child.kind() == "name" || child.kind() == "identifier" || child.kind() == "package_name"
        {
            return Some(child.utf8_text(source.as_bytes()).unwrap_or("").to_string());
        }
        // Some versions use 'subroutine_name' node
        if kind == SymbolKind::Function && child.kind() == "subroutine_name" {
            return Some(child.utf8_text(source.as_bytes()).unwrap_or("").to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::domain::{FileOutcome, LanguageId, SymbolKind};
    use crate::parsing::process_file;

    #[test]
    fn test_perl_process_file_extracts_subroutine() {
        let source = b"sub greet { print \"hello\\n\"; }";
        let result = process_file("test.pl", source, LanguageId::Perl);
        assert!(
            matches!(
                result.outcome,
                FileOutcome::Processed | FileOutcome::PartialParse { .. }
            ),
            "Perl should parse successfully: {:?}",
            result.outcome
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Function && s.name == "greet"),
            "should extract greet subroutine, symbols: {:?}",
            result.symbols
        );
    }

    #[test]
    fn test_perl_package_extracted_as_module() {
        let source = b"package MyApp::Module;\n\nsub new { return bless {}, shift; }";
        let result = process_file("test.pl", source, LanguageId::Perl);
        assert!(
            matches!(
                result.outcome,
                FileOutcome::Processed | FileOutcome::PartialParse { .. }
            ),
            "Perl should parse successfully: {:?}",
            result.outcome
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Module && s.name == "MyApp::Module"),
            "should extract package as Module, symbols: {:?}",
            result.symbols
        );
    }
}
