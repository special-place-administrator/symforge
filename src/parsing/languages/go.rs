use tree_sitter::Node;

use crate::domain::{SymbolKind, SymbolRecord};

pub fn extract_symbols(node: &Node, source: &str) -> Vec<SymbolRecord> {
    let mut symbols = Vec::new();
    let mut sort_order = 0u32;
    walk_node(node, source, 0, &mut sort_order, &mut symbols);
    symbols
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

    if let Some(symbol_kind) = kind {
        if let Some(name) = find_name(node, source) {
            symbols.push(SymbolRecord {
                name,
                kind: symbol_kind,
                depth,
                sort_order: *sort_order,
                byte_range: (node.start_byte() as u32, node.end_byte() as u32),
                line_range: (
                    node.start_position().row as u32,
                    node.end_position().row as u32,
                ),
            });
            *sort_order += 1;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let child_depth = if kind.is_some() { depth + 1 } else { depth };
        walk_node(&child, source, child_depth, sort_order, symbols);
    }
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
        if child.kind() == "type_spec" {
            if let Some(name) = find_name(&child, source) {
                let kind = classify_type_spec(&child);
                symbols.push(SymbolRecord {
                    name,
                    kind,
                    depth,
                    sort_order: *sort_order,
                    byte_range: (child.start_byte() as u32, child.end_byte() as u32),
                    line_range: (
                        child.start_position().row as u32,
                        child.end_position().row as u32,
                    ),
                });
                *sort_order += 1;
            }
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
        if child.kind() == "const_spec" || child.kind() == "var_spec" {
            if let Some(name) = find_name(&child, source) {
                symbols.push(SymbolRecord {
                    name,
                    kind: if is_const {
                        SymbolKind::Constant
                    } else {
                        SymbolKind::Variable
                    },
                    depth,
                    sort_order: *sort_order,
                    byte_range: (child.start_byte() as u32, child.end_byte() as u32),
                    line_range: (
                        child.start_position().row as u32,
                        child.end_position().row as u32,
                    ),
                });
                *sort_order += 1;
            }
        }
    }
}

fn find_name(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "type_identifier" {
            return Some(child.utf8_text(source.as_bytes()).unwrap_or("").to_string());
        }
    }
    None
}
