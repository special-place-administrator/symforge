use tree_sitter::Node;

use super::{
    DocCommentSpec, SymbolSink, collect_symbols, has_child_kind, push_named_symbol, walk_children,
};

pub(super) const DOC_SPEC: DocCommentSpec = DocCommentSpec {
    comment_node_types: &["comment"],
    doc_prefixes: Some(&["///", "/**", "/*!"]),
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
        "function_definition" => Some(SymbolKind::Function),
        // Only extract struct/enum definitions (with a body), not type-reference
        // usages like `enum ggml_type param` in function signatures.
        "struct_specifier" if has_child_kind(node, "field_declaration_list") => {
            Some(SymbolKind::Struct)
        }
        "enum_specifier" if has_child_kind(node, "enumerator_list") => Some(SymbolKind::Enum),
        "enumerator" => Some(SymbolKind::Constant),
        "type_definition" => Some(SymbolKind::Type),
        _ => None,
    };

    {
        let mut sink = SymbolSink::new(source, sort_order, symbols, &DOC_SPEC);
        push_named_symbol(
            node,
            depth,
            kind,
            |node, source, _| find_c_name(node, source),
            &mut sink,
        );
    }
    // Recurse into children, but skip struct/enum bodies to avoid re-extracting nested types
    // as children of the outer specifier (they get their own entry when directly encountered)
    walk_children(node, source, depth, sort_order, symbols, kind, walk_node);
}

/// Find the name for C declarations.
///
/// - `function_definition`: walk the declarator chain to find the function name.
///   C declarator grammar is recursive: declarator -> pointer_declarator -> function_declarator -> identifier.
/// - `struct_specifier` / `enum_specifier`: find the child `type_identifier`.
/// - `type_definition`: find the aliased name (last `type_identifier` child), or fall back to inner specifier name.
fn find_c_name(node: &Node, source: &str) -> Option<String> {
    match node.kind() {
        "function_definition" => find_function_name(node, source),
        "struct_specifier" | "enum_specifier" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "type_identifier" {
                    return Some(child.utf8_text(source.as_bytes()).unwrap_or("").to_string());
                }
            }
            None
        }
        "type_definition" => {
            // typedef struct Foo { ... } Foo_t;
            // The aliased name is the last `type_identifier` that appears directly under type_definition
            // (not inside the inner specifier body). Walk children from right to left to find it.
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            // The typedef alias is the last type_identifier or identifier before the semicolon
            for child in children.iter().rev() {
                if child.kind() == "type_identifier" || child.kind() == "identifier" {
                    return Some(child.utf8_text(source.as_bytes()).unwrap_or("").to_string());
                }
            }
            None
        }
        "enumerator" => {
            // enum Foo { BAR = 1 }; — enumerator's name is an `identifier` child.
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    return Some(child.utf8_text(source.as_bytes()).unwrap_or("").to_string());
                }
            }
            None
        }
        _ => None,
    }
}

/// Walk the declarator chain for a function_definition to extract the function name.
/// The chain is: function_definition -> declarator (pointer_declarator*) -> function_declarator -> identifier/qualified_identifier
fn find_function_name(node: &Node, source: &str) -> Option<String> {
    // Find the 'declarator' child of function_definition
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if (child.kind() == "declarator"
            || child.kind() == "pointer_declarator"
            || child.kind() == "function_declarator")
            && let Some(name) = extract_declarator_name(&child, source)
        {
            return Some(name);
        }
    }
    None
}

