//! Smart query routing: natural-language entry point that classifies intent
//! and dispatches to the right specialized tool internally.

/// Classified intent from a natural-language query.
#[derive(Debug)]
pub enum QueryIntent {
    /// "who calls X", "callers of X", "references to X"
    FindCallers { symbol: String },
    /// "where is X defined", "find symbol X"
    FindSymbol { name: String, kind: Option<String> },
    /// "find file X", "where is file X", "path to X"
    FindFile { hint: String },
    /// "what changed", "recent changes", "uncommitted"
    FindChanges,
    /// "how does X work", "explain X", "understand X"
    Understand { concept: String },
    /// "search for X in code", "grep X", code pattern
    SearchCode { pattern: String },
    /// "what depends on X", "dependents of X"
    FindDependents { target: String },
    /// "implementations of X", "who implements X"
    FindImplementations { name: String },
    /// Fallback: explore the concept
    Explore { query: String },
}

/// Classify a natural-language query into a routable intent.
pub fn classify_intent(query: &str) -> QueryIntent {
    let q = query.trim();
    let lower = q.to_ascii_lowercase();

    // --- Pattern: "who/what calls X" or "callers of X" or "references to X" ---
    if let Some(sym) = strip_prefix_phrase(&lower, &[
        "who calls ", "what calls ", "callers of ", "callers for ",
        "references to ", "references for ", "find references ",
        "usages of ", "who uses ",
    ]) {
        return QueryIntent::FindCallers {
            symbol: clean_symbol_name(sym, q),
        };
    }

    // --- Pattern: "what depends on X" or "dependents of X" ---
    if let Some(target) = strip_prefix_phrase(&lower, &[
        "what depends on ", "depends on ", "dependents of ",
        "dependents for ", "who imports ", "what imports ",
    ]) {
        return QueryIntent::FindDependents {
            target: clean_symbol_name(target, q),
        };
    }

    // --- Pattern: "implementations of X" or "who implements X" ---
    if let Some(name) = strip_prefix_phrase(&lower, &[
        "implementations of ", "implementors of ", "who implements ",
        "what implements ", "implementations for ",
    ]) {
        return QueryIntent::FindImplementations {
            name: clean_symbol_name(name, q),
        };
    }

    // --- Pattern: "where is X defined" or "find symbol X" or "definition of X" ---
    if let Some(name) = strip_prefix_phrase(&lower, &[
        "where is ", "find symbol ", "definition of ",
        "show me ", "go to ", "jump to ", "locate ",
        "find definition ", "find function ", "find struct ",
        "find class ", "find type ", "find method ",
        "find enum ", "find trait ", "find interface ",
    ]) {
        let name = name.trim_end_matches(" defined")
            .trim_end_matches(" declaration");
        let (kind, clean_name) = extract_kind_hint(name);
        return QueryIntent::FindSymbol {
            name: clean_symbol_name(clean_name, q),
            kind,
        };
    }

    // --- Pattern: "find file X" or "path to X" ---
    if let Some(hint) = strip_prefix_phrase(&lower, &[
        "find file ", "path to ", "where is file ",
        "locate file ", "which file ",
    ]) {
        return QueryIntent::FindFile {
            hint: hint.to_string(),
        };
    }

    // --- Pattern: "what changed" or "recent changes" ---
    if lower.starts_with("what changed")
        || lower.starts_with("recent changes")
        || lower.starts_with("uncommitted")
        || lower == "changes"
        || lower.starts_with("what's changed")
        || lower.starts_with("show changes")
        || lower.starts_with("git status")
        || lower.starts_with("what did i change")
    {
        return QueryIntent::FindChanges;
    }

    // --- Pattern: "how does X work" or "explain X" or "understand X" ---
    if let Some(concept) = strip_prefix_phrase(&lower, &[
        "how does ", "how do ", "explain ", "understand ",
        "what is ", "what are ", "describe ", "tell me about ",
        "help me understand ", "walk me through ",
    ]) {
        let concept = concept.trim_end_matches(" work")
            .trim_end_matches(" works")
            .trim_end_matches("?");
        return QueryIntent::Understand {
            concept: concept.to_string(),
        };
    }

    // --- Pattern: "search for X" or "grep X" or "find X in code" ---
    if let Some(pattern) = strip_prefix_phrase(&lower, &[
        "search for ", "search ", "grep ", "find in code ",
        "look for ", "find text ", "find string ",
    ]) {
        return QueryIntent::SearchCode {
            pattern: pattern.trim_matches('"').trim_matches('\'').to_string(),
        };
    }

    // --- Heuristic: looks like a file path (contains / or common extensions) ---
    if looks_like_path(q) {
        return QueryIntent::FindFile {
            hint: q.to_string(),
        };
    }

    // --- Heuristic: looks like a symbol name (CamelCase, snake_case, no spaces) ---
    if looks_like_symbol(q) {
        return QueryIntent::FindSymbol {
            name: q.to_string(),
            kind: None,
        };
    }

    // --- Heuristic: looks like a code pattern (operators, keywords, brackets) ---
    if looks_like_code_pattern(q) {
        return QueryIntent::SearchCode {
            pattern: q.to_string(),
        };
    }

    // --- Default: explore the concept ---
    QueryIntent::Explore {
        query: q.to_string(),
    }
}

