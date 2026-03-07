use std::fs;
use std::sync::Arc;

use tokenizor_agentic_mcp::domain::{IndexRunMode, IndexRunStatus};
use tokenizor_agentic_mcp::storage::registry_persistence::RegistryPersistence;
use tokenizor_agentic_mcp::application::run_manager::RunManager;

#[tokio::test]
async fn test_launch_run_transitions_queued_running_succeeded() {
    let dir = tempfile::tempdir().unwrap();
    let registry_path = dir.path().join("registry.json");
    let persistence = RegistryPersistence::new(registry_path);
    let manager = Arc::new(RunManager::new(persistence));

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("main.rs"), "fn main() {}").unwrap();
    fs::write(repo_dir.path().join("lib.py"), "def foo(): pass").unwrap();

    let (run, progress) = manager
        .launch_run("test-repo", IndexRunMode::Full, repo_dir.path().to_path_buf())
        .unwrap();

    assert_eq!(run.status, IndexRunStatus::Queued);

    // Wait for background task to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let finished_run = manager.persistence().find_run(&run.run_id).unwrap().unwrap();
    assert_eq!(finished_run.status, IndexRunStatus::Succeeded);
    assert!(finished_run.finished_at_unix_ms.is_some());

    assert_eq!(
        progress.total_files.load(std::sync::atomic::Ordering::Relaxed),
        2
    );
    assert_eq!(
        progress.files_processed.load(std::sync::atomic::Ordering::Relaxed),
        2
    );

    // Active run should be deregistered
    assert!(!manager.has_active_run("test-repo"));
}

#[tokio::test]
async fn test_single_file_failure_does_not_poison_run() {
    let dir = tempfile::tempdir().unwrap();
    let registry_path = dir.path().join("registry.json");
    let persistence = RegistryPersistence::new(registry_path);
    let manager = Arc::new(RunManager::new(persistence));

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("good.rs"), "fn good() {}").unwrap();
    fs::write(repo_dir.path().join("also_good.py"), "def also(): pass").unwrap();
    // A syntactically broken file will result in PartialParse, not Failed.
    // Files that successfully parse but have errors are still "processed".
    // The run should still succeed.
    fs::write(repo_dir.path().join("broken.rs"), "fn broken( { }").unwrap();

    let (run, _progress) = manager
        .launch_run("test-repo", IndexRunMode::Full, repo_dir.path().to_path_buf())
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let finished_run = manager.persistence().find_run(&run.run_id).unwrap().unwrap();
    // Even with a partial parse, the run should succeed (not fail)
    assert_eq!(finished_run.status, IndexRunStatus::Succeeded);
}
