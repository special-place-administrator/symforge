use std::fs;
use std::sync::Arc;

use tokenizor_agentic_mcp::application::search::{
    get_file_outline, get_repo_outline, get_symbol, get_symbols, search_symbols, search_text,
};
use tokenizor_agentic_mcp::application::{ApplicationContext, run_manager::RunManager};
use tokenizor_agentic_mcp::config::{BlobStoreConfig, ControlPlaneBackend, ServerConfig};
use tokenizor_agentic_mcp::domain::{
    BatchRetrievalRequest, BatchRetrievalResponseData, BatchRetrievalResultItem, FileOutcomeStatus,
    GetSymbolsResponse, IndexRunMode, IndexRunStatus, NextAction, PersistedFileOutcome, Provenance,
    Repository, RepositoryKind, RepositoryStatus, ResultEnvelope, RetrievalOutcome,
    SearchResultItem, SymbolKind, TrustLevel, VerifiedCodeSliceResponse, VerifiedSourceResponse,
};
use tokenizor_agentic_mcp::error::TokenizorError;
use tokenizor_agentic_mcp::storage::registry_persistence::RegistryPersistence;
use tokenizor_agentic_mcp::storage::{BlobStore, LocalCasBlobStore};

fn setup_test_env() -> (
    tempfile::TempDir,
    Arc<RunManager>,
    tempfile::TempDir,
    Arc<dyn BlobStore>,
) {
    let dir = tempfile::tempdir().unwrap();
    let registry_path = dir.path().join("registry.json");
    let persistence = RegistryPersistence::new(registry_path);
    let manager = Arc::new(RunManager::new(persistence));

    let cas_dir = tempfile::tempdir().unwrap();
    let cas: Arc<dyn BlobStore> = Arc::new(LocalCasBlobStore::new(BlobStoreConfig {
        root_dir: cas_dir.path().to_path_buf(),
    }));

    (dir, manager, cas_dir, cas)
}

fn setup_application_env() -> (tempfile::TempDir, ApplicationContext, Arc<dyn BlobStore>) {
    let dir = tempfile::tempdir().unwrap();
    let mut config = ServerConfig::default();
    config.blob_store.root_dir = dir.path().join(".tokenizor");
    config.control_plane.backend = ControlPlaneBackend::InMemory;
    config.runtime.require_ready_control_plane = false;

    let application = ApplicationContext::from_config(config.clone()).unwrap();
    application.initialize_local_storage().unwrap();

    let cas: Arc<dyn BlobStore> = Arc::new(LocalCasBlobStore::new(BlobStoreConfig {
        root_dir: config.blob_store.root_dir.clone(),
    }));
    cas.initialize().unwrap();

    (dir, application, cas)
}

fn register_repo(manager: &RunManager, repo_id: &str, status: RepositoryStatus) {
    let (quarantined_at_unix_ms, quarantine_reason) = if status == RepositoryStatus::Quarantined {
        (
            Some(1_709_827_200_000),
            Some("retrieval trust suspended".to_string()),
        )
    } else {
        (None, None)
    };
    let repo = Repository {
        repo_id: repo_id.to_string(),
        kind: RepositoryKind::Local,
        root_uri: "/tmp/test".to_string(),
        project_identity: "test-project".to_string(),
        project_identity_kind: Default::default(),
        default_branch: None,
        last_known_revision: None,
        status,
        invalidated_at_unix_ms: None,
        invalidation_reason: None,
        quarantined_at_unix_ms,
        quarantine_reason,
    };
    manager.persistence().save_repository(&repo).unwrap();
}

fn symbol_request(
    relative_path: &str,
    symbol_name: &str,
    kind_filter: Option<SymbolKind>,
) -> BatchRetrievalRequest {
    BatchRetrievalRequest::Symbol {
        relative_path: relative_path.to_string(),
        symbol_name: symbol_name.to_string(),
        kind_filter,
    }
}

fn code_slice_request(relative_path: &str, byte_range: (u32, u32)) -> BatchRetrievalRequest {
    BatchRetrievalRequest::CodeSlice {
        relative_path: relative_path.to_string(),
        byte_range,
    }
}

fn symbol_data(item: &BatchRetrievalResultItem) -> &VerifiedSourceResponse {
    let result = match item {
        BatchRetrievalResultItem::Symbol { result, .. } => result,
        other => panic!("expected symbol batch item, got: {other:?}"),
    };
    match result.data.as_ref().unwrap() {
        BatchRetrievalResponseData::Symbol(data) => data,
        other => panic!("expected symbol response data, got: {other:?}"),
    }
}

fn code_slice_data(item: &BatchRetrievalResultItem) -> &VerifiedCodeSliceResponse {
    let result = match item {
        BatchRetrievalResultItem::CodeSlice { result, .. } => result,
        other => panic!("expected code-slice batch item, got: {other:?}"),
    };
    match result.data.as_ref().unwrap() {
        BatchRetrievalResponseData::CodeSlice(data) => data,
        other => panic!("expected code-slice response data, got: {other:?}"),
    }
}

