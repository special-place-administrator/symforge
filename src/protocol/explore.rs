//! Concept → pattern mapping for the `explore` tool.

/// A set of search patterns associated with a programming concept.
pub struct ConceptPattern {
    pub label: &'static str,
    pub symbol_queries: &'static [&'static str],
    pub text_queries: &'static [&'static str],
    pub kind_filters: &'static [&'static str],
}

const FALLBACK_STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "does", "for", "from", "has", "he",
    "how", "in", "is", "it", "its", "not", "of", "on", "or", "that", "the", "this", "to", "was",
    "were", "what", "when", "where", "which", "who", "why", "will", "with",
];

// Sorted by key length descending so longer/more-specific keys match first.
pub const CONCEPT_MAP: &[(&str, ConceptPattern)] = &[
    (
        "actor supervision",
        ConceptPattern {
            label: "Actor Supervision",
            symbol_queries: &[
                "Actor",
                "ActorRef",
                "supervisor",
                "supervision",
                "mailbox",
                "spawn",
                "message",
            ],
            text_queries: &[
                "handle_supervisor_evt",
                "SupervisionEvent",
                "ActorProcessingErr",
                "Actor::spawn",
                "ActorRef",
            ],
            kind_filters: &["struct", "fn", "impl"],
        },
    ),
    (
        "error handling",
        ConceptPattern {
            label: "Error Handling",
            symbol_queries: &["Error", "Result", "anyhow", "bail", "catch"],
            text_queries: &["unwrap()", ".expect(", "return Err(", "try {", "catch"],
            kind_filters: &["struct", "enum", "fn"],
        },
    ),
    (
        "file watching",
        ConceptPattern {
            label: "File Watching",
            symbol_queries: &["watcher", "notify", "debounce", "burst"],
            text_queries: &["notify::Event", "DebouncedEvent", "file_event", "inotify"],
            kind_filters: &[],
        },
    ),
    (
        "serialization",
        ConceptPattern {
            label: "Serialization",
            symbol_queries: &["serialize", "deserialize", "serde", "json", "postcard"],
            text_queries: &[
                "#[derive(Serialize",
                "#[derive(Deserialize",
                "serde_json",
                "postcard::",
            ],
            kind_filters: &[],
        },
    ),
    (
        "authentication",
        ConceptPattern {
            label: "Authentication",
            symbol_queries: &[
                "auth",
                "login",
                "session",
                "token",
                "credential",
                "password",
            ],
            text_queries: &["Bearer", "JWT", "OAuth", "verify_token", "authenticate"],
            kind_filters: &[],
        },
    ),
    (
        "configuration",
        ConceptPattern {
            label: "Configuration",
            symbol_queries: &["config", "settings", "env", "options", "params"],
            text_queries: &["dotenv", "env::var", "process.env", "serde", "toml", "yaml"],
            kind_filters: &["struct"],
        },
    ),
    (
        "concurrency",
        ConceptPattern {
            label: "Concurrency",
            symbol_queries: &["Mutex", "RwLock", "Atomic", "channel", "spawn", "async"],
            text_queries: &[
                "tokio::spawn",
                "thread::spawn",
                ".lock()",
                ".read()",
                ".write()",
            ],
            kind_filters: &[],
        },
    ),
    (
        "permissions",
        ConceptPattern {
            label: "Permissions / Authorization",
            symbol_queries: &["permission", "role", "policy", "acl", "authorize"],
            text_queries: &["forbidden", "unauthorized", "access_control", "RBAC"],
            kind_filters: &[],
        },
    ),
    (
        "deployment",
        ConceptPattern {
            label: "Deployment / Release",
            symbol_queries: &["release", "deploy", "version", "publish", "migrate"],
            text_queries: &[
                "npm publish",
                "cargo publish",
                "release-please",
                "changelog",
            ],
            kind_filters: &[],
        },
    ),
    (
        "networking",
        ConceptPattern {
            label: "Networking",
            symbol_queries: &["socket", "listener", "bind", "connect", "server"],
            text_queries: &["TcpListener", "hyper", "axum", "reqwest", "tonic"],
            kind_filters: &[],
        },
    ),
    (
        "database",
        ConceptPattern {
            label: "Database",
            symbol_queries: &[
                "query",
                "migrate",
                "schema",
                "pool",
                "connection",
                "transaction",
            ],
            text_queries: &[
                "SELECT",
                "INSERT",
                "CREATE TABLE",
                "sqlx",
                "diesel",
                "TypeORM",
            ],
            kind_filters: &[],
        },
    ),
    (
        "indexing",
        ConceptPattern {
            label: "Indexing",
            symbol_queries: &["index", "reindex", "snapshot", "persist"],
            text_queries: &["LiveIndex", "index.bin", "reindex", "rebuild_reverse"],
            kind_filters: &[],
        },
    ),
    (
        "testing",
        ConceptPattern {
            label: "Testing",
            symbol_queries: &["test", "mock", "fixture", "assert", "expect"],
            text_queries: &["#[test]", "#[tokio::test]", "describe(", "it(", "pytest"],
            kind_filters: &["fn"],
        },
    ),
    (
        "parsing",
        ConceptPattern {
            label: "Parsing",
            symbol_queries: &["parse", "parser", "ast", "node", "tree_sitter"],
            text_queries: &["tree_sitter::", ".parse(", "syntax tree", "grammar"],
            kind_filters: &[],
        },
    ),
    (
        "caching",
        ConceptPattern {
            label: "Caching",
            symbol_queries: &["cache", "lru", "memoize", "ttl", "expire"],
            text_queries: &["LruCache", "cache.get(", "cached::", "moka::"],
            kind_filters: &[],
        },
    ),
    (
        "logging",
        ConceptPattern {
            label: "Logging / Observability",
            symbol_queries: &["log", "trace", "span", "metric", "telemetry"],
            text_queries: &["tracing::", "log::", "debug!", "warn!", "info!"],
            kind_filters: &[],
        },
    ),
    (
        "api",
        ConceptPattern {
            label: "API / HTTP",
            symbol_queries: &[
                "handler",
                "route",
                "endpoint",
                "controller",
                "request",
                "response",
            ],
            text_queries: &[
                "GET", "POST", "PUT", "DELETE", "Router", "axum", "actix", "express",
            ],
            kind_filters: &["fn"],
        },
    ),
    (
        "cli",
        ConceptPattern {
            label: "CLI / Command Line",
            symbol_queries: &["cli", "args", "command", "subcommand"],
            text_queries: &["clap", "structopt", "Arg::", "Command::new"],
            kind_filters: &[],
        },
    ),
];

