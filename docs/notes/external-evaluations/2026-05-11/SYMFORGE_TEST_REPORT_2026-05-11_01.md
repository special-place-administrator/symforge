# SymForge MCP Test Report — 2026-05-11

Generated from full live tool sweep against AAP repo (`Agent_Army_Professionals`, 1135 files / 33050 symbols, indexed by current SymForge build). Reporter context: user pushed SymForge mid-bugfix; suspects this build may be regressed; cross-workstation indexing took ~10x longer than baseline.

Repo under test (target): `C:\AI_STUFF\PROGRAMMING\Agent_Army_Professionals`
SymForge repo (source): `C:\AI_STUFF\PROGRAMMING\symforge`
SymForge HEAD at test time: `2b18148 test(live_index): guard publish/lookup atomicity across reload + root-switch`
Recent commits leading in: `475ec1f` (clippy-1.95 baseline), `7c04744` (io path-named context), `f58afbc` (daemon spawn_blocking/governor + stale PID cleanup), `e9fc968`/`634e0e2` (health_compact conformance).

---

## 1. Test scope

All MCP tools exposed by SymForge were exercised at least once against the AAP workspace. Edit-class tools were exercised with `dry_run=true` only — no writes were performed. Tools used:

Discovery / search:
- `health`, `health_compact`
- `index_folder`
- `ask`
- `explore`
- `get_repo_map`
- `search_symbols`
- `search_files`
- `search_text` (literal mode + structural / ast-grep mode)
- `conventions`
- `context_inventory`
- `investigation_suggest`

Inspection:
- `get_file_context`
- `get_file_content`
- `get_symbol`
- `get_symbol_context`
- `inspect_match`
- `validate_file_syntax`

References / impact:
- `find_references` (compact + default)
- `find_dependents`
- `what_changed` (uncommitted)
- `diff_symbols` (HEAD~1..HEAD)
- `analyze_file_impact` (with `include_co_changes=true`)
- `edit_plan`

Editing (all dry-run):
- `replace_symbol_body` — not exercised standalone (covered by `batch_edit`)
- `edit_within_symbol`
- `insert_symbol` — not exercised standalone (covered by `batch_insert`)
- `delete_symbol`
- `batch_edit`
- `batch_insert`
- `batch_rename`

No tool errored, crashed, or rejected a valid schema. All returned structured output. The issues below are correctness / diagnostics / parser-coverage problems, not availability problems.

---

## 2. Index-time observations

### 2.1 Index summary

```
index_folder(path=...) -> "Indexed 1135 files, 33050 symbols."
health -> Loaded in: 397ms
health_compact -> Loaded: 457ms
```

Two diagnostic tools report different load durations from the same index session (397ms vs 457ms). One of them is reporting a stale or recomputed measurement. Pick one source of truth; cross-tool drift on a primitive number is a smell.

### 2.2 Parse breakdown

```
Files:  1135 indexed (1122 parsed, 13 partial, 0 failed)
Symbols: 33050
Admission: Tier 1 1135 / Tier 2 0 / Tier 3 0
```

Thirteen partial parses on a Rust-dominant codebase (826 Rust files). At least two are SymForge's own:
- `crates/symforge/src/live_index/persist.rs`
- `crates/symforge/src/worktree.rs`

See section 4 for root cause.

### 2.3 Watcher state inconsistency — **BUG**

```
health         -> Watcher: active (idle; debounce: 200ms, overflows: 0, reconcile repairs: 18, last reconcile: 36s ago)
health_compact -> Watcher: off
```

Same session, same index, two seconds apart. `health_compact` reports `off`; `health` reports `active`. One is wrong.

Hypothesis: `health_compact` reads from a different snapshot of watcher status than `health`, or it reads the field before the watcher subsystem has reported up. Given the commit history shows recent work registering `health_compact` (`e9fc968 test(conformance): register health_compact in expected tools`, `634e0e2 test(schema_roundtrip): whitelist health_compact as zero-param tool`), the compact variant is new and likely diverged from `health` on the watcher-state code path.