/// Try each prefix phrase; return the remainder if one matches.
fn strip_prefix_phrase<'a>(lower: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    for prefix in prefixes {
        if lower.starts_with(prefix) {
            let rest = &lower[prefix.len()..];
            let rest = rest.trim();
            if !rest.is_empty() {
                return Some(rest);
            }
        }
    }
    None
}

/// Extract a kind hint from phrases like "function foo" or "struct Bar".
fn extract_kind_hint(name: &str) -> (Option<String>, &str) {
    let kind_prefixes = [
        ("function ", "fn"),
        ("fn ", "fn"),
        ("struct ", "struct"),
        ("class ", "class"),
        ("type ", "type"),
        ("method ", "method"),
        ("enum ", "enum"),
        ("trait ", "trait"),
        ("interface ", "interface"),
        ("constant ", "constant"),
        ("const ", "constant"),
        ("variable ", "variable"),
        ("var ", "variable"),
    ];
    for (prefix, kind) in &kind_prefixes {
        if name.starts_with(prefix) {
            return (Some(kind.to_string()), &name[prefix.len()..]);
        }
    }
    (None, name)
}

/// Preserve original casing from the user's query for the matched portion.
fn clean_symbol_name(lower_match: &str, original: &str) -> String {
    // Try to find the original-cased version in the user's query
    let lower_original = original.to_ascii_lowercase();
    if let Some(pos) = lower_original.find(lower_match) {
        return original[pos..pos + lower_match.len()].trim().to_string();
    }
    lower_match.trim().to_string()
}

fn looks_like_path(q: &str) -> bool {
    // Contains path separators or file extensions
    (q.contains('/') || q.contains('\\'))
        || q.ends_with(".rs")
        || q.ends_with(".py")
        || q.ends_with(".ts")
        || q.ends_with(".js")
        || q.ends_with(".go")
        || q.ends_with(".java")
        || q.ends_with(".toml")
        || q.ends_with(".yaml")
        || q.ends_with(".yml")
        || q.ends_with(".json")
        || q.ends_with(".md")
}

fn looks_like_symbol(q: &str) -> bool {
    if q.contains(' ') || q.is_empty() {
        return false;
    }
    // CamelCase: has uppercase not at start, or has underscore (snake_case)
    let has_camel = q.chars().skip(1).any(|c| c.is_uppercase());
    let has_snake = q.contains('_');
    let has_colons = q.contains("::");
    let all_alnum = q.chars().all(|c| c.is_alphanumeric() || c == '_' || c == ':');
    all_alnum && (has_camel || has_snake || has_colons)
}

fn looks_like_code_pattern(q: &str) -> bool {
    // Contains operators, brackets, or obvious code syntax
    q.contains("==") || q.contains("!=") || q.contains("->")
        || q.contains("=>") || q.contains("fn ") || q.contains("pub ")
        || q.contains("let ") || q.contains("def ") || q.contains("class ")
        || q.contains("impl ") || q.contains("struct ")
        || (q.contains('(') && q.contains(')'))
        || (q.contains('{') && q.contains('}'))
}