/// Lightweight English word stemmer for concept matching.
/// Strips common suffixes so inflected queries ("errors", "handling", "serialization")
/// match concept keys ("error", "handling", "serialization").
pub fn stem_word(word: &str) -> String {
    let w = word.to_ascii_lowercase();
    // Longest suffixes first to avoid partial stripping.
    for (suffix, min_base) in &[
        ("ization", 3usize),
        ("isation", 3),
        ("ation", 3),
        ("tion", 3),
        ("sion", 3),
        ("ment", 3),
        ("ness", 3),
        ("ible", 3),
        ("able", 3),
        ("ize", 3),
        ("ise", 3),
        ("ing", 3),
        ("ed", 3),
        ("er", 3),
        ("ly", 3),
        ("es", 3),
        ("s", 3),
    ] {
        if let Some(base) = w.strip_suffix(suffix) {
            if base.len() >= *min_base && !(*suffix == "s" && w.ends_with("ss")) {
                return base.to_string();
            }
        }
    }
    w
}

/// Check whether two words match after stemming, using exact-stem or tight prefix overlap.
/// Prefix overlap (min 4 chars, max 2 char difference) handles -ing → base vs base+e
/// ("handl" ↔ "handle") without false positives like "data" ↔ "database".
fn stems_match(a: &str, b: &str) -> bool {
    let sa = stem_word(a);
    let sb = stem_word(b);
    if sa == sb {
        return true;
    }
    let min_len = sa.len().min(sb.len());
    let max_len = sa.len().max(sb.len());
    min_len >= 4
        && max_len - min_len <= 2
        && (sa.starts_with(sb.as_str()) || sb.starts_with(sa.as_str()))
}