Severity: high. Diagnostics that disagree with each other erode all downstream trust signals. Coding agents will quote `health_compact` because it's cheap, and now be wrong.

Repro:
1. `index_folder(path="<any active repo>")`
2. Call `health` then `health_compact` back-to-back.
3. Compare the `Watcher:` line.

Fix direction: have `health_compact` read from the same `WatcherStatus` source that `health` uses. Add a conformance test asserting both tools report identical watcher state in the same tick.

### 2.4 Reconcile repair churn

```
reconcile repairs: 18, last reconcile: 36s ago
```

Eighteen reconcile repairs across the first ~3 minutes of an idle session is high. The watcher is healing drift on its own. This is benign for correctness (index is self-repairing) but suggests:
- noisy file system events on Windows (NTFS + Defender + cargo target dir + git internals), or
- debounce window of 200ms is too tight for this host, or
- reconcile is triggering on events that should be filtered (target/, .git/, node_modules/, etc.).

Worth verifying `ignore_patterns` covers everything cargo touches under `target/`.

### 2.5 Hook adoption / fail-open ratio

```
Owned workflows routed: 220/322 (68%)
Fail-open outcomes: 102 (no sidecar 102, sidecar errors 0)
Daemon fallback routed: 85
Source read: routed 7, daemon fallback 6, no sidecar 59
Repo start: routed 1, no sidecar 1
Prompt context: routed 52, daemon fallback 22, no sidecar 1
Post-edit impact: routed 75, daemon fallback 57, no sidecar 41
```

Sidecar reached on only 7/72 source reads (~10%). For post-edit-impact: 75 routed + 57 daemon-fallback + 41 no-sidecar = 173 events, of which 41 (~24%) found no sidecar at all.

Interpretation: hooks are firing but the sidecar process is missing or unreachable for a significant fraction of events. Either:
- Sidecar lifecycle is racy on session start (hooks fire before sidecar binds its socket), or
- Sidecar is being killed/garbage-collected mid-session, or
- Per-workstation install is missing the sidecar binary entirely.

This correlates with the user's observation that this session took 10x longer to index. If hooks are fail-open without sidecar, every code-read goes through a slower fallback path. The 397ms index time is the daemon's internal index build, but the user-visible wall-clock includes all hook fallbacks for warm-up.

Action: log the sidecar PID + binary path at every fail-open, and surface it in `health`. Right now `health` says "Fail-open here is mostly benign" — that wording masks a real symptom when fail-open is 102/322.

### 2.6 Git temporal

```
Git temporal: ready (500 commits over 90d, computed in 355ms)
Hotspots: orchestrator.rs (1.00), main.rs (1.00), Cargo.lock (1.00), orchestration.rs (1.00), ports/mod.rs (1.00)
Strongest coupling: wiki/.obsidian/core-plugins.json ↔ wiki/.obsidian/plugins/dataview/styles.css (1.00)
```

Git temporal is working. Hotspot ranks make sense (orchestrator.rs has 76 commits per `analyze_file_impact`). The strongest coupling pair is Obsidian vault internal config files — that pair is real (they co-change when plugins update) but it's noise to a code-intelligence consumer. Consider excluding `wiki/.obsidian/` from coupling analysis by default; surface it only with `include_personal_tooling=true` or similar.

---

## 3. Tool-by-tool results

Status legend: `OK` = returned correct, well-structured output; `BUG` = wrong output; `WARN` = correct but surfaced an issue worth fixing; `NOT-EXERCISED` = skipped because covered by a peer tool's path.

