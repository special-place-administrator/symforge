use tree_sitter::Node;

use super::{
    DocCommentSpec, SymbolSink, collect_symbols, find_first_named_child, push_named_symbol,
    push_symbol, walk_children,
};

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
        "function_declaration" => Some(SymbolKind::Function),
        "method_declaration" => Some(SymbolKind::Method),
        "type_declaration" => {
            extract_type_declarations(node, source, depth, sort_order, symbols);
            return;
        }
        "const_declaration" | "var_declaration" => {
            extract_var_declarations(node, source, depth, sort_order, symbols);
            return;
        }
        _ => None,
    };

    {
        let mut sink = SymbolSink::new(source, sort_order, symbols, &DOC_SPEC);
        push_named_symbol(
            node,
            depth,
            kind,
            |node, source, _| find_name(node, source),
            &mut sink,
        );
    }
    walk_children(node, source, depth, sort_order, symbols, kind, walk_node);
}

fn extract_type_declarations(
    node: &Node,
    source: &str,
    depth: u32,
    sort_order: &mut u32,
    symbols: &mut Vec<SymbolRecord>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_spec"
            && let Some(name) = find_name(&child, source)
        {
            let kind = classify_type_spec(&child);
            let mut sink = SymbolSink::new(source, sort_order, symbols, &DOC_SPEC);
            push_symbol(&child, name, kind, depth, &mut sink);
        }
    }
}

fn classify_type_spec(node: &Node) -> SymbolKind {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "struct_type" => return SymbolKind::Struct,
            "interface_type" => return SymbolKind::Interface,
            _ => {}
        }
    }
    SymbolKind::Type
}

fn extract_var_declarations(
    node: &Node,
    source: &str,
    depth: u32,
    sort_order: &mut u32,
    symbols: &mut Vec<SymbolRecord>,
) {
    let is_const = node.kind() == "const_declaration";
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if (child.kind() == "const_spec" || child.kind() == "var_spec")
            && let Some(name) = find_name(&child, source)
        {
            let mut sink = SymbolSink::new(source, sort_order, symbols, &DOC_SPEC);
            push_symbol(
                &child,
                name,
                if is_const {
                    SymbolKind::Constant
                } else {
                    SymbolKind::Variable
                },
                depth,
                &mut sink,
            );
        }
    }
}

fn find_name(node: &Node, source: &str) -> Option<String> {
    // For method_declaration, the receiver's parameter_list contains an
    // identifier that would match before the method's field_identifier.
    // Skip the receiver and look for field_identifier directly.
    if node.kind() == "method_declaration" {
        return find_method_name(node, source);
    }
    find_first_named_child(
        node,
        source,
        &["identifier", "type_identifier", "field_identifier"],
    )
}

/// Extract method name from a Go method_declaration node.
/// The grammar structure is: receiver(parameter_list) + name(field_identifier) + ...
/// We need the top-level field_identifier, not the identifier inside the receiver.
fn find_method_name(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "field_identifier" {
            let text = source[child.start_byte()..child.end_byte()].trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
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
    fn test_go_function_extracted() {
        let source = b"package main\n\nfunc main() {}";
        let result = process_file("test.go", source, LanguageId::Go);
        assert!(
            matches!(
                result.outcome,
                FileOutcome::Processed | FileOutcome::PartialParse { .. }
            ),
            "Go should parse successfully: {:?}",
            result.outcome
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Function && s.name == "main"),
            "should extract func main as Function, got: {:?}",
            result.symbols
        );
    }

    #[test]
    fn test_go_method_extracted() {
        let source = b"package main\n\ntype Server struct{}\n\nfunc (s *Server) Handle() {}";
        let result = process_file("test.go", source, LanguageId::Go);
        assert!(
            matches!(
                result.outcome,
                FileOutcome::Processed | FileOutcome::PartialParse { .. }
            ),
            "Go should parse successfully: {:?}",
            result.outcome
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Method && s.name == "Handle"),
            "should extract Handle as Method, got: {:?}",
            result.symbols
        );
    }

    #[test]
    fn test_go_struct_extracted() {
        let source = b"package main\n\ntype Config struct { Name string }";
        let result = process_file("test.go", source, LanguageId::Go);
        assert!(
            matches!(
                result.outcome,
                FileOutcome::Processed | FileOutcome::PartialParse { .. }
            ),
            "Go should parse successfully: {:?}",
            result.outcome
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Struct && s.name == "Config"),
            "should extract Config as Struct, got: {:?}",
            result.symbols
        );
    }

    #[test]
    fn test_go_interface_extracted() {
        let source = b"package main\n\ntype Handler interface { Handle() }";
        let result = process_file("test.go", source, LanguageId::Go);
        assert!(
            matches!(
                result.outcome,
                FileOutcome::Processed | FileOutcome::PartialParse { .. }
            ),
            "Go should parse successfully: {:?}",
            result.outcome
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Interface && s.name == "Handler"),
            "should extract Handler as Interface, got: {:?}",
            result.symbols
        );
    }

    #[test]
    fn test_go_constant_extracted() {
        let source = b"package main\n\nconst MaxRetries = 3";
        let result = process_file("test.go", source, LanguageId::Go);
        assert!(
            matches!(
                result.outcome,
                FileOutcome::Processed | FileOutcome::PartialParse { .. }
            ),
            "Go should parse successfully: {:?}",
            result.outcome
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Constant && s.name == "MaxRetries"),
            "should extract MaxRetries as Constant, got: {:?}",
            result.symbols
        );
    }

    #[test]
    fn test_go_variable_extracted() {
        let source = b"package main\n\nvar DefaultTimeout = 30";
        let result = process_file("test.go", source, LanguageId::Go);
        assert!(
            matches!(
                result.outcome,
                FileOutcome::Processed | FileOutcome::PartialParse { .. }
            ),
            "Go should parse successfully: {:?}",
            result.outcome
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Variable && s.name == "DefaultTimeout"),
            "should extract DefaultTimeout as Variable, got: {:?}",
            result.symbols
        );
    }

    #[test]
    fn test_go_all_symbol_kinds() {
        let source = b"package main\n\nfunc main() {}\n\ntype Server struct{}\n\nfunc (s *Server) Handle() {}\n\ntype Config struct { Name string }\n\ntype Handler interface { Handle() }\n\nconst MaxRetries = 3\n\nvar DefaultTimeout = 30";
        let result = process_file("test.go", source, LanguageId::Go);
        assert!(
            matches!(
                result.outcome,
                FileOutcome::Processed | FileOutcome::PartialParse { .. }
            ),
            "Go should parse successfully: {:?}",
            result.outcome
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Function),
            "missing Function"
        );
        assert!(
            result.symbols.iter().any(|s| s.kind == SymbolKind::Method),
            "missing Method"
        );
        assert!(
            result.symbols.iter().any(|s| s.kind == SymbolKind::Struct),
            "missing Struct"
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Interface),
            "missing Interface"
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Constant),
            "missing Constant"
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Variable),
            "missing Variable"
        );
    }
}