/// Find the best matching concept for a query.
/// Returns the matched key and the corresponding pattern, or `None` if no concept matches.
/// Uses word-boundary matching to avoid substring collisions (e.g. "clinical" should not match "cli").
/// Falls back to stemmed matching when exact words don't match.
pub fn match_concept(query: &str) -> Option<(&'static str, &'static ConceptPattern)> {
    let query_words: Vec<&str> = query.split_whitespace().collect();

    // Exact word-boundary match (original behavior).
    let exact = CONCEPT_MAP.iter().find(|(key, _)| {
        let key_words: Vec<&str> = key.split_whitespace().collect();
        query_words.windows(key_words.len()).any(|window| {
            window
                .iter()
                .zip(key_words.iter())
                .all(|(qw, kw)| qw.eq_ignore_ascii_case(kw))
        })
    });
    if let Some((key, pattern)) = exact {
        return Some((*key, pattern));
    }

    // Stemmed fallback with bag-of-words matching: each key word must match some query
    // word after stemming.  Order-independent so "handle errors" matches "error handling".
    CONCEPT_MAP
        .iter()
        .find(|(key, _)| {
            let key_words: Vec<&str> = key.split_whitespace().collect();
            key_words
                .iter()
                .all(|kw| query_words.iter().any(|qw| stems_match(qw, kw)))
        })
        .map(|(key, pattern)| (*key, pattern))
}

/// For queries that don't match a concept, split into search terms.
pub fn fallback_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != ':' && c != '-')
                .to_ascii_lowercase()
        })
        .filter(|w| w.len() >= 3)
        .filter(|w| !FALLBACK_STOP_WORDS.contains(&w.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_concept_finds_error_handling() {
        let concept = match_concept("error handling patterns");
        assert!(concept.is_some());
        assert_eq!(concept.unwrap().1.label, "Error Handling");
    }

    #[test]
    fn test_match_concept_case_insensitive() {
        let concept = match_concept("Error Handling");
        assert!(concept.is_some());
        assert_eq!(concept.unwrap().1.label, "Error Handling");
    }

    #[test]
    fn test_match_concept_returns_none_for_unknown() {
        let concept = match_concept("quantum entanglement");
        assert!(concept.is_none());
    }

    #[test]
    fn test_fallback_terms_splits_query() {
        let terms = fallback_terms("process data handler");
        assert_eq!(terms, vec!["process", "data", "handler"]);
    }

    #[test]
    fn test_fallback_terms_filters_short_words() {
        let terms = fallback_terms("a bb ccc");
        assert_eq!(terms, vec!["ccc"]);
    }

    #[test]
    fn test_fallback_terms_filters_stop_words_and_punctuation() {
        let terms = fallback_terms("how does actor supervision and error recovery work?");
        assert_eq!(
            terms,
            vec!["actor", "supervision", "error", "recovery", "work"]
        );
    }

    #[test]
    fn test_match_concept_finds_actor_supervision() {
        let concept = match_concept("actor supervision and error recovery");
        assert!(concept.is_some());
        assert_eq!(concept.unwrap().1.label, "Actor Supervision");
    }

    #[test]
    fn test_match_concept_word_boundary_no_substring() {
        assert!(match_concept("clinical trial data").is_none());
        assert!(match_concept("capital investment").is_none());
        assert!(match_concept("cli tools").is_some());
        assert!(match_concept("api endpoints").is_some());
    }

    #[test]
    fn test_stem_word_common_suffixes() {
        assert_eq!(stem_word("errors"), "error");
        assert_eq!(stem_word("handling"), "handl");
        assert_eq!(stem_word("serialization"), "serial");
        assert_eq!(stem_word("serialize"), "serial");
        assert_eq!(stem_word("parsed"), "pars");
        assert_eq!(stem_word("caching"), "cach");
        assert_eq!(stem_word("deployments"), "deployment"); // strips -s first
    }

    #[test]
    fn test_stem_word_preserves_short_words() {
        assert_eq!(stem_word("cli"), "cli");
        assert_eq!(stem_word("api"), "api");
        assert_eq!(stem_word("log"), "log");
    }

    #[test]
    fn test_stems_match_inflected_variants() {
        assert!(stems_match("error", "errors"));
        assert!(stems_match("handle", "handling"));
        assert!(stems_match("parse", "parsing"));
        assert!(stems_match("serialize", "serialization"));
    }

    #[test]
    fn test_stems_match_rejects_short_prefix_overlap() {
        // "cli" vs "clinical" — short stem, should not prefix-match
        assert!(!stems_match("cli", "clinical"));
    }

    #[test]
    fn test_match_concept_stemmed_fallback() {
        // "handle errors" should match "error handling" via stemming
        let concept = match_concept("handle errors");
        assert!(concept.is_some());
        assert_eq!(concept.unwrap().1.label, "Error Handling");
    }

    #[test]
    fn test_match_concept_stemmed_serialized() {
        let concept = match_concept("serialized data");
        assert!(concept.is_some());
        assert_eq!(concept.unwrap().1.label, "Serialization");
    }
}