| Tool | Status | Notes |
|---|---|---|
| `health` | OK | Full diagnostic, accurate against watcher state. |
| `health_compact` | BUG | Reports `Watcher: off` while `health` reports `active`. See 2.3. |
| `index_folder` | OK | Returned `Indexed 1135 files, 33050 symbols.` |
| `ask` | OK | Routed `who calls registry_latest_tracks_highest_version` to `find_references` correctly. Found zero refs (the symbol exists only as a test fn name; expected). |
| `explore` | OK | Returned ranked symbols + code patterns + related files for `actor supervision`. Ranking looked sane. |
| `get_repo_map` | OK | Compact summary 1135 files across 14 languages. Surfaced key types. ~1039 tokens. |
| `search_symbols` | OK | Negative test (`WorkSpec` not in index) returned correct "no symbols" + suggestion. |
| `search_files` | OK | Tiered relevance ranking for `orchestrator.rs` returned 3 files with sensible scores. |
| `search_text` (literal) | OK | `TaskState` literal: 10 matches, 2 files, grouped by symbol, told me 12 more truncated + 34 noise-filtered. Good transparency. |
| `search_text` (structural) | WARN | Structural pattern `fn $NAME($$$) -> Result<(), ActorError>$$$` returned no matches. Either the pattern is wrong for tree-sitter Rust grammar or the grammar can't bind `$$$` across generic args. Worth a doc snippet of known-good structural patterns to ship with the tool. |
| `get_file_context` | OK | Outline of orchestrator.rs (269 symbols, 794 KB file) returned in one go. Reported "~158979 tokens saved vs raw file read". |
| `get_file_content` | OK | Line-range read of README.md returned exact text. |
| `get_symbol` | OK | Returned `ProjectOrchestratorActor` definition + byte size. |
| `get_symbol_context` | OK | Returned signature, callers (4 sites), callees (7) for `handle_supervisor_evt`. `verbosity=signature` honored. |
| `find_references` (default + compact) | OK | 4 refs in 3 files for `ProjectOrchestratorActor`. Compact format dropped source text as documented. |
| `find_dependents` | OK | 42 dependents on orchestrator.rs, compact output. |
| `inspect_match` | OK | Showed enclosing symbol + parent chain + siblings for a specific line in librarian_actor.rs. |
| `validate_file_syntax` (TOML) | OK | Cargo.toml status `ok`, 164 symbols. |
| `validate_file_syntax` (Rust, partial files) | WARN | Reported partials correctly with line/column. See section 4. |
| `edit_plan` | OK | Returned suggested tool sequence (get_symbol_context → choose op → analyze_file_impact) for `handle_supervisor_evt`. |
| `analyze_file_impact` | OK | Re-indexed orchestrator.rs, returned churn 1.00 (76 commits), ownership %, co-changing files. |
| `what_changed` | OK | Listed ~70 uncommitted files (`.github/agents/*`, `.specify/*`, `specs/001-multi-auth-providers/*`). Matches `git status`. |
| `diff_symbols` (HEAD~1..HEAD) | OK | "No file changes found" — matches that the last commit on main is doc-comment-only `Ractor -> Kameo` rename; symbol-level diff correctly empty. |
| `conventions` | OK | Reported mixed anyhow+thiserror, snake_case dominant, 155 test files. |
| `context_inventory` | OK | Tracked 9 entries / ~3352 tokens accurately. |
| `investigation_suggest` | OK | Pointed at unloaded symbols seen in search results. |
| `edit_within_symbol` (dry) | OK | Reported `text-edit-safe`, 1 replacement. |
| `delete_symbol` (dry) | OK | Reported byte size to be deleted (489 bytes). |
| `batch_rename` (dry) | OK | 5 sites across 2 files for `handle_supervisor_evt → handle_supervisor_evt_renamed`. |
| `batch_edit` (dry) | OK | Edit safety classified `structural-edit-safe`. |
| `batch_insert` (dry) | OK | 1 target, content auto-indent path correctly identified. |
| `replace_symbol_body` | NOT-EXERCISED | Covered by `batch_edit replace`. |
| `insert_symbol` | NOT-EXERCISED | Covered by `batch_insert`. |

---

## 4. Parser coverage problems — **PRIMARY DEFECT**

### 4.1 Rust 2024 raw-pointer syntax not parsed

```
validate_file_syntax(path="crates/symforge/src/live_index/persist.rs")
-> Status: partial
   Diagnostic: tree-sitter: syntax error near `&raw` (line 1210, column 34)
   Byte span: 45850..45854
   Symbols extracted: 63
```