fn batch_result(
    item: &BatchRetrievalResultItem,
) -> &tokenizor_agentic_mcp::domain::ResultEnvelope<BatchRetrievalResponseData> {
    match item {
        BatchRetrievalResultItem::Symbol { result, .. } => result,
        BatchRetrievalResultItem::CodeSlice { result, .. } => result,
    }
}

async fn wait_for_run_success(manager: &RunManager, run_id: &str, timeout_ms: u64) {
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_millis(timeout_ms);
    loop {
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for run '{run_id}' to succeed after {timeout_ms}ms");
        }
        let report = manager
            .inspect_run(run_id)
            .unwrap_or_else(|err| panic!("failed to inspect run '{run_id}': {err}"));
        if report.run.status == IndexRunStatus::Succeeded {
            return;
        }
        if report.run.status.is_terminal() {
            panic!(
                "run '{run_id}' terminated with status {:?}: {:?}",
                report.run.status, report.run.error_summary
            );
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn test_index_then_search_returns_results() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(
        repo_dir.path().join("main.rs"),
        "fn main() {\n    println!(\"hello world\");\n}\n",
    )
    .unwrap();
    fs::write(
        repo_dir.path().join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )
    .unwrap();

    register_repo(&manager, "search-repo", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "search-repo",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    let result = search_text(
        "search-repo",
        "println",
        manager.persistence(),
        &manager,
        cas.as_ref(),
    )
    .unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    let data = result.data.unwrap();
    assert!(!data.is_empty());
    assert!(data[0].line_content.contains("println"));
    assert!(!data[0].provenance.run_id.is_empty());
    assert!(data[0].provenance.committed_at_unix_ms > 0);
}

#[tokio::test]
async fn test_search_with_quarantined_files_excludes_them() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(
        repo_dir.path().join("good.rs"),
        "fn good_function() { let x = 1; }\n",
    )
    .unwrap();

    register_repo(&manager, "q-repo", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "q-repo",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    // Manually quarantine one of the file records
    let run = manager
        .persistence()
        .get_latest_completed_run("q-repo")
        .unwrap()
        .unwrap();
    let mut records = manager.persistence().get_file_records(&run.run_id).unwrap();

    // Add a quarantined record
    if let Some(r) = records.first() {
        let mut quarantined = r.clone();
        quarantined.relative_path = "quarantined.rs".to_string();
        quarantined.outcome = PersistedFileOutcome::Quarantined {
            reason: "test quarantine".to_string(),
        };
        records.push(quarantined);
    }
    manager
        .persistence()
        .save_file_records(&run.run_id, &records)
        .unwrap();

    let result = search_text(
        "q-repo",
        "good_function",
        manager.persistence(),
        &manager,
        cas.as_ref(),
    )
    .unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    let data = result.data.unwrap();
    // No quarantined file results
    for item in &data {
        assert_ne!(item.relative_path, "quarantined.rs");
    }
}

#[tokio::test]
async fn test_search_against_invalidated_repo_is_rejected() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("main.rs"), "fn main() {}").unwrap();

    register_repo(&manager, "inv-repo", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "inv-repo",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    // Invalidate the repo
    manager
        .invalidate_repository("inv-repo", None, Some("test invalidation"))
        .unwrap();

    let result = search_text(
        "inv-repo",
        "main",
        manager.persistence(),
        &manager,
        cas.as_ref(),
    );

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        TokenizorError::RequestGated { .. }
    ));
}

#[test]
fn test_search_with_no_completed_runs_returns_never_indexed_gate_error() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();
    register_repo(&manager, "empty-repo", RepositoryStatus::Pending);

    let result = search_text(
        "empty-repo",
        "test",
        manager.persistence(),
        &manager,
        cas.as_ref(),
    );

    // Gate fires first with NeverIndexed — this is the expected behavior.
    // The defense-in-depth NotIndexed branch in search_text_ungated is tested
    // separately in the unit tests via search_text_ungated_returns_not_indexed.
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, TokenizorError::RequestGated { .. }));
    assert!(err.to_string().contains("has not been indexed"));
}

// --- Symbol search integration tests ---

#[tokio::test]
async fn test_index_then_symbol_search_returns_results() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(
        repo_dir.path().join("main.rs"),
        "fn main() {\n    println!(\"hello world\");\n}\n\nstruct MyStruct {\n    value: i32,\n}\n",
    )
    .unwrap();
    fs::write(
        repo_dir.path().join("lib.py"),
        "def add(a, b):\n    return a + b\n\nclass Calculator:\n    pass\n",
    )
    .unwrap();

    register_repo(&manager, "sym-search-repo", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "sym-search-repo",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    let result = search_symbols(
        "sym-search-repo",
        "MyStruct",
        None,
        manager.persistence(),
        &manager,
    )
    .unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    let data = result.data.unwrap();
    assert_eq!(data.matches.len(), 1);
    let first = &data.matches[0];
    assert_eq!(first.symbol_name, "MyStruct");
    assert_eq!(
        first.symbol_kind,
        tokenizor_agentic_mcp::domain::SymbolKind::Struct
    );
    assert_eq!(first.relative_path, "main.rs");
    assert_eq!(first.line_range, (4, 6));
    assert!(!first.provenance.run_id.is_empty());
    assert!(first.provenance.committed_at_unix_ms > 0);
}

