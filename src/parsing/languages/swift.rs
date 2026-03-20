use tree_sitter::Node;

use super::{
    DocCommentSpec, collect_symbols, find_first_named_child, push_named_symbol, walk_children,
};

pub(super) const DOC_SPEC: DocCommentSpec = DocCommentSpec {
    comment_node_types: &["comment", "multiline_comment"],
    doc_prefixes: Some(&["///", "/**"]),
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
        "class_declaration" => Some(SymbolKind::Class),
        "struct_declaration" => Some(SymbolKind::Struct),
        "enum_declaration" => Some(SymbolKind::Enum),
        "protocol_declaration" => Some(SymbolKind::Interface),
        "extension_declaration" => Some(SymbolKind::Impl),
        _ => None,
    };

    push_named_symbol(
        node,
        source,
        depth,
        sort_order,
        symbols,
        kind,
        |node, source, _| find_name(node, source),
        &DOC_SPEC,
    );
    walk_children(node, source, depth, sort_order, symbols, kind, walk_node);
}

fn find_name(node: &Node, source: &str) -> Option<String> {
    find_first_named_child(node, source, &["simple_identifier", "type_identifier"])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::domain::{FileOutcome, LanguageId, SymbolKind};
    use crate::parsing::process_file;

    #[test]
    fn test_swift_process_file_extracts_class_and_function() {
        let source = b"class Foo { func bar() -> Int { return 0 } }";
        let result = process_file("test.swift", source, LanguageId::Swift);
        assert!(
            matches!(
                result.outcome,
                FileOutcome::Processed | FileOutcome::PartialParse { .. }
            ),
            "Swift should parse successfully: {:?}",
            result.outcome
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Class && s.name == "Foo"),
            "should extract Foo class, symbols: {:?}",
            result.symbols
        );
    }

    // tree-sitter-swift v0.7.1 emits `class_declaration` for both `class` and
    // `extension` declarations (it cannot distinguish them at the grammar level).
    // Therefore extensions are extracted as Class, not Impl.
    #[test]
    fn test_swift_extension_extracted_as_class() {
        let source = b"protocol Drawable {}\nclass MyClass {}\nextension MyClass: Drawable {}";
        let result = process_file("test.swift", source, LanguageId::Swift);
        assert!(
            matches!(
                result.outcome,
                FileOutcome::Processed | FileOutcome::PartialParse { .. }
            ),
            "Swift should parse successfully: {:?}",
            result.outcome
        );
        // The grammar maps extension_declaration -> class_declaration, so the
        // extractor produces Class.  Verify the name is captured regardless.
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Class && s.name == "MyClass"),
            "should extract extension as Class (grammar limitation), symbols: {:?}",
            result.symbols
        );
    }

    #[test]
    fn test_swift_protocol_extracted_as_interface() {
        let source = b"protocol Drawable {}";
        let result = process_file("test.swift", source, LanguageId::Swift);
        assert!(
            matches!(
                result.outcome,
                FileOutcome::Processed | FileOutcome::PartialParse { .. }
            ),
            "Swift should parse successfully: {:?}",
            result.outcome
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Interface && s.name == "Drawable"),
            "should extract protocol as Interface, symbols: {:?}",
            result.symbols
        );
    }

    // tree-sitter-swift v0.7.1 emits `class_declaration` for `enum` as well.
    // Verify the enum name is captured even though the kind is Class.
    #[test]
    fn test_swift_enum_extracted_as_class() {
        let source = b"enum Direction { case north }";
        let result = process_file("test.swift", source, LanguageId::Swift);
        assert!(
            matches!(
                result.outcome,
                FileOutcome::Processed | FileOutcome::PartialParse { .. }
            ),
            "Swift should parse successfully: {:?}",
            result.outcome
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.kind == SymbolKind::Class && s.name == "Direction"),
            "should extract enum as Class (grammar limitation), symbols: {:?}",
            result.symbols
        );
    }
}