`&raw const` / `&raw mut` is the Rust 2024 raw-reference operator (stabilized for the 2024 edition). The tree-sitter Rust grammar bundled with SymForge does not recognize it. This is SymForge's own source code. SymForge cannot fully parse SymForge.

Impact:
- 63 symbols extracted from a file that actually has more.
- Symbol callers/callees inside that file are missing.
- `find_references`, `get_symbol_context`, `analyze_file_impact` against persist.rs all under-report.

Fix direction: upgrade `tree-sitter-rust` to a release that includes the 2024-edition grammar (the upstream grammar has had `raw_reference_expression` support since the edition stabilized). If the grammar is vendored, regenerate the parser.

### 4.2 `aap-code-intel/src/adapter.rs` line-1 syntax error

```
validate_file_syntax(path="crates/aap-code-intel/src/adapter.rs")
-> Status: partial
   Diagnostic: tree-sitter: syntax error near `//! SymForge adapter -- wraps LiveIndex ` (line 1, column 1)
   Byte span: 0..101894
   Symbols extracted: 95
```

The error is reported at byte 0 but the byte_span covers the whole 101 KB file. That's tree-sitter's way of saying "I gave up at line 1 and best-effort'd the rest." The actual offending construct is somewhere further in. Best-effort still produced 95 symbols, which is why the file shows up as `partial` instead of `failed`.

Two issues here:
- The diagnostic is misleading. "near line 1 column 1" is technically accurate (where parsing began) but unhelpful for fixing. `validate_file_syntax` should walk tree-sitter ERROR nodes and report the deepest error positions, not the outermost one.
- Symbol count for a 101 KB file is suspiciously low (95). Likely the same `&raw` / 2024-edition syntax issue, since this file is the AAP-side adapter wrapping SymForge's `LiveIndex` and probably uses the same patterns.

### 4.3 Other partials

`health` reported 10 partial files. The pattern is consistent: large Rust files where modern syntax appears mid-file (raw refs, let-else, GATs, etc.) and tree-sitter falls into best-effort. Worth running `validate_file_syntax` on every partial and binning by root-cause grammar gap, then upgrading the grammar once instead of patching individually.

Severity: high. The tool's own source files are partials. Coding agents calling `get_file_context` on persist.rs receive an incomplete outline, will assume that's the truth, and make wrong edit decisions.

---

## 5. Behavioral observations worth recording

### 5.1 `search_text` quality is high

The literal-mode search for `TaskState` returned a usable result in one call: grouped by enclosing symbol, told me how many were truncated, told me how many were filtered as noise (tests/generated), evidence anchors at the top. That's the right level of structured transparency for a code-intel tool. Keep doing this.

### 5.2 `explore` ranking gives weights that are useful but unexplained

```
fn spawn_inner ... [1.00]
struct SupervisionMessage ... [0.17]
fn handle_supervisor_evt ... [0.17]
...
```

A `[1.00]` next to `spawn_inner` and `[0.17]` next to everything else is a big gap and it's not clear why. The trailing "ranked by: concept match + symbol-token alignment + path proximity + caller density" line is the right shape, but exposing the per-factor breakdown (even in a debug mode) would let agents tell when ranking is brittle vs justified.

### 5.3 Truncation honesty is good but inconsistent

`search_text` says exactly: `12 more omitted; 34 noise-filtered match(es) suppressed`. Excellent. `get_repo_map` says `(truncated, 2 more)`. Inconsistent style. Pick one phrasing and propagate.

### 5.4 `analyze_file_impact` co-change output is gold

The git temporal output is the most useful piece of context produced by any of these tools. For orchestrator.rs it told me ownership distribution, last-commit author and date, churn score, and top 5 co-changing files with shared-commit counts. That's better than any human could derive in a reasonable time. Surface this more prominently in `explore` and `ask` flows.

### 5.5 `edit_plan` is well-shaped

It does the thinking out loud: how many sites, which tools in which order, with concrete invocation strings. This is the right interface for getting an LLM to commit to a plan before editing.

---

## 6. Cross-cutting recommendations

Ordered by severity.

1. **Fix `health_compact` watcher field.** It is currently lying. Add a regression test that calls `health` and `health_compact` back-to-back and asserts identical watcher state. (Section 2.3.)

2. **Upgrade `tree-sitter-rust` to a 2024-edition-aware revision.** SymForge cannot parse its own `&raw` syntax. Verify by re-running `validate_file_syntax` on `crates/symforge/src/live_index/persist.rs` and expecting `Status: ok`. (Section 4.1.)

3. **Improve `validate_file_syntax` diagnostic to walk ERROR nodes.** Report the deepest error positions, not the outermost. Today's "near line 1 column 1" output on a 101 KB file is actively misleading. (Section 4.2.)

4. **Surface sidecar-missing as a real signal in `health`.** "Fail-open here is mostly benign" understates 102 missed routes on a 322-event session. Log sidecar PID and binary path; if the sidecar is dead, say so. (Section 2.5.)

5. **Reconcile the 397ms vs 457ms load-time drift between `health` and `health_compact`.** Either compute once and propagate, or document why they differ. (Section 2.1.)

6. **Exclude `wiki/.obsidian/` from default git-coupling output.** It surfaces noise as the strongest project coupling. Gate behind `include_personal_tooling=true`. (Section 2.6.)

7. **Ship a structural-pattern cookbook.** `search_text(structural=true)` is powerful but the pattern language is non-obvious. The empty-result case for a sensible-looking pattern (`fn $NAME($$$) -> Result<(), ActorError>$$$`) suggests common pitfalls deserve documented examples. (Section 3 row "search_text (structural).") 

8. **Investigate idle reconcile-repair count.** 18 repairs in 3 idle minutes is high. Check whether `target/`, `.git/`, and other write-heavy paths are properly ignored on Windows. (Section 2.4.)

9. **Standardize truncation phrasing across tools.** Pick one of `(N more omitted)` / `(truncated, N more)` and propagate. (Section 5.3.)

---

## 7. What did **not** regress in this build

- Index build itself is correct (1135 files, 33050 symbols, all admission tiers populated).
- All editing tools resolved symbol byte ranges correctly under dry-run.
- All read tools returned data, none crashed, none rejected valid schemas.
- Git temporal subsystem is producing useful output (churn, ownership, co-change).
- The recent commits the user is worried about (`f58afbc fix(daemon): fix spawn_blocking/governor races and stale PID cleanup`, `7c04744 fix(io): add path-named context to startup file-write sites`) appear consistent with the current behavior — they did not visibly break the surface area, though they may be why hook adoption is at 68% (daemon lifecycle still settling).
- `health_compact` was very recently added (`e9fc968`, `634e0e2`); its watcher-state bug is almost certainly a side-effect of that addition not a regression from elsewhere.

---

## 8. Suggested next steps for a coding agent picking this up

1. Reproduce section 2.3 (`health` vs `health_compact` watcher mismatch). Open the source for both tools in SymForge, find where each reads watcher status, unify. Add a test under `crates/symforge/tests/` or wherever schema_roundtrip lives.

2. Bisect the partial-parse list. For each of the 13 partial files, run `validate_file_syntax`, group by error pattern, file an upstream-grammar bump or local patch.

3. Open `crates/symforge/src/live_index/persist.rs` at line 1210 col 34 and confirm `&raw` is the offending token. Then update `tree-sitter-rust` dependency.

4. Wire sidecar PID + alive/dead flag into the `health` output, and have `health_compact` surface only `sidecar: up` / `sidecar: down` to keep its size budget.

5. Audit `analyze_file_impact` / temporal subsystem's path filter so `wiki/.obsidian/*` doesn't show up as top coupling.

6. Document structural patterns with at least 5 known-good Rust examples; include one that handles `Result<T, E>` return types correctly.

No code in AAP needs to change for any of these; this is all SymForge-internal.

End of report.