#[tokio::test]
async fn test_symbol_search_returns_empty_with_coverage_metadata() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    register_repo(&manager, "sym-empty-repo", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "sym-empty-repo",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    let result = search_symbols(
        "sym-empty-repo",
        "nonexistent_symbol_xyz_12345",
        None,
        manager.persistence(),
        &manager,
    )
    .unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Empty);
    // data is Some with coverage even on Empty
    let data = result.data.unwrap();
    assert!(data.matches.is_empty());
    // Coverage should report files were searched
    assert!(data.coverage.files_searched > 0 || data.coverage.files_without_symbols > 0);
}

#[tokio::test]
async fn test_symbol_search_against_invalidated_repo_is_rejected() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("main.rs"), "fn main() {}").unwrap();

    register_repo(&manager, "sym-inv-repo", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "sym-inv-repo",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    manager
        .invalidate_repository("sym-inv-repo", None, Some("test invalidation"))
        .unwrap();

    let result = search_symbols(
        "sym-inv-repo",
        "main",
        None,
        manager.persistence(),
        &manager,
    );

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        TokenizorError::RequestGated { .. }
    ));
}

#[tokio::test]
async fn test_symbol_search_with_quarantined_files_excludes_them() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("good.rs"), "fn good_function() {}\n").unwrap();

    register_repo(&manager, "sym-q-repo", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "sym-q-repo",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    // Add a quarantined file record with symbols
    let run = manager
        .persistence()
        .get_latest_completed_run("sym-q-repo")
        .unwrap()
        .unwrap();
    let mut records = manager.persistence().get_file_records(&run.run_id).unwrap();

    if let Some(r) = records.first() {
        let mut quarantined = r.clone();
        quarantined.relative_path = "quarantined.rs".to_string();
        quarantined.outcome = PersistedFileOutcome::Quarantined {
            reason: "test quarantine".to_string(),
        };
        records.push(quarantined);
    }
    manager
        .persistence()
        .save_file_records(&run.run_id, &records)
        .unwrap();

    let result = search_symbols(
        "sym-q-repo",
        "good_function",
        None,
        manager.persistence(),
        &manager,
    )
    .unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    let data = result.data.unwrap();
    assert_eq!(data.coverage.files_skipped_quarantined, 1);
    assert!(!data.matches.is_empty());
    for item in &data.matches {
        assert_ne!(item.relative_path, "quarantined.rs");
    }
}

#[test]
fn test_symbol_search_with_no_completed_runs_returns_never_indexed() {
    let (_dir, manager, _cas_dir, _cas) = setup_test_env();
    register_repo(&manager, "sym-empty-runs", RepositoryStatus::Pending);

    let result = search_symbols(
        "sym-empty-runs",
        "test",
        None,
        manager.persistence(),
        &manager,
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, TokenizorError::RequestGated { .. }));
    assert!(err.to_string().contains("has not been indexed"));
}

// --- Outline integration tests ---

#[tokio::test]
async fn test_get_file_outline_returns_symbols() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(
        repo_dir.path().join("main.rs"),
        "fn main() {\n    println!(\"hello world\");\n}\n\nstruct MyStruct {\n    value: i32,\n}\n",
    )
    .unwrap();
    fs::write(
        repo_dir.path().join("lib.py"),
        "def add(a, b):\n    return a + b\n",
    )
    .unwrap();

    register_repo(&manager, "outline-int-1", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "outline-int-1",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    let result =
        get_file_outline("outline-int-1", "main.rs", manager.persistence(), &manager).unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    let data = result.data.unwrap();
    assert_eq!(data.relative_path, "main.rs");
    assert_eq!(data.symbols.len(), 2);
    assert_eq!(data.symbols[0].name, "main");
    assert_eq!(data.symbols[0].kind, SymbolKind::Function);
    assert_eq!(data.symbols[0].line_range, (0, 2));
    assert_eq!(data.symbols[0].sort_order, 0);
    assert_eq!(data.symbols[1].name, "MyStruct");
    assert_eq!(data.symbols[1].kind, SymbolKind::Struct);
    assert_eq!(data.symbols[1].line_range, (4, 6));
    assert_eq!(data.symbols[1].sort_order, 1);
    assert!(data.has_symbol_support);
    assert!(result.provenance.is_some());
}

#[tokio::test]
async fn test_get_file_outline_returns_explicit_empty_for_supported_language() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("empty.rs"), "// comment only\n").unwrap();

    register_repo(&manager, "outline-int-2", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "outline-int-2",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    let result =
        get_file_outline("outline-int-2", "empty.rs", manager.persistence(), &manager).unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    let data = result.data.unwrap();
    assert_eq!(data.relative_path, "empty.rs");
    assert!(data.symbols.is_empty());
    assert!(data.has_symbol_support);
}