/// Describe which tool was routed to, for the LLM to learn the mapping.
pub fn route_description(intent: &QueryIntent) -> String {
    match intent {
        QueryIntent::FindCallers { symbol } => {
            format!("[Routed to: find_references(name=\"{symbol}\")]")
        }
        QueryIntent::FindSymbol { name, kind } => {
            if let Some(k) = kind {
                format!("[Routed to: search_symbols(query=\"{name}\", kind=\"{k}\")]")
            } else {
                format!("[Routed to: search_symbols(query=\"{name}\")]")
            }
        }
        QueryIntent::FindFile { hint } => {
            format!("[Routed to: search_files(query=\"{hint}\")]")
        }
        QueryIntent::FindChanges => {
            "[Routed to: what_changed(uncommitted=true)]".to_string()
        }
        QueryIntent::Understand { concept } => {
            format!("[Routed to: explore(query=\"{concept}\", depth=2)]")
        }
        QueryIntent::SearchCode { pattern } => {
            format!("[Routed to: search_text(query=\"{pattern}\")]")
        }
        QueryIntent::FindDependents { target } => {
            format!("[Routed to: find_dependents(path=\"{target}\")]")
        }
        QueryIntent::FindImplementations { name } => {
            format!("[Routed to: find_references(name=\"{name}\", mode=\"implementations\")]")
        }
        QueryIntent::Explore { query } => {
            format!("[Routed to: explore(query=\"{query}\")]")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_callers() {
        match classify_intent("who calls optimize_deterministic") {
            QueryIntent::FindCallers { symbol } => assert_eq!(symbol, "optimize_deterministic"),
            other => panic!("Expected FindCallers, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_callers_references() {
        match classify_intent("references to LiveIndex") {
            QueryIntent::FindCallers { symbol } => assert_eq!(symbol, "LiveIndex"),
            other => panic!("Expected FindCallers, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_find_symbol() {
        match classify_intent("where is optimize_deterministic defined") {
            QueryIntent::FindSymbol { name, kind } => {
                assert_eq!(name, "optimize_deterministic");
                assert!(kind.is_none());
            }
            other => panic!("Expected FindSymbol, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_find_symbol_with_kind() {
        match classify_intent("find struct LiveIndex") {
            QueryIntent::FindSymbol { name, kind } => {
                assert_eq!(name, "LiveIndex");
                assert_eq!(kind, None);
            }
            other => panic!("Expected FindSymbol, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_find_file() {
        match classify_intent("find file tools.rs") {
            QueryIntent::FindFile { hint } => assert_eq!(hint, "tools.rs"),
            other => panic!("Expected FindFile, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_path_heuristic() {
        match classify_intent("src/protocol/mod.rs") {
            QueryIntent::FindFile { hint } => assert_eq!(hint, "src/protocol/mod.rs"),
            other => panic!("Expected FindFile, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_symbol_heuristic() {
        match classify_intent("LiveIndex") {
            QueryIntent::FindSymbol { name, .. } => assert_eq!(name, "LiveIndex"),
            other => panic!("Expected FindSymbol, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_snake_case_heuristic() {
        match classify_intent("search_symbols_with_options") {
            QueryIntent::FindSymbol { name, .. } => assert_eq!(name, "search_symbols_with_options"),
            other => panic!("Expected FindSymbol, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_changes() {
        match classify_intent("what changed") {
            QueryIntent::FindChanges => {}
            other => panic!("Expected FindChanges, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_understand() {
        match classify_intent("how does the synergy pipeline work") {
            QueryIntent::Understand { concept } => assert_eq!(concept, "the synergy pipeline"),
            other => panic!("Expected Understand, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_search_code() {
        match classify_intent("search for TODO") {
            QueryIntent::SearchCode { pattern } => assert_eq!(pattern, "todo"),
            other => panic!("Expected SearchCode, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_dependents() {
        match classify_intent("what depends on src/protocol/mod.rs") {
            QueryIntent::FindDependents { target } => assert_eq!(target, "src/protocol/mod.rs"),
            other => panic!("Expected FindDependents, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_implementations() {
        match classify_intent("implementations of LlmClient") {
            QueryIntent::FindImplementations { name } => assert_eq!(name, "LlmClient"),
            other => panic!("Expected FindImplementations, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_explore_fallback() {
        match classify_intent("error handling patterns") {
            QueryIntent::Explore { query } => assert_eq!(query, "error handling patterns"),
            other => panic!("Expected Explore, got {:?}", other),
        }
    }

    #[test]
    fn test_route_description() {
        let intent = classify_intent("who calls optimize_deterministic");
        let desc = route_description(&intent);
        assert!(desc.contains("find_references"));
        assert!(desc.contains("optimize_deterministic"));
    }
}
