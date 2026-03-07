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
        "function_item" => Some(SymbolKind::Function),
        "struct_item" => Some(SymbolKind::Struct),
        "enum_item" => Some(SymbolKind::Enum),
        "trait_item" => Some(SymbolKind::Trait),
        "impl_item" => Some(SymbolKind::Impl),
        "const_item" => Some(SymbolKind::Constant),
        "static_item" => Some(SymbolKind::Variable),
        "mod_item" => Some(SymbolKind::Module),
        "type_item" => Some(SymbolKind::Type),
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
                line_range: (node.start_position().row as u32, node.end_position().row as u32),
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

fn find_name(node: &Node, source: &str) -> Option<String> {
    // For impl items, construct "impl Type" or "impl Trait for Type"
    if node.kind() == "impl_item" {
        return extract_impl_name(node, source);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "name"
            || child.kind() == "identifier"
            || child.kind() == "type_identifier"
        {
            return Some(child.utf8_text(source.as_bytes()).unwrap_or("").to_string());
        }
    }
    None
}

fn extract_impl_name(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    let mut trait_name = None;
    let mut type_name = None;
    let mut found_for = false;

    for child in &children {
        match child.kind() {
            "type_identifier" | "scoped_type_identifier" | "generic_type" => {
                let text = child.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                if found_for {
                    type_name = Some(text);
                } else if trait_name.is_none() {
                    trait_name = Some(text);
                } else {
                    type_name = Some(text);
                }
            }
            "for" => {
                found_for = true;
            }
            _ => {}
        }
    }

    if found_for {
        match (&trait_name, &type_name) {
            (Some(tr), Some(ty)) => return Some(format!("impl {tr} for {ty}")),
            _ => {}
        }
    }

    trait_name.map(|n| format!("impl {n}"))
}