#[tokio::test]
async fn test_get_file_outline_missing_file_returns_invalid_argument() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    register_repo(&manager, "outline-int-3", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "outline-int-3",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    let result = get_file_outline(
        "outline-int-3",
        "missing.rs",
        manager.persistence(),
        &manager,
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, TokenizorError::InvalidArgument(_)));
    assert!(
        err.to_string()
            .contains("file not found in index: missing.rs")
    );
}

#[tokio::test]
async fn test_get_repo_outline_returns_listing_with_coverage_metadata() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(repo_dir.path().join("empty.rs"), "// comment only\n").unwrap();

    register_repo(&manager, "outline-int-4", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "outline-int-4",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    let latest_run = manager
        .persistence()
        .get_latest_completed_run("outline-int-4")
        .unwrap()
        .unwrap();
    let mut records = manager
        .persistence()
        .get_file_records(&latest_run.run_id)
        .unwrap();
    if let Some(record) = records.first() {
        let mut quarantined = record.clone();
        quarantined.relative_path = "quarantined.rs".to_string();
        quarantined.outcome = PersistedFileOutcome::Quarantined {
            reason: "test quarantine".to_string(),
        };
        records.push(quarantined);
    }
    manager
        .persistence()
        .save_file_records(&latest_run.run_id, &records)
        .unwrap();

    let result = get_repo_outline("outline-int-4", manager.persistence(), &manager).unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    let data = result.data.unwrap();
    assert_eq!(data.files.len(), 3);
    assert_eq!(data.coverage.total_files, 3);
    assert_eq!(data.coverage.files_with_symbols, 1);
    assert_eq!(data.coverage.files_without_symbols, 1);
    assert_eq!(data.coverage.files_quarantined, 1);
    assert_eq!(data.coverage.files_failed, 0);
    let quarantined = data
        .files
        .iter()
        .find(|entry| entry.relative_path == "quarantined.rs")
        .unwrap();
    assert_eq!(quarantined.status, FileOutcomeStatus::Quarantined);
}

#[tokio::test]
async fn test_get_repo_outline_against_invalidated_repo_is_rejected() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    register_repo(&manager, "outline-int-5", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "outline-int-5",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;
    manager
        .invalidate_repository("outline-int-5", None, Some("test invalidation"))
        .unwrap();

    let result = get_repo_outline("outline-int-5", manager.persistence(), &manager);

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        TokenizorError::RequestGated { .. }
    ));
}

#[test]
fn test_get_repo_outline_with_no_completed_runs_returns_never_indexed() {
    let (_dir, manager, _cas_dir, _cas) = setup_test_env();
    register_repo(&manager, "outline-int-6", RepositoryStatus::Pending);

    let result = get_repo_outline("outline-int-6", manager.persistence(), &manager);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, TokenizorError::RequestGated { .. }));
    assert!(err.to_string().contains("has not been indexed"));
}

// --- Story 3.4: ApplicationContext retrieval delegation tests ---

