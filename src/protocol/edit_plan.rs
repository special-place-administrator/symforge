//! Edit planning: analyzes a target symbol/file and suggests the right
//! sequence of SymForge edit tools to accomplish a change.

use crate::live_index::store::LiveIndex;

fn split_path_qualified_target(target: &str) -> Option<(&str, &str)> {
    if let Some((path, name)) = target.split_once("::") {
        let path = path.trim();
        let name = name.trim();
        if !path.is_empty() && !name.is_empty() {
            return Some((path, name));
        }
    }

    None
}

/// Plan an edit operation: analyze impact and suggest tool sequence.
pub fn plan_edit(index: &LiveIndex, target: &str) -> String {
    let target = target.trim();
    let qualified_target = split_path_qualified_target(target);
    let symbol_target_name = qualified_target.map(|(_, name)| name).unwrap_or(target);

    // Try to find the target as a symbol first
    let mut symbol_hits = Vec::new();
    let mut file_hit = None;

    for (path, file) in index.all_files() {
        for sym in &file.symbols {
            let symbol_match = if let Some((target_path, target_name)) = qualified_target {
                sym.name == target_name && (path == target_path || path.ends_with(target_path))
            } else {
                sym.name == target
            };
            if symbol_match {
                symbol_hits.push((path.clone(), sym.name.clone(), sym.kind, sym.line_range));
            }
        }
        if path.ends_with(target) || path == target {
            file_hit = Some(path.clone());
        }
    }

    let mut lines = vec!["── Edit Plan ──".to_string()];

    if symbol_hits.is_empty() && file_hit.is_none() {
        lines.push(format!("Target '{}' not found.", target));
        lines.push("Try: search_symbols(query=\"...\") to find the correct name.".to_string());
        return lines.join("\n");
    }

    if !symbol_hits.is_empty() {
        lines.push(format!(
            "Found {} symbol(s) matching '{}':",
            symbol_hits.len(),
            target
        ));
        for (path, name, kind, (start, end)) in &symbol_hits {
            lines.push(format!(
                "  {:?} {} in {} (lines {}-{})",
                kind, name, path, start, end
            ));
        }

        // Count references
        let ref_count = index
            .find_references_for_name(symbol_target_name, None, false)
            .len();
        lines.push(format!(
            "\nReferences: {} call sites across the project",
            ref_count
        ));

        if ref_count > 10 {
            lines.push(
                "⚠ HIGH IMPACT: >10 callers. Use batch_rename(dry_run=true) to preview."
                    .to_string(),
            );
        }

        lines.push("\nSuggested tool sequence:".to_string());
        if symbol_hits.len() == 1 {
            let (path, name, _, _) = &symbol_hits[0];
            lines.push(format!("  1. get_symbol_context(name=\"{name}\", path=\"{path}\", bundle=true) — understand full context"));
            lines.push(format!("  2. Choose edit approach:"));
            lines.push(format!("     - Small change: edit_within_symbol(path=\"{path}\", name=\"{name}\", old_text=..., new_text=...)"));
            lines.push(format!("     - Full rewrite: replace_symbol_body(path=\"{path}\", name=\"{name}\", new_body=...)"));
            lines.push(format!("     - Rename: batch_rename(path=\"{path}\", name=\"{name}\", new_name=..., dry_run=true)"));
            lines.push(format!(
                "     - Delete: delete_symbol(path=\"{path}\", name=\"{name}\", dry_run=true)"
            ));
            lines.push(format!(
                "  3. analyze_file_impact(path=\"{path}\") — verify changes"
            ));
        } else {
            lines.push("  1. Use symbol_line to disambiguate the target".to_string());
            lines.push("  2. get_symbol_context(bundle=true) on the chosen symbol".to_string());
            lines.push("  3. batch_edit(dry_run=true) for multi-symbol changes".to_string());
        }
    }

    if let Some(path) = file_hit {
        if symbol_hits.is_empty() {
            lines.push(format!("Found file: {}", path));
            lines.push(format!("\nSuggested approach:"));
            lines.push(format!("  1. get_file_context(path=\"{path}\", sections=[\"outline\"]) — understand structure"));
            lines.push(format!(
                "  2. get_symbol(path=\"{path}\", name=\"<target>\") — read specific symbols"
            ));
            lines.push(format!(
                "  3. Use edit_within_symbol or replace_symbol_body for changes"
            ));
            lines.push(format!(
                "  4. analyze_file_impact(path=\"{path}\") — verify"
            ));
        }
    }

    lines.join("\n")
}
