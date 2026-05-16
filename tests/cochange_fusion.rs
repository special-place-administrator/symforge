use std::collections::HashMap;
use std::fs;

use symforge::live_index::{
    LiveIndex, SearchFilesCouplingEvidence, SearchFilesTier, SearchFilesView,
};
use tempfile::TempDir;

fn write_routes_workspace() -> TempDir {
    let tmp = TempDir::new().unwrap();
    for dir in ["src/auth", "src/server", "src/client"] {
        fs::create_dir_all(tmp.path().join(dir)).unwrap();
    }
    fs::write(
        tmp.path().join("src/auth/routes.rs"),
        "pub fn auth_routes() {}\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("src/server/routes.rs"),
        "pub fn server_routes() {}\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("src/client/routes.rs"),
        "pub fn client_routes() {}\n",
    )
    .unwrap();
    tmp
}

fn paths(view: &SearchFilesView) -> Vec<&str> {
    match view {
        SearchFilesView::Found { hits, .. } => hits.iter().map(|hit| hit.path.as_str()).collect(),
        other => panic!("expected found view, got {other:?}"),
    }
}

fn first_hit(view: &SearchFilesView) -> (&str, SearchFilesTier, Option<f32>, Option<u32>) {
    match view {
        SearchFilesView::Found { hits, .. } => {
            let hit = hits.first().expect("at least one hit");
            (
                hit.path.as_str(),
                hit.tier,
                hit.coupling_score,
                hit.shared_commits,
            )
        }
        other => panic!("expected found view, got {other:?}"),
    }
}

#[test]
fn path_cochange_promotes_file_level_partner_with_shared_floor() {
    let tmp = write_routes_workspace();
    let shared = LiveIndex::load(tmp.path()).unwrap();
    let mut neighbors = HashMap::new();
    neighbors.insert(
        "src/server/routes.rs".to_string(),
        SearchFilesCouplingEvidence {
            shared_commits: 2,
            weighted_score: 9.0,
        },
    );

    let view = shared.read().capture_search_files_view(
        "routes.rs",
        10,
        None,
        Some(("src/auth/routes.rs", &neighbors)),
    );

    assert_eq!(
        first_hit(&view),
        (
            "src/server/routes.rs",
            SearchFilesTier::CoChange,
            Some(1.0),
            Some(2),
        )
    );
}

#[test]
fn path_cochange_excludes_pairs_below_shared_floor() {
    let tmp = write_routes_workspace();
    let shared = LiveIndex::load(tmp.path()).unwrap();
    let baseline = shared
        .read()
        .capture_search_files_view("routes.rs", 10, None, None);
    let mut neighbors = HashMap::new();
    neighbors.insert(
        "src/server/routes.rs".to_string(),
        SearchFilesCouplingEvidence {
            shared_commits: 1,
            weighted_score: 900.0,
        },
    );

    let view = shared.read().capture_search_files_view(
        "routes.rs",
        10,
        None,
        Some(("src/auth/routes.rs", &neighbors)),
    );

    assert_eq!(view, baseline);
}

#[test]
fn path_cochange_falls_back_when_anchor_confidence_is_weak() {
    let tmp = write_routes_workspace();
    let shared = LiveIndex::load(tmp.path()).unwrap();
    let baseline = shared
        .read()
        .capture_search_files_view("rou", 10, None, None);
    let mut neighbors = HashMap::new();
    neighbors.insert(
        "src/server/routes.rs".to_string(),
        SearchFilesCouplingEvidence {
            shared_commits: 4,
            weighted_score: 9.0,
        },
    );

    let view = shared.read().capture_search_files_view(
        "rou",
        10,
        None,
        Some(("src/auth/routes.rs", &neighbors)),
    );

    assert_eq!(view, baseline);
}

#[test]
fn path_cochange_keeps_tiny_relative_weighted_score_usable() {
    let tmp = write_routes_workspace();
    let shared = LiveIndex::load(tmp.path()).unwrap();
    let mut neighbors = HashMap::new();
    neighbors.insert(
        "src/server/routes.rs".to_string(),
        SearchFilesCouplingEvidence {
            shared_commits: 2,
            weighted_score: 0.000_001,
        },
    );

    let view = shared.read().capture_search_files_view(
        "routes.rs",
        10,
        None,
        Some(("src/auth/routes.rs", &neighbors)),
    );

    match view {
        SearchFilesView::Found { hits, .. } => {
            let hit = hits
                .iter()
                .find(|hit| hit.path == "src/server/routes.rs")
                .expect("co-change partner should remain in the result set");
            assert_eq!(hit.tier, SearchFilesTier::CoChange);
            assert_eq!(hit.coupling_score, Some(1.0));
            assert_eq!(hit.shared_commits, Some(2));
        }
        other => panic!("expected found view, got {other:?}"),
    }
}

#[test]
fn path_cochange_ignores_unmatched_neighbors_byte_identically() {
    let tmp = write_routes_workspace();
    let shared = LiveIndex::load(tmp.path()).unwrap();
    let baseline = shared
        .read()
        .capture_search_files_view("routes.rs", 10, None, None);
    let mut neighbors = HashMap::new();
    neighbors.insert(
        "src/other/routes.rs".to_string(),
        SearchFilesCouplingEvidence {
            shared_commits: 9,
            weighted_score: 100.0,
        },
    );

    let view = shared.read().capture_search_files_view(
        "routes.rs",
        10,
        None,
        Some(("src/auth/routes.rs", &neighbors)),
    );

    assert_eq!(view, baseline);
}

#[test]
fn path_cochange_preserves_path_tier_order_without_context() {
    let tmp = write_routes_workspace();
    let shared = LiveIndex::load(tmp.path()).unwrap();
    let baseline = shared
        .read()
        .capture_search_files_view("routes.rs", 10, None, None);

    let view = shared
        .read()
        .capture_search_files_view("routes.rs", 10, None, None);

    assert_eq!(paths(&view), paths(&baseline));
    assert_eq!(view, baseline);
}