#[tokio::test]
async fn test_application_search_text_delegates_correctly() {
    let (_dir, application, cas) = setup_application_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(
        repo_dir.path().join("main.rs"),
        "fn main() {\n    println!(\"hello world\");\n}\n",
    )
    .unwrap();

    register_repo(
        application.run_manager().as_ref(),
        "app-st-1",
        RepositoryStatus::Ready,
    );

    let (run, _progress) = application
        .run_manager()
        .launch_run(
            "app-st-1",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(application.run_manager().as_ref(), &run.run_id, 10_000).await;

    let result = application.search_text("app-st-1", "println").unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    assert_eq!(result.trust, TrustLevel::Verified);
    assert!(result.provenance.is_some());
    let prov = result.provenance.unwrap();
    assert!(!prov.run_id.is_empty());
    assert!(prov.committed_at_unix_ms > 0);
    assert_eq!(prov.repo_id, "app-st-1");
    let data = result.data.unwrap();
    assert!(!data.is_empty());
}

#[tokio::test]
async fn test_application_search_symbols_delegates_correctly() {
    let (_dir, application, cas) = setup_application_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(
        repo_dir.path().join("main.rs"),
        "fn main() {}\n\nstruct Foo {\n    x: i32,\n}\n",
    )
    .unwrap();

    register_repo(
        application.run_manager().as_ref(),
        "app-ss-1",
        RepositoryStatus::Ready,
    );

    let (run, _progress) = application
        .run_manager()
        .launch_run(
            "app-ss-1",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(application.run_manager().as_ref(), &run.run_id, 10_000).await;

    let result = application
        .search_symbols("app-ss-1", "Foo", Some(SymbolKind::Struct))
        .unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    assert_eq!(result.trust, TrustLevel::Verified);
    assert!(result.provenance.is_some());
    let data = result.data.unwrap();
    assert_eq!(data.matches.len(), 1);
    assert_eq!(data.matches[0].symbol_name, "Foo");
    assert_eq!(data.matches[0].symbol_kind, SymbolKind::Struct);
}

#[tokio::test]
async fn test_application_get_file_outline_delegates_correctly() {
    let (_dir, application, cas) = setup_application_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(
        repo_dir.path().join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )
    .unwrap();

    register_repo(
        application.run_manager().as_ref(),
        "app-fo-1",
        RepositoryStatus::Ready,
    );

    let (run, _progress) = application
        .run_manager()
        .launch_run(
            "app-fo-1",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(application.run_manager().as_ref(), &run.run_id, 10_000).await;

    let result = application.get_file_outline("app-fo-1", "lib.rs").unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    assert_eq!(result.trust, TrustLevel::Verified);
    assert!(result.provenance.is_some());
    let data = result.data.unwrap();
    assert_eq!(data.relative_path, "lib.rs");
    assert!(data.has_symbol_support);
    assert!(!data.symbols.is_empty());
}

#[tokio::test]
async fn test_application_get_repo_outline_delegates_correctly() {
    let (_dir, application, cas) = setup_application_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    register_repo(
        application.run_manager().as_ref(),
        "app-ro-1",
        RepositoryStatus::Ready,
    );

    let (run, _progress) = application
        .run_manager()
        .launch_run(
            "app-ro-1",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(application.run_manager().as_ref(), &run.run_id, 10_000).await;

    let result = application.get_repo_outline("app-ro-1").unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    assert_eq!(result.trust, TrustLevel::Verified);
    assert!(result.provenance.is_some());
    let data = result.data.unwrap();
    assert!(!data.files.is_empty());
    assert!(data.coverage.total_files > 0);
}

#[tokio::test]
async fn test_invalidated_repo_rejects_any_retrieval_method() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    register_repo(&manager, "app-inv-1", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "app-inv-1",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    manager
        .invalidate_repository("app-inv-1", None, Some("test"))
        .unwrap();

    // All four retrieval methods should return RequestGated
    let r1 = search_text(
        "app-inv-1",
        "main",
        manager.persistence(),
        &manager,
        cas.as_ref(),
    );
    assert!(matches!(
        r1.unwrap_err(),
        TokenizorError::RequestGated { .. }
    ));

    let r2 = search_symbols("app-inv-1", "main", None, manager.persistence(), &manager);
    assert!(matches!(
        r2.unwrap_err(),
        TokenizorError::RequestGated { .. }
    ));

    let r3 = get_file_outline("app-inv-1", "main.rs", manager.persistence(), &manager);
    assert!(matches!(
        r3.unwrap_err(),
        TokenizorError::RequestGated { .. }
    ));

    let r4 = get_repo_outline("app-inv-1", manager.persistence(), &manager);
    assert!(matches!(
        r4.unwrap_err(),
        TokenizorError::RequestGated { .. }
    ));
}

#[tokio::test]
async fn test_search_text_serialization_fidelity() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(
        repo_dir.path().join("main.rs"),
        "fn main() {\n    println!(\"hello\");\n}\n",
    )
    .unwrap();

    register_repo(&manager, "ser-fid-1", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "ser-fid-1",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    let result = search_text(
        "ser-fid-1",
        "println",
        manager.persistence(),
        &manager,
        cas.as_ref(),
    )
    .unwrap();

    // Serialize to JSON — same as MCP tool does
    let json = serde_json::to_string_pretty(&result).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Verify JSON contains all required envelope fields (AC 3)
    assert!(parsed.get("outcome").is_some(), "missing 'outcome' field");
    assert!(parsed.get("trust").is_some(), "missing 'trust' field");
    assert!(
        parsed.get("provenance").is_some(),
        "missing 'provenance' field"
    );
    assert!(parsed.get("data").is_some(), "missing 'data' field");

    // Verify provenance subfields
    let prov = parsed.get("provenance").unwrap();
    assert!(prov.get("run_id").is_some(), "missing 'provenance.run_id'");
    assert!(
        prov.get("committed_at_unix_ms").is_some(),
        "missing 'provenance.committed_at_unix_ms'"
    );
    assert!(
        prov.get("repo_id").is_some(),
        "missing 'provenance.repo_id'"
    );

    // Round-trip deserialization
    let deserialized: ResultEnvelope<Vec<SearchResultItem>> = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, result);
}

// --- Story 3.5: get_symbol integration tests ---

#[tokio::test]
async fn test_get_symbol_returns_verified_source_end_to_end() {
    let (_dir, application, cas) = setup_application_env();

    let repo_dir = tempfile::tempdir().unwrap();
    let rust_src = "fn main() {\n    println!(\"hello world\");\n}\n\nfn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
    fs::write(repo_dir.path().join("main.rs"), rust_src).unwrap();

    register_repo(
        application.run_manager().as_ref(),
        "gs-int-1",
        RepositoryStatus::Ready,
    );

    let (run, _progress) = application
        .run_manager()
        .launch_run(
            "gs-int-1",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(application.run_manager().as_ref(), &run.run_id, 10_000).await;

    let result = application
        .get_symbol("gs-int-1", "main.rs", "main", None)
        .unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    assert_eq!(result.trust, TrustLevel::Verified);
    assert!(result.provenance.is_some());
    let prov = result.provenance.unwrap();
    assert!(!prov.run_id.is_empty());
    assert!(prov.committed_at_unix_ms > 0);
    assert_eq!(prov.repo_id, "gs-int-1");

    let data = result.data.unwrap();
    assert_eq!(data.symbol_name, "main");
    assert_eq!(data.symbol_kind, SymbolKind::Function);
    assert_eq!(data.relative_path, "main.rs");
    assert!(data.source.contains("println"));
    assert!(!data.source.is_empty());
}

#[tokio::test]
async fn test_get_symbol_missing_symbol_returns_invalid_argument() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    register_repo(&manager, "gs-int-2", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "gs-int-2",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    let result = get_symbol(
        "gs-int-2",
        "main.rs",
        "nonexistent_sym",
        None,
        manager.persistence(),
        &manager,
        cas.as_ref(),
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, TokenizorError::InvalidArgument(_)));
    assert!(err.to_string().contains("symbol not found"));
    assert!(err.to_string().contains("nonexistent_sym"));
}

#[tokio::test]
async fn test_get_symbol_missing_file_returns_invalid_argument() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    register_repo(&manager, "gs-int-3", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "gs-int-3",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    let result = get_symbol(
        "gs-int-3",
        "nonexistent.rs",
        "main",
        None,
        manager.persistence(),
        &manager,
        cas.as_ref(),
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, TokenizorError::InvalidArgument(_)));
    assert!(err.to_string().contains("file not found in index"));
}

#[tokio::test]
async fn test_get_symbol_invalidated_repo_returns_request_gated() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    register_repo(&manager, "gs-int-4", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "gs-int-4",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    manager
        .invalidate_repository("gs-int-4", None, Some("test"))
        .unwrap();

    let result = get_symbol(
        "gs-int-4",
        "main.rs",
        "main",
        None,
        manager.persistence(),
        &manager,
        cas.as_ref(),
    );

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        TokenizorError::RequestGated { .. }
    ));
}