/// Recursively walk a declarator node to find the identifier.
fn extract_declarator_name(node: &Node, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" => Some(node.utf8_text(source.as_bytes()).unwrap_or("").to_string()),
        "qualified_identifier" => {
            // Preserve the full qualified name so edit tools match what the LLM sees.
            Some(node.utf8_text(source.as_bytes()).unwrap_or("").to_string())
        }
        _ => {
            // Recurse into pointer_declarator, function_declarator, abstract_declarator etc.
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(name) = extract_declarator_name(&child, source) {
                    return Some(name);
                }
            }
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FileOutcome, LanguageId};
    use crate::parsing::process_file;
    use tree_sitter::Parser;

    fn parse_c(source: &str) -> Vec<SymbolRecord> {
        let mut parser = Parser::new();
        let lang: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
        parser.set_language(&lang).expect("set C language");
        let tree = parser.parse(source, None).expect("parse C source");
        extract_symbols(&tree.root_node(), source)
    }

    #[test]
    fn test_c_language_function_definition() {
        let source = "int add(int a, int b) { return a + b; }";
        let symbols = parse_c(source);
        let func = symbols.iter().find(|s| s.kind == SymbolKind::Function);
        assert!(
            func.is_some(),
            "should extract function, got: {:?}",
            symbols
        );
        assert_eq!(func.unwrap().name, "add");
    }

    #[test]
    fn test_c_language_struct_specifier() {
        let source = "struct Point { int x; int y; };";
        let symbols = parse_c(source);
        let s = symbols.iter().find(|s| s.kind == SymbolKind::Struct);
        assert!(s.is_some(), "should extract struct, got: {:?}", symbols);
        assert_eq!(s.unwrap().name, "Point");
    }

    #[test]
    fn test_c_language_enum_specifier() {
        let source = "enum Color { RED, GREEN, BLUE };";
        let symbols = parse_c(source);
        let e = symbols.iter().find(|s| s.kind == SymbolKind::Enum);
        assert!(e.is_some(), "should extract enum, got: {:?}", symbols);
        assert_eq!(e.unwrap().name, "Color");
    }

    #[test]
    fn test_c_language_enum_variants_extracted() {
        let source = "enum Color { RED, GREEN = 1, BLUE };";
        let symbols = parse_c(source);
        let variants: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Constant)
            .collect();
        assert_eq!(
            variants.len(),
            3,
            "should extract 3 variants, got: {:?}",
            variants
        );
        let names: Vec<&str> = variants.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"RED"), "missing RED");
        assert!(names.contains(&"GREEN"), "missing GREEN");
        assert!(names.contains(&"BLUE"), "missing BLUE");
    }

    #[test]
    fn test_c_language_enum_variants_nested_under_enum() {
        let source = "enum Status {\n    OK = 0,\n    ERR = 1\n};";
        let symbols = parse_c(source);
        let enum_sym = symbols.iter().find(|s| s.kind == SymbolKind::Enum).unwrap();
        let variants: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Constant)
            .collect();
        // Variants should be at greater depth than the enum
        for v in &variants {
            assert!(
                v.depth > enum_sym.depth,
                "variant {} depth {} should be > enum depth {}",
                v.name,
                v.depth,
                enum_sym.depth
            );
        }
    }

    #[test]
    fn test_c_language_enum_in_param_not_extracted() {
        // `enum Color` used as a parameter type should NOT create an Enum symbol.
        let source = "void paint(enum Color c) { }";
        let symbols = parse_c(source);
        let enums: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Enum)
            .collect();
        assert!(
            enums.is_empty(),
            "parameter-type enum should not be extracted: {:?}",
            enums
        );
    }

    #[test]
    fn test_c_language_struct_in_param_not_extracted() {
        let source = "void draw(struct Point p) { }";
        let symbols = parse_c(source);
        let structs: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .collect();
        assert!(
            structs.is_empty(),
            "parameter-type struct should not be extracted: {:?}",
            structs
        );
    }

    #[test]
    fn test_c_language_typedef() {
        let source = "typedef struct Point { int x; int y; } Point_t;";
        let symbols = parse_c(source);
        let t = symbols.iter().find(|s| s.kind == SymbolKind::Type);
        assert!(t.is_some(), "should extract typedef, got: {:?}", symbols);
        assert_eq!(t.unwrap().name, "Point_t");
    }

    #[test]
    fn test_c_language_pointer_function() {
        let source = "void *malloc_wrapper(size_t size) { return 0; }";
        let symbols = parse_c(source);
        let func = symbols.iter().find(|s| s.kind == SymbolKind::Function);
        assert!(
            func.is_some(),
            "should extract pointer-return function, got: {:?}",
            symbols
        );
        assert_eq!(func.unwrap().name, "malloc_wrapper");
    }

    #[test]
    fn test_c_language_process_file_returns_processed() {
        let source = b"int main(int argc, char **argv) { return 0; }\nstruct Node { int val; };";
        let result = process_file("test.c", source, LanguageId::C);
        assert_eq!(
            result.outcome,
            FileOutcome::Processed,
            "outcome: {:?}",
            result.outcome
        );
        assert!(!result.symbols.is_empty(), "should have symbols");
        let func = result
            .symbols
            .iter()
            .find(|s| s.kind == SymbolKind::Function && s.name == "main");
        assert!(func.is_some(), "should have main function");
    }
}
