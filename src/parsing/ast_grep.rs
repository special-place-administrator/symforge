//! Adapter between SymForge's `LanguageId` and `ast_grep_core` types.
//!
//! Implements `ast_grep_core::Language` and `LanguageExt` so we can use
//! ast-grep's structural pattern matching on indexed source files.

use crate::domain::index::LanguageId;
use ast_grep_core::language::Language;
use ast_grep_core::matcher::PatternBuilder;
use ast_grep_core::meta_var::MetaVariable;
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};
use ast_grep_core::{Pattern, PatternError};

/// Wrapper that implements ast-grep's `Language` trait for a SymForge `LanguageId`.
#[derive(Clone)]
pub struct SgLang {
    ts_lang: TSLanguage,
}

impl SgLang {
    /// Returns `None` for config-only languages (JSON, TOML, YAML, Markdown, Env)
    /// that have no meaningful AST patterns.
    pub fn from_language_id(lang: &LanguageId) -> Option<Self> {
        let ts_lang: TSLanguage = match lang {
            LanguageId::Rust => tree_sitter_rust::LANGUAGE.into(),
            LanguageId::Python => tree_sitter_python::LANGUAGE.into(),
            LanguageId::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            LanguageId::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            LanguageId::Go => tree_sitter_go::LANGUAGE.into(),
            LanguageId::Java => tree_sitter_java::LANGUAGE.into(),
            LanguageId::C => tree_sitter_c::LANGUAGE.into(),
            LanguageId::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            LanguageId::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            LanguageId::Ruby => tree_sitter_ruby::LANGUAGE.into(),
            LanguageId::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            LanguageId::Swift => tree_sitter_swift::LANGUAGE.into(),
            LanguageId::Perl => tree_sitter_perl::LANGUAGE.into(),
            LanguageId::Kotlin => tree_sitter_kotlin_sg::LANGUAGE.into(),
            LanguageId::Dart => tree_sitter_dart::language(),
            LanguageId::Elixir => tree_sitter_elixir::LANGUAGE.into(),
            LanguageId::Html => tree_sitter_html::LANGUAGE.into(),
            LanguageId::Css => tree_sitter_css::LANGUAGE.into(),
            LanguageId::Scss => tree_sitter_scss::language(),
            // Config-only languages — no structural patterns
            LanguageId::Json
            | LanguageId::Toml
            | LanguageId::Yaml
            | LanguageId::Markdown
            | LanguageId::Env => return None,
        };
        Some(Self { ts_lang })
    }
}

impl Language for SgLang {
    fn kind_to_id(&self, kind: &str) -> u16 {
        self.ts_lang.id_for_node_kind(kind, true)
    }

    fn field_to_id(&self, field: &str) -> Option<u16> {
        self.ts_lang.field_id_for_name(field).map(|f| f.get())
    }

    fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
        builder.build(|src| StrDoc::try_new(src, self.clone()))
    }
}

impl LanguageExt for SgLang {
    fn get_ts_language(&self) -> TSLanguage {
        self.ts_lang.clone()
    }
}

/// A single structural match result.
pub struct StructuralMatch {
    /// Byte offset of the match start in the source.
    pub start_byte: usize,
    /// Byte offset of the match end in the source.
    pub end_byte: usize,
    /// Zero-based start line.
    pub start_line: usize,
    /// Zero-based start column.
    pub start_col: usize,
    /// The matched text.
    pub text: String,
    /// Captured metavariables: (name, text).
    pub captures: Vec<(String, String)>,
}

/// Search `source` for occurrences of an ast-grep `pattern` in the given language.
///
/// Returns an error string if the pattern cannot be compiled (e.g., syntax error).
pub fn structural_search(
    source: &str,
    pattern_str: &str,
    lang: &LanguageId,
) -> Result<Vec<StructuralMatch>, String> {
    let sg_lang = SgLang::from_language_id(lang)
        .ok_or_else(|| format!("structural search not supported for {:?}", lang))?;

    let pattern = Pattern::try_new(pattern_str, sg_lang.clone())
        .map_err(|e| format!("invalid structural pattern: {e}"))?;

    let root = sg_lang.ast_grep(source);

    let matches: Vec<StructuralMatch> = root
        .root()
        .find_all(&pattern)
        .map(|node_match| {
            let start = node_match.start_pos();
            let text = node_match.text().to_string();

            // Extract metavariable captures
            let env = node_match.get_env();
            let captures: Vec<(String, String)> = env
                .get_matched_variables()
                .filter_map(|var| match var {
                    MetaVariable::Capture(name, _) => {
                        env.get_match(&name).map(|n| (name, n.text().to_string()))
                    }
                    MetaVariable::MultiCapture(name) => {
                        let nodes = env.get_multiple_matches(&name);
                        let combined: String = nodes
                            .iter()
                            .map(|n| n.text().to_string())
                            .collect::<Vec<_>>()
                            .join(", ");
                        if combined.is_empty() {
                            None
                        } else {
                            Some((name, combined))
                        }
                    }
                    _ => None,
                })
                .collect();

            StructuralMatch {
                start_byte: node_match.range().start,
                end_byte: node_match.range().end,
                start_line: start.line(),
                start_col: start.byte_point().1,
                text,
                captures,
            }
        })
        .collect();

    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structural_search_rust_function() {
        let source = r#"
fn hello() {
    println!("hello");
}
fn world(x: i32) {
    println!("{}", x);
}
"#;
        let matches =
            structural_search(source, "fn $NAME($$$) { $$$ }", &LanguageId::Rust)
                .expect("pattern should compile");
        assert_eq!(matches.len(), 2);
        assert!(matches[0].text.contains("hello"));
        assert!(matches[1].text.contains("world"));
    }

    #[test]
    fn test_structural_search_captures() {
        let source = "let x = 42;\nlet y = 100;";
        let matches = structural_search(source, "let $NAME = $VALUE", &LanguageId::Rust)
            .expect("pattern should compile");
        assert_eq!(matches.len(), 2);
        assert!(!matches[0].captures.is_empty());
    }

    #[test]
    fn test_structural_search_no_match() {
        let source = "fn main() {}";
        let matches = structural_search(source, "struct $NAME { $$$FIELDS }", &LanguageId::Rust)
            .expect("pattern should compile");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_structural_search_config_language_rejected() {
        let result = structural_search("{}", "{ $$$BODY }", &LanguageId::Json);
        assert!(result.is_err());
    }

    #[test]
    fn test_structural_search_javascript() {
        let source = "const x = 42;\nconst y = 100;";
        let matches =
            structural_search(source, "const $NAME = $VALUE", &LanguageId::JavaScript)
                .expect("pattern should compile");
        assert_eq!(matches.len(), 2);
    }
}