#[tokio::test]
async fn test_get_symbol_with_kind_filter_disambiguates() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    // Create a file with both a function and a struct with the same name
    fs::write(
        repo_dir.path().join("main.rs"),
        "fn Foo() {}\n\nstruct Foo {\n    x: i32,\n}\n",
    )
    .unwrap();

    register_repo(&manager, "gs-int-5", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "gs-int-5",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    // With kind_filter=Struct, should get the struct
    let result = get_symbol(
        "gs-int-5",
        "main.rs",
        "Foo",
        Some(SymbolKind::Struct),
        manager.persistence(),
        &manager,
        cas.as_ref(),
    )
    .unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    let data = result.data.unwrap();
    assert_eq!(data.symbol_kind, SymbolKind::Struct);
    assert_eq!(data.symbol_name, "Foo");
}

#[tokio::test]
async fn test_get_symbol_serialization_fidelity() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(
        repo_dir.path().join("main.rs"),
        "fn main() {\n    println!(\"hello\");\n}\n",
    )
    .unwrap();

    register_repo(&manager, "gs-int-6", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "gs-int-6",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    let result = get_symbol(
        "gs-int-6",
        "main.rs",
        "main",
        None,
        manager.persistence(),
        &manager,
        cas.as_ref(),
    )
    .unwrap();

    let json = serde_json::to_string_pretty(&result).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Verify envelope fields
    assert!(parsed.get("outcome").is_some(), "missing 'outcome'");
    assert!(parsed.get("trust").is_some(), "missing 'trust'");
    assert!(parsed.get("provenance").is_some(), "missing 'provenance'");
    assert!(parsed.get("data").is_some(), "missing 'data'");

    // Verify VerifiedSourceResponse fields inside data
    let data = parsed.get("data").unwrap();
    assert!(data.get("source").is_some(), "missing 'data.source'");
    assert!(
        data.get("symbol_name").is_some(),
        "missing 'data.symbol_name'"
    );
    assert!(
        data.get("symbol_kind").is_some(),
        "missing 'data.symbol_kind'"
    );
    assert!(
        data.get("relative_path").is_some(),
        "missing 'data.relative_path'"
    );
    assert!(
        data.get("line_range").is_some(),
        "missing 'data.line_range'"
    );
    assert!(
        data.get("byte_range").is_some(),
        "missing 'data.byte_range'"
    );
    assert!(data.get("language").is_some(), "missing 'data.language'");

    // Round-trip
    let deserialized: ResultEnvelope<VerifiedSourceResponse> = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, result);
}

// --- Story 3.6: Quarantine and NextAction integration tests ---

