# Class C Test Flakes — Windows libgit2 lockfile race

Pre-existing environmental flakes outside the scope of in-flight work.
Same family across entries: libgit2 fails to rename a refs/heads/* lockfile
during a tight commit loop on Windows. The race surfaces non-deterministically
and is not caused by SymForge logic.

Error signature:

    commit: Error { code: -1, klass: 2,
        message: "failed to rename lockfile to
            'C:/.../.git/refs/heads/master': Access is denied." }

## Tracked tests

- `tests/frecency_ranking.rs::head_change_resets_scores_at_1000_commits`
  - First observed: pre-Task 0.1 (frecency_ranking flake characterization).
  - Reproduction rate: 1/5 to 4/5 runs depending on system load.

- `src/live_index/persist.rs::tests::run_frecency_init_zeros_above_500_commits`
  - First observed during Task 0.2 verification (2026-05-11).
  - Not caused by Task 0.2 patch; sidecar change cannot reach persist.rs.
  - Same libgit2-lockfile race family.

- `tests/frecency_ranking.rs::head_change_halves_scores_at_100_commits`
  - First observed during Task 1.4 verification (2026-05-11).
  - Not caused by Task 1.4 patch; format-render change cannot reach git2.
  - Same libgit2-lockfile race family.

## Mitigation candidates (not yet scheduled)

- Wrap `init_repo_with_root_commit` helper retries around lockfile rename
  failure (3 retries, 50 ms backoff).
- Replace `git2::Repository::commit` with a process-spawned `git commit`
  call in the test helper to avoid libgit2 lockfile handling entirely.
- Pin the affected tests to `#[ignore = "libgit2 lockfile flake on Windows"]`
  and run them in a serial single-thread block.

## Vault sync

Needs vault sync — copy this entry to the Obsidian Class C tracker note
when next available.
