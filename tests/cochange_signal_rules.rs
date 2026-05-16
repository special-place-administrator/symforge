use std::path::Path;

use symforge::live_index::coupling::{AnchorKey, CouplingRow, CouplingStore};
use symforge::live_index::rank_signals::{CoChangeSignal, RankCtx, RankSignal};

fn score_for(
    query: &str,
    token_values: &[&str],
    target_path: Option<&str>,
    count: Option<u32>,
    weighted_score: Option<f32>,
) -> f32 {
    let tokens: Vec<String> = token_values.iter().map(|token| token.to_string()).collect();
    let ctx = RankCtx {
        query,
        tokens: &tokens,
        current_file: None,
        target_path,
        co_change_count: count,
        co_change_weighted_score: weighted_score,
    };
    CoChangeSignal.score(Path::new("src/partner.rs"), &ctx)
}

fn score(count: Option<u32>, weighted_score: Option<f32>) -> f32 {
    score_for(
        "anchor.rs",
        &["anchor.rs"],
        Some("src/anchor.rs"),
        count,
        weighted_score,
    )
}

fn row(anchor: &str, partner: &str, shared: u32, weighted: f64) -> CouplingRow {
    CouplingRow {
        anchor: AnchorKey::file(anchor),
        partner: AnchorKey::file(partner),
        shared_commits: shared,
        weighted_score: weighted,
        last_commit_ts: 1_700_000_000,
    }
}

#[test]
fn rule1_excludes_pairs_below_file_level_shared_commit_floor() {
    assert_eq!(score(Some(1), Some(9.0)), 0.0);
}

#[test]
fn rule1_file_level_floor_allows_weighted_score() {
    assert_eq!(score(Some(2), Some(9.0)), 9.0);
}

#[test]
fn absent_co_change_inputs_return_zero() {
    assert_eq!(score(None, Some(9.0)), 0.0);
    assert_eq!(score(Some(0), Some(9.0)), 0.0);
}

#[test]
fn missing_weighted_score_is_safe_and_fail_closed() {
    assert_eq!(score(Some(3), None), 0.0);
}

#[test]
fn invalid_weighted_scores_are_safe_and_fail_closed() {
    for weighted_score in [Some(0.0), Some(-1.0), Some(f32::NAN), Some(f32::INFINITY)] {
        assert_eq!(score(Some(3), weighted_score), 0.0);
    }
}

#[test]
fn rule4_symbol_gate_withheld_score_fails_closed() {
    assert_eq!(score(Some(3), None), 0.0);
}

#[test]
fn rule3_chore_anchor_does_not_drive_score() {
    assert_eq!(
        score_for(
            "Cargo.lock",
            &["cargo.lock"],
            Some("Cargo.lock"),
            Some(120),
            Some(41.6),
        ),
        0.0
    );
    assert_eq!(
        score_for(
            "ci.yml",
            &["ci.yml"],
            Some(".github/workflows/ci.yml"),
            Some(20),
            Some(10.0),
        ),
        0.0
    );
}

#[test]
fn rule5_missing_or_weak_anchor_confidence_does_not_drive_score() {
    assert_eq!(
        score_for("anchor.rs", &["anchor.rs"], None, Some(3), Some(9.0)),
        0.0
    );
    assert_eq!(
        score_for("anc", &["anc"], Some("src/anchor.rs"), Some(3), Some(9.0)),
        0.0
    );
}

#[test]
fn rule6_tiny_relative_score_is_not_rejected_by_absolute_threshold() {
    let tiny_score = 0.000_001;
    assert_eq!(score(Some(2), Some(tiny_score)), tiny_score);
}

#[test]
fn rule2_partner_cap_uses_weighted_score_ordering_with_shared_floor() {
    let store = CouplingStore::open_in_memory().unwrap();
    let anchor = AnchorKey::file("src/anchor.rs");
    let mut rows = vec![row("src/anchor.rs", "src/too_weak.rs", 1, 10_000.0)];
    for n in 0..25 {
        rows.push(row(
            "src/anchor.rs",
            &format!("src/partner_{n:02}.rs"),
            2,
            f64::from(100 - n),
        ));
    }

    store.bulk_upsert(&rows).unwrap();

    let got = store.query_with_floor(&anchor, 20, 2).unwrap();

    assert_eq!(got.len(), 20);
    assert!(
        got.iter()
            .all(|row| row.partner.as_str() != "file:src/too_weak.rs")
    );
    assert_eq!(got[0].partner.as_str(), "file:src/partner_00.rs");
    assert_eq!(got[19].partner.as_str(), "file:src/partner_19.rs");
}
