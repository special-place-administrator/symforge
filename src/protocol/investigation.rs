//! Investigation mode: structured multi-step exploration with gap analysis.
//! Builds on SessionContext to suggest what the LLM hasn't looked at yet.

use crate::live_index::store::LiveIndex;
use crate::protocol::session::SessionContext;

/// Analyze session context and suggest unexplored symbols/files.
pub fn suggest_next_steps(
    index: &LiveIndex,
    session: &SessionContext,
    focus: Option<&str>,
) -> String {
    let snap = session.snapshot();
    let mut lines = vec!["── Investigation Suggestions ──".to_string()];

    if snap.symbols.is_empty() && snap.files.is_empty() {
        lines.push("No symbols or files fetched yet. Start with:".to_string());
        lines.push("  - get_repo_map(detail=\"compact\") for project overview".to_string());
        lines.push("  - explore(query=\"<topic>\") for concept discovery".to_string());
        lines.push("  - search_symbols(query=\"<name>\") to find specific symbols".to_string());
        return lines.join("\n");
    }

    lines.push(format!(
        "You've loaded {} symbols and {} files (~{} tokens).",
        snap.symbols.len(),
        snap.files.len(),
        snap.total_tokens
    ));

    // Find symbols that are referenced by loaded symbols but not yet fetched
    let loaded_symbol_names: std::collections::HashSet<&str> = snap
        .symbols
        .iter()
        .map(|(_, name, _)| name.as_str())
        .collect();

    let mut suggested_symbols: Vec<(String, String, &str)> = Vec::new(); // (path, name, reason)

    for (path, name, _) in &snap.symbols {
        // Find callees of loaded symbols that aren't loaded themselves
        if let Some(file) = index.get_file(path) {
            for sym in &file.symbols {
                if sym.name == *name {
                    // Look at references within this symbol's range
                    for reference in &file.references {
                        if reference.line_range.0 >= sym.line_range.0
                            && reference.line_range.1 <= sym.line_range.1
                            && matches!(reference.kind, crate::domain::index::ReferenceKind::Call)
                            && !loaded_symbol_names.contains(reference.name.as_str())
                            && reference.name.len() > 2
                        {
                            suggested_symbols.push((
                                path.clone(),
                                reference.name.clone(),
                                "called by loaded symbol",
                            ));
                        }
                    }
                }
            }
        }
    }

    // Deduplicate and limit
    suggested_symbols.sort_by(|a, b| a.1.cmp(&b.1));
    suggested_symbols.dedup_by(|a, b| a.1 == b.1);

    // Apply focus filter if provided
    if let Some(focus_term) = focus {
        let focus_lower = focus_term.to_ascii_lowercase();
        suggested_symbols.retain(|(path, name, _)| {
            path.to_ascii_lowercase().contains(&focus_lower)
                || name.to_ascii_lowercase().contains(&focus_lower)
        });
    }

    if !suggested_symbols.is_empty() {
        lines.push(String::new());
        lines.push("Symbols referenced but not yet loaded:".to_string());
        for (_, name, reason) in suggested_symbols.iter().take(10) {
            lines.push(format!("  {name} — {reason}"));
        }
        if suggested_symbols.len() > 10 {
            lines.push(format!("  ... and {} more", suggested_symbols.len() - 10));
        }
        lines.push(String::new());
        lines.push("To investigate, call:".to_string());
        if let Some((_, name, _)) = suggested_symbols.first() {
            lines.push(format!(
                "  get_symbol_context(name=\"{name}\", verbosity=\"compact\")"
            ));
        }
    } else {
        lines.push(String::new());
        lines.push("No obvious gaps in loaded context. You may want to:".to_string());
        lines.push("  - search_text(query=\"TODO\") for outstanding work".to_string());
        lines.push("  - what_changed(uncommitted=true) for recent modifications".to_string());
        lines.push("  - find_dependents on a key file to check impact radius".to_string());
    }

    lines.join("\n")
}
