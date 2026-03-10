//! HTTP endpoint handlers for the tokenizor sidecar.
//!
//! All handlers follow this contract:
//!  - Accept `State(index): State<SharedIndex>` plus optional `Query(params)`.
//!  - Acquire `index.read()`, extract owned data, drop the guard, then return `Json`.
//!  - Never hold a `RwLockReadGuard` across an `.await` point.
//!  - On lock poison: return `StatusCode::INTERNAL_SERVER_ERROR`.
//!  - On file not found: return `StatusCode::NOT_FOUND`.

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::live_index::store::SharedIndex;

// ---------------------------------------------------------------------------
// Request parameter structs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct OutlineParams {
    pub path: String,
}

#[derive(Deserialize)]
pub struct ImpactParams {
    pub path: String,
}

#[derive(Deserialize)]
pub struct SymbolContextParams {
    pub name: String,
    /// Optional: restrict search to a specific file.
    pub file: Option<String>,
}

// ---------------------------------------------------------------------------
// Response value types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct HealthResponse {
    pub file_count: usize,
    pub symbol_count: usize,
    pub index_state: String,
    pub uptime_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: String,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Serialize)]
pub struct ReferenceInfo {
    pub line: u32,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct FileReferences {
    pub file: String,
    pub references: Vec<ReferenceInfo>,
}

#[derive(Serialize)]
pub struct SymbolContextRef {
    pub line: u32,
    pub kind: String,
    /// Name of the enclosing symbol, if any.
    pub enclosing: Option<String>,
}

#[derive(Serialize)]
pub struct SymbolContextEntry {
    pub file: String,
    pub references: Vec<SymbolContextRef>,
}

#[derive(Serialize)]
pub struct RepoMapEntry {
    pub path: String,
    pub symbol_count: usize,
    pub parse_status: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /health` — index state, file count, symbol count, uptime.
pub async fn health_handler(
    State(index): State<SharedIndex>,
) -> Result<Json<HealthResponse>, StatusCode> {
    let guard = index.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let file_count = guard.file_count();
    let symbol_count = guard.symbol_count();
    let state_str = format!("{:?}", guard.index_state());
    // loaded_at_system is a SystemTime; uptime is time since it was set.
    let uptime_secs = guard
        .loaded_at_system()
        .elapsed()
        .unwrap_or_default()
        .as_secs();

    drop(guard);

    Ok(Json(HealthResponse {
        file_count,
        symbol_count,
        index_state: state_str,
        uptime_secs,
    }))
}

/// `GET /outline?path=<relative>` — symbol outline for a single file.
pub async fn outline_handler(
    State(index): State<SharedIndex>,
    Query(params): Query<OutlineParams>,
) -> Result<Json<Vec<SymbolInfo>>, StatusCode> {
    let guard = index.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Verify the file exists.
    if guard.get_file(&params.path).is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let symbols: Vec<SymbolInfo> = guard
        .symbols_for_file(&params.path)
        .iter()
        .map(|s| SymbolInfo {
            name: s.name.clone(),
            kind: format!("{}", s.kind),
            start_line: s.line_range.0,
            end_line: s.line_range.1,
        })
        .collect();

    drop(guard);
    Ok(Json(symbols))
}

/// `GET /impact?path=<relative>` — files that import (depend on) the given file.
pub async fn impact_handler(
    State(index): State<SharedIndex>,
    Query(params): Query<ImpactParams>,
) -> Result<Json<Vec<FileReferences>>, StatusCode> {
    let guard = index.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if guard.get_file(&params.path).is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Collect owned data before dropping the guard.
    let raw = guard.find_dependents_for_file(&params.path);

    // Group by file path.
    let mut map: std::collections::HashMap<String, Vec<ReferenceInfo>> = std::collections::HashMap::new();
    for (file_path, reference) in &raw {
        map.entry(file_path.to_string()).or_default().push(ReferenceInfo {
            line: reference.line_range.0,
            name: reference.name.clone(),
        });
    }

    drop(guard);

    let mut result: Vec<FileReferences> = map
        .into_iter()
        .map(|(file, references)| FileReferences { file, references })
        .collect();
    result.sort_by(|a, b| a.file.cmp(&b.file));

    Ok(Json(result))
}

/// `GET /symbol-context?name=<name>[&file=<path>]` — all references to a named symbol.
///
/// Capped at 50 results total.
pub async fn symbol_context_handler(
    State(index): State<SharedIndex>,
    Query(params): Query<SymbolContextParams>,
) -> Result<Json<Vec<SymbolContextEntry>>, StatusCode> {
    let guard = index.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let raw = guard.find_references_for_name(&params.name, None, false);

    // Group by file, applying the optional file filter.
    let mut map: std::collections::HashMap<String, Vec<SymbolContextRef>> =
        std::collections::HashMap::new();

    let mut total = 0usize;
    for (file_path, reference) in &raw {
        if total >= 50 {
            break;
        }
        if let Some(ref filter_file) = params.file {
            if *file_path != filter_file.as_str() {
                continue;
            }
        }
        // Resolve enclosing symbol name if index is available.
        let enclosing = reference
            .enclosing_symbol_index
            .and_then(|idx| {
                guard
                    .get_file(file_path)
                    .and_then(|f| f.symbols.get(idx as usize))
                    .map(|s| s.name.clone())
            });

        map.entry(file_path.to_string()).or_default().push(SymbolContextRef {
            line: reference.line_range.0,
            kind: format!("{}", reference.kind),
            enclosing,
        });
        total += 1;
    }

    drop(guard);

    let mut result: Vec<SymbolContextEntry> = map
        .into_iter()
        .map(|(file, references)| SymbolContextEntry { file, references })
        .collect();
    result.sort_by(|a, b| a.file.cmp(&b.file));

    Ok(Json(result))
}

/// `GET /repo-map` — summary of all indexed files.
pub async fn repo_map_handler(
    State(index): State<SharedIndex>,
) -> Result<Json<Vec<RepoMapEntry>>, StatusCode> {
    use crate::live_index::store::ParseStatus;

    let guard = index.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut entries: Vec<RepoMapEntry> = guard
        .all_files()
        .map(|(path, file)| RepoMapEntry {
            path: path.clone(),
            symbol_count: file.symbols.len(),
            parse_status: match &file.parse_status {
                ParseStatus::Parsed => "parsed".to_string(),
                ParseStatus::PartialParse { .. } => "partial".to_string(),
                ParseStatus::Failed { .. } => "failed".to_string(),
            },
        })
        .collect();

    drop(guard);

    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Json(entries))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};
    use std::time::{Duration, Instant, SystemTime};

    use crate::domain::{LanguageId, ReferenceKind, ReferenceRecord, SymbolKind, SymbolRecord};
    use crate::live_index::store::{
        CircuitBreakerState, IndexedFile, LiveIndex, ParseStatus,
    };

    // -----------------------------------------------------------------------
    // Test helper: minimal LiveIndex with known contents
    // -----------------------------------------------------------------------

    fn make_symbol(name: &str, kind: SymbolKind, start: u32, end: u32) -> SymbolRecord {
        SymbolRecord {
            name: name.to_string(),
            kind,
            depth: 0,
            sort_order: 0,
            byte_range: (0, 10),
            line_range: (start, end),
        }
    }

    fn make_reference(name: &str, kind: ReferenceKind, line: u32) -> ReferenceRecord {
        ReferenceRecord {
            name: name.to_string(),
            qualified_name: None,
            kind,
            byte_range: (100, 110),
            line_range: (line, line),
            enclosing_symbol_index: None,
        }
    }

    fn make_indexed_file(
        path: &str,
        symbols: Vec<SymbolRecord>,
        references: Vec<ReferenceRecord>,
        status: ParseStatus,
    ) -> IndexedFile {
        IndexedFile {
            relative_path: path.to_string(),
            language: LanguageId::Rust,
            content: b"fn test() {}".to_vec(),
            symbols,
            parse_status: status,
            byte_len: 12,
            content_hash: "abc".to_string(),
            references,
            alias_map: HashMap::new(),
        }
    }

    fn build_shared_index(files: Vec<(&str, IndexedFile)>) -> SharedIndex {
        let mut index = LiveIndex {
            files: files.into_iter().map(|(p, f)| (p.to_string(), f)).collect(),
            loaded_at: Instant::now(),
            loaded_at_system: SystemTime::now(),
            load_duration: Duration::from_millis(10),
            cb_state: CircuitBreakerState::new(0.20),
            is_empty: false,
            reverse_index: HashMap::new(),
        };
        index.rebuild_reverse_index();
        Arc::new(RwLock::new(index))
    }

    // -----------------------------------------------------------------------
    // health_handler
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_health_handler_returns_counts() {
        let f1 = make_indexed_file(
            "src/main.rs",
            vec![make_symbol("main", SymbolKind::Function, 1, 10)],
            vec![],
            ParseStatus::Parsed,
        );
        let f2 = make_indexed_file(
            "src/lib.rs",
            vec![
                make_symbol("foo", SymbolKind::Function, 1, 5),
                make_symbol("bar", SymbolKind::Function, 7, 12),
            ],
            vec![],
            ParseStatus::Parsed,
        );
        let index = build_shared_index(vec![("src/main.rs", f1), ("src/lib.rs", f2)]);

        let result = health_handler(State(index)).await.unwrap();
        let body = result.0;
        assert_eq!(body.file_count, 2, "health should report 2 files");
        assert_eq!(body.symbol_count, 3, "health should report 3 symbols");
        assert!(
            body.index_state.contains("Ready"),
            "index_state should include Ready"
        );
    }

    #[tokio::test]
    async fn test_health_handler_empty_index() {
        let index = build_shared_index(vec![]);
        let result = health_handler(State(index)).await.unwrap();
        let body = result.0;
        assert_eq!(body.file_count, 0);
        assert_eq!(body.symbol_count, 0);
    }

    // -----------------------------------------------------------------------
    // outline_handler
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_outline_handler_returns_symbols() {
        let file = make_indexed_file(
            "src/foo.rs",
            vec![
                make_symbol("alpha", SymbolKind::Function, 1, 5),
                make_symbol("Beta", SymbolKind::Struct, 7, 10),
            ],
            vec![],
            ParseStatus::Parsed,
        );
        let index = build_shared_index(vec![("src/foo.rs", file)]);

        let params = OutlineParams {
            path: "src/foo.rs".to_string(),
        };
        let result = outline_handler(State(index), Query(params)).await.unwrap();
        let symbols = result.0;
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "alpha");
        assert_eq!(symbols[1].name, "Beta");
        assert_eq!(symbols[1].kind, "struct");
    }

    #[tokio::test]
    async fn test_outline_handler_not_found_for_missing_file() {
        let index = build_shared_index(vec![]);
        let params = OutlineParams {
            path: "nonexistent.rs".to_string(),
        };
        let err = outline_handler(State(index), Query(params)).await.unwrap_err();
        assert_eq!(err, StatusCode::NOT_FOUND);
    }

    // -----------------------------------------------------------------------
    // impact_handler
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_impact_handler_returns_importers() {
        // b.rs imports "db" — should appear as dependent of src/db.rs
        let db_file = make_indexed_file("src/db.rs", vec![], vec![], ParseStatus::Parsed);
        let b_file = make_indexed_file(
            "src/b.rs",
            vec![],
            vec![make_reference("db", ReferenceKind::Import, 1)],
            ParseStatus::Parsed,
        );
        let index = build_shared_index(vec![("src/db.rs", db_file), ("src/b.rs", b_file)]);

        let params = ImpactParams {
            path: "src/db.rs".to_string(),
        };
        let result = impact_handler(State(index), Query(params)).await.unwrap();
        let body = result.0;
        assert_eq!(body.len(), 1, "one file imports db");
        assert_eq!(body[0].file, "src/b.rs");
        assert_eq!(body[0].references[0].name, "db");
    }

    #[tokio::test]
    async fn test_impact_handler_not_found_for_missing_file() {
        let index = build_shared_index(vec![]);
        let params = ImpactParams {
            path: "missing.rs".to_string(),
        };
        let err = impact_handler(State(index), Query(params)).await.unwrap_err();
        assert_eq!(err, StatusCode::NOT_FOUND);
    }

    // -----------------------------------------------------------------------
    // symbol_context_handler
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_symbol_context_handler_returns_references() {
        let f = make_indexed_file(
            "src/main.rs",
            vec![],
            vec![make_reference("process", ReferenceKind::Call, 5)],
            ParseStatus::Parsed,
        );
        let index = build_shared_index(vec![("src/main.rs", f)]);

        let params = SymbolContextParams {
            name: "process".to_string(),
            file: None,
        };
        let result = symbol_context_handler(State(index), Query(params))
            .await
            .unwrap();
        let body = result.0;
        assert_eq!(body.len(), 1);
        assert_eq!(body[0].file, "src/main.rs");
        assert_eq!(body[0].references[0].line, 5);
        assert_eq!(body[0].references[0].kind, "call");
    }

    #[tokio::test]
    async fn test_symbol_context_handler_caps_at_50() {
        // Create 60 files each with one reference to "target".
        let files: Vec<(&str, IndexedFile)> = (0..60usize)
            .map(|i| {
                let path = Box::leak(format!("src/f{i}.rs").into_boxed_str()) as &'static str;
                let file = make_indexed_file(
                    path,
                    vec![],
                    vec![make_reference("target", ReferenceKind::Call, 1)],
                    ParseStatus::Parsed,
                );
                (path, file)
            })
            .collect();
        let index = build_shared_index(files);

        let params = SymbolContextParams {
            name: "target".to_string(),
            file: None,
        };
        let result = symbol_context_handler(State(index), Query(params))
            .await
            .unwrap();
        let total_refs: usize = result.0.iter().map(|e| e.references.len()).sum();
        assert!(total_refs <= 50, "total references must be capped at 50, got {total_refs}");
    }

    // -----------------------------------------------------------------------
    // repo_map_handler
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_repo_map_handler_returns_all_files() {
        let f1 = make_indexed_file(
            "a.rs",
            vec![make_symbol("x", SymbolKind::Function, 1, 3)],
            vec![],
            ParseStatus::Parsed,
        );
        let f2 = make_indexed_file("b.rs", vec![], vec![], ParseStatus::Failed {
            error: "oops".to_string(),
        });
        let index = build_shared_index(vec![("a.rs", f1), ("b.rs", f2)]);

        let result = repo_map_handler(State(index)).await.unwrap();
        let entries = result.0;
        assert_eq!(entries.len(), 2, "should return all 2 files");
        // Sorted by path: a.rs then b.rs
        assert_eq!(entries[0].path, "a.rs");
        assert_eq!(entries[0].symbol_count, 1);
        assert_eq!(entries[0].parse_status, "parsed");
        assert_eq!(entries[1].parse_status, "failed");
    }

    #[tokio::test]
    async fn test_repo_map_handler_empty_index() {
        let index = build_shared_index(vec![]);
        let result = repo_map_handler(State(index)).await.unwrap();
        assert!(result.0.is_empty(), "empty index returns empty repo map");
    }
}
