//! Generation-fence tests for `SharedIndexHandle` project identity.

use std::fs;
use std::path::Path;

use symforge::live_index::LiveIndex;
use tempfile::tempdir;

fn write_file(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn generation_fence_blocks_stale_remove() {
    let dir_a = tempdir().unwrap();
    write_file(dir_a.path(), "a/file.rs", "pub fn from_a() {}\n");
    let shared = LiveIndex::load(dir_a.path()).unwrap();
    let gen_a = shared.current_project_generation();

    let dir_b = tempdir().unwrap();
    write_file(dir_b.path(), "b/file.rs", "pub fn from_b() {}\n");
    shared.reload(dir_b.path()).unwrap();
    let count_after_reload = shared.read().file_count();

    let removed = shared.remove_file_at_generation("a/file.rs", gen_a);

    assert!(!removed, "stale generation must reject remove");
    assert_eq!(
        shared.read().file_count(),
        count_after_reload,
        "stale remove must leave the current index unchanged"
    );
}

#[test]
fn generation_fence_allows_current_remove() {
    let dir_a = tempdir().unwrap();
    write_file(dir_a.path(), "a/file.rs", "pub fn from_a() {}\n");
    let shared = LiveIndex::load(dir_a.path()).unwrap();

    let dir_b = tempdir().unwrap();
    write_file(dir_b.path(), "b/file.rs", "pub fn from_b() {}\n");
    shared.reload(dir_b.path()).unwrap();
    let gen_b = shared.current_project_generation();
    let count_before_remove = shared.read().file_count();

    let removed = shared.remove_file_at_generation("b/file.rs", gen_b);

    assert!(removed, "current generation must allow remove");
    let guard = shared.read();
    assert_eq!(
        guard.file_count(),
        count_before_remove - 1,
        "current remove must delete exactly one indexed file"
    );
    assert!(
        guard.get_file("b/file.rs").is_none(),
        "removed file must be absent from the current index"
    );
}

#[test]
fn generation_bumps_on_reload_only() {
    let dir_a = tempdir().unwrap();
    write_file(dir_a.path(), "src/update.rs", "pub fn before() {}\n");
    let shared = LiveIndex::load(dir_a.path()).unwrap();
    let gen_initial = shared.current_project_generation();
    let indexed = shared.read().get_file("src/update.rs").unwrap().clone();

    shared.update_file("src/update.rs".to_string(), indexed);

    assert_eq!(
        shared.current_project_generation(),
        gen_initial,
        "single-file update must not bump project generation"
    );

    let dir_b = tempdir().unwrap();
    write_file(dir_b.path(), "src/reloaded.rs", "pub fn after() {}\n");
    shared.reload(dir_b.path()).unwrap();

    assert!(
        shared.current_project_generation() > gen_initial,
        "reload must bump project generation"
    );
}
