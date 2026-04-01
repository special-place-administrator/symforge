pub(crate) fn format_search_envelope(
    match_type: &str,
    source_authority: &str,
    parse_state: &str,
    completeness: &str,
    scope: &str,
    evidence: &str,
) -> String {
    format!(
        "Match type: {match_type}\nSource authority: {source_authority}\nParse state: {parse_state}\nCompleteness: {completeness}\nScope: {scope}\nEvidence: {evidence}"
    )
}

#[cfg(test)]
mod tests {
    use super::format_search_envelope;

    #[test]
    fn test_format_search_envelope() {
        let rendered = format_search_envelope(
            "constrained (literal)",
            "current index",
            "parsed",
            "full for current scope",
            "repo-wide; tests filtered; generated filtered",
            "line anchors `src/lib.rs:7`, `src/mod.rs:12`",
        );

        assert!(rendered.contains("Match type: constrained (literal)"));
        assert!(rendered.contains("Source authority: current index"));
        assert!(rendered.contains("Parse state: parsed"));
        assert!(rendered.contains("Completeness: full for current scope"));
        assert!(rendered.contains("Scope: repo-wide; tests filtered; generated filtered"));
        assert!(rendered.contains("Evidence: line anchors `src/lib.rs:7`, `src/mod.rs:12`"));
    }
}