#[test]
fn test_quarantined_repo_blocks_all_retrieval() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();
    register_repo(&manager, "repo-1", RepositoryStatus::Quarantined);

    // search_text should be blocked
    let r1 = search_text(
        "repo-1",
        "anything",
        manager.persistence(),
        &manager,
        cas.as_ref(),
    );
    assert!(r1.is_err());
    let e1 = r1.unwrap_err().to_string();
    assert_eq!(
        e1,
        "request gated: repository quarantined: retrieval trust suspended [next_action: repair]"
    );

    // search_symbols should be blocked
    let r2 = search_symbols("repo-1", "anything", None, manager.persistence(), &manager);
    assert!(r2.is_err());
    let e2 = r2.unwrap_err().to_string();
    assert_eq!(
        e2,
        "request gated: repository quarantined: retrieval trust suspended [next_action: repair]"
    );

    // get_file_outline should be blocked
    let r3 = get_file_outline("repo-1", "any.rs", manager.persistence(), &manager);
    assert!(r3.is_err());
    let e3 = r3.unwrap_err().to_string();
    assert_eq!(
        e3,
        "request gated: repository quarantined: retrieval trust suspended [next_action: repair]"
    );

    // get_repo_outline should be blocked
    let r4 = get_repo_outline("repo-1", manager.persistence(), &manager);
    assert!(r4.is_err());
    let e4 = r4.unwrap_err().to_string();
    assert_eq!(
        e4,
        "request gated: repository quarantined: retrieval trust suspended [next_action: repair]"
    );

    // get_symbol should be blocked
    let r5 = get_symbol(
        "repo-1",
        "any.rs",
        "any",
        None,
        manager.persistence(),
        &manager,
        cas.as_ref(),
    );
    assert!(r5.is_err());
    let e5 = r5.unwrap_err().to_string();
    assert_eq!(
        e5,
        "request gated: repository quarantined: retrieval trust suspended [next_action: repair]"
    );
}

#[test]
fn test_blocked_result_includes_next_action_in_json() {
    let envelope: ResultEnvelope<String> = ResultEnvelope {
        outcome: RetrievalOutcome::Blocked {
            reason: "blob integrity check failed".to_string(),
        },
        trust: TrustLevel::Suspect,
        provenance: None,
        data: None,
        next_action: Some(NextAction::Reindex),
    };

    let json = serde_json::to_string(&envelope).unwrap();
    assert!(
        json.contains("\"next_action\":\"reindex\""),
        "expected '\"next_action\":\"reindex\"' in: {json}"
    );
}

#[test]
fn test_quarantined_file_result_includes_next_action_in_json() {
    let envelope: ResultEnvelope<String> = ResultEnvelope {
        outcome: RetrievalOutcome::Quarantined,
        trust: TrustLevel::Quarantined,
        provenance: None,
        data: None,
        next_action: Some(NextAction::Repair),
    };

    let json = serde_json::to_string(&envelope).unwrap();
    assert!(
        json.contains("\"next_action\":\"repair\""),
        "expected '\"next_action\":\"repair\"' in: {json}"
    );
}

#[test]
fn test_success_result_omits_next_action_in_json() {
    let envelope: ResultEnvelope<String> = ResultEnvelope {
        outcome: RetrievalOutcome::Success,
        trust: TrustLevel::Verified,
        provenance: Some(Provenance {
            run_id: "run-1".to_string(),
            committed_at_unix_ms: 1000,
            repo_id: "repo-1".to_string(),
        }),
        data: Some("hello".to_string()),
        next_action: None,
    };

    let json = serde_json::to_string(&envelope).unwrap();
    assert!(
        !json.contains("next_action"),
        "expected 'next_action' to be absent in: {json}"
    );
}

// =========================================================================
// Story 3.7: Batch retrieval integration tests
// =========================================================================

#[tokio::test]
async fn test_get_symbols_batch_verified_retrieval() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(
        repo_dir.path().join("main.rs"),
        "fn main() {\n    println!(\"hello world\");\n}\n\nfn helper() {\n    let x = 42;\n}\n",
    )
    .unwrap();
    fs::write(
        repo_dir.path().join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )
    .unwrap();

    register_repo(&manager, "batch-int", RepositoryStatus::Ready);

    let (run, _progress) = manager
        .launch_run(
            "batch-int",
            IndexRunMode::Full,
            repo_dir.path().to_path_buf(),
            cas.clone(),
        )
        .unwrap();

    wait_for_run_success(&manager, &run.run_id, 10_000).await;

    let requests = vec![
        symbol_request("main.rs", "main", None),
        code_slice_request("main.rs", (0, 12)),
        symbol_request("lib.rs", "add", None),
    ];

    let result = get_symbols(
        "batch-int",
        &requests,
        manager.persistence(),
        &manager,
        cas.as_ref(),
    )
    .unwrap();

    assert_eq!(result.outcome, RetrievalOutcome::Success);
    assert_eq!(result.trust, TrustLevel::Verified);
    assert!(result.provenance.is_some());

    let data = result.data.unwrap();
    assert_eq!(data.results.len(), 3);

    for (i, item) in data.results.iter().enumerate() {
        assert_eq!(
            batch_result(item).outcome,
            RetrievalOutcome::Success,
            "item {i} should be Success"
        );
        assert_eq!(
            batch_result(item).trust,
            TrustLevel::Verified,
            "item {i} trust"
        );
        assert!(
            batch_result(item).data.is_some(),
            "item {i} should have source data"
        );
    }

    assert_eq!(symbol_data(&data.results[0]).symbol_name, "main");
    assert_eq!(code_slice_data(&data.results[1]).byte_range, (0, 12));
    assert!(code_slice_data(&data.results[1]).source.contains("fn main"));
    assert_eq!(symbol_data(&data.results[2]).symbol_name, "add");
}

#[test]
fn test_get_symbols_batch_gate_failure_blocks_all() {
    let (_dir, manager, _cas_dir, cas) = setup_test_env();
    register_repo(&manager, "gate-batch", RepositoryStatus::Ready);

    // Create and complete a run
    let run = manager.start_run("gate-batch", IndexRunMode::Full).unwrap();
    manager
        .persistence()
        .transition_to_running(&run.run_id, 1000)
        .unwrap();
    manager
        .persistence()
        .update_run_status(&run.run_id, IndexRunStatus::Succeeded, None)
        .unwrap();

    // Invalidate the repo
    manager
        .invalidate_repository("gate-batch", None, Some("test invalidation"))
        .unwrap();

    let requests = vec![
        symbol_request("src/a.rs", "foo", None),
        symbol_request("src/b.rs", "bar", None),
    ];

    let result = get_symbols(
        "gate-batch",
        &requests,
        manager.persistence(),
        &manager,
        cas.as_ref(),
    );

    assert!(result.is_err(), "gate should reject entire batch");
    match result.unwrap_err() {
        TokenizorError::RequestGated { gate_error } => {
            assert!(gate_error.contains("invalidated"));
        }
        other => panic!("expected RequestGated, got: {other}"),
    }
}

#[test]
fn test_get_symbols_batch_mixed_outcomes_in_json() {
    // Build a batch response with mixed outcomes and verify JSON structure
    let response = GetSymbolsResponse {
        results: vec![
            BatchRetrievalResultItem::Symbol {
                relative_path: "src/a.rs".to_string(),
                symbol_name: "good".to_string(),
                kind_filter: None,
                result: ResultEnvelope {
                    outcome: RetrievalOutcome::Success,
                    trust: TrustLevel::Verified,
                    provenance: Some(Provenance {
                        run_id: "run-1".to_string(),
                        committed_at_unix_ms: 1000,
                        repo_id: "repo-1".to_string(),
                    }),
                    data: Some(BatchRetrievalResponseData::Symbol(VerifiedSourceResponse {
                        relative_path: "src/a.rs".to_string(),
                        language: tokenizor_agentic_mcp::domain::LanguageId::Rust,
                        symbol_name: "good".to_string(),
                        symbol_kind: SymbolKind::Function,
                        line_range: (0, 1),
                        byte_range: (0, 10),
                        source: "fn good() {}".to_string(),
                    })),
                    next_action: None,
                },
            },
            BatchRetrievalResultItem::CodeSlice {
                relative_path: "src/b.rs".to_string(),
                byte_range: (5, 15),
                result: ResultEnvelope {
                    outcome: RetrievalOutcome::Missing,
                    trust: TrustLevel::Verified,
                    provenance: Some(Provenance {
                        run_id: "run-1".to_string(),
                        committed_at_unix_ms: 1000,
                        repo_id: "repo-1".to_string(),
                    }),
                    data: None,
                    next_action: None,
                },
            },
        ],
    };

    let envelope: ResultEnvelope<GetSymbolsResponse> = ResultEnvelope {
        outcome: RetrievalOutcome::Success,
        trust: TrustLevel::Verified,
        provenance: Some(Provenance {
            run_id: "run-1".to_string(),
            committed_at_unix_ms: 1000,
            repo_id: "repo-1".to_string(),
        }),
        data: Some(response),
        next_action: None,
    };

    let json = serde_json::to_string_pretty(&envelope).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Verify outer envelope structure
    assert!(parsed.get("outcome").is_some(), "missing outer 'outcome'");
    assert!(parsed.get("trust").is_some(), "missing outer 'trust'");

    // Verify per-item structure
    let results = parsed["data"]["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);

    // First item: success
    assert_eq!(results[0]["request_type"], "symbol");
    assert_eq!(results[0]["result"]["outcome"], "success");
    assert_eq!(results[0]["result"]["trust"], "verified");
    assert!(
        results[0]["result"].get("next_action").is_none(),
        "success item should not have next_action"
    );

    // Second item: missing code slice
    assert_eq!(results[1]["request_type"], "code_slice");
    assert_eq!(results[1]["result"]["outcome"], "missing");
    assert_eq!(results[1]["result"]["trust"], "verified");
    assert!(results[1]["result"].get("next_action").is_none());
}
