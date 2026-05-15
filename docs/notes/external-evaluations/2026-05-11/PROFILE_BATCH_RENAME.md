# PROFILE_BATCH_RENAME  H.7 investigation (2026-05-15)

## Reproduction

Primary evaluator input:

```json
{
  "path": "src/daemon.rs",
  "name": "health",
  "new_name": "get_health",
  "dry_run": true
}
```

Worktree:

- `C:\Users\rakovnik\.config\superpowers\worktrees\symforge\codex4-h7-batch-rename-profile`
- branch `codex4/h7-batch-rename-profile`
- `HEAD = 766c72c`
- Windows 11 / PowerShell / Rust `1.94.0-x86_64-pc-windows-msvc`

Measured runs:

| Path | Command / invocation | Wall time | Result |
|---|---:|---:|---|
| Installed live MCP daemon | `mcp__symforge__.batch_rename({ path: "src/daemon.rs", name: "health", new_name: "get_health", dry_run: true, working_directory: <worktree> })` | `120.0176s` | MCP client timeout |
| Installed live MCP after local fallback | same invocation | `120.0148s` | MCP client timeout |
| Installed live MCP with `code_only=true` | same invocation plus `code_only: true` | `0.0680s` | success, 14 confident sites across 6 files |
| Current `main @ 766c72c`, scratch integration dispatch | `RUST_LOG=symforge=trace cargo test --test batch_rename_perf -- --test-threads=1 --nocapture` | `4.60s` test wall; dispatch `53.6788ms` | success |
| Current `main @ 766c72c`, real MCP stdio, fresh index | `target\debug\symforge.exe` JSON-RPC `index_folder` then `tools/call batch_rename` | index `4.684s`; rename `0.063s` | success, 20 confident sites across 8 files |
| Current `main @ 766c72c`, real MCP stdio after watcher admitted `target/` generated files | same, after touching 484 generated target files; health showed 907 indexed files / 25496 symbols | index `4.517s`; rename `0.067s` | success |

Interpretation: the evaluator timeout is reproducible in the installed/live daemon path, but is not reproducible from freshly built current `main @ 766c72c`. The current-source profile does not show a remaining >60s bottleneck for the primary repro.

## Capture methodology

Used hybrid capture:

- `trace`: required cargo trace run was captured to:
  - `C:\Users\rakovnik\AppData\Local\Temp\symforge-h7-batch-rename-trace-20260515-085029.stdout.log`
  - `C:\Users\rakovnik\AppData\Local\Temp\symforge-h7-batch-rename-trace-20260515-085029.stderr.log`
- `manual-instant`: the scratch integration wrapper timed `LiveIndex::load` and the `dispatch_tool_for_tests("batch_rename", ...)` segment.
- `manual-instant`: a PowerShell JSON-RPC harness drove freshly built `target\debug\symforge.exe` over real MCP stdio.
- `backtrace+thread-dump` was attempted against the live daemon PID `18380`, but `rust-lldb.exe` is unavailable for the active `1.94.0-x86_64-pc-windows-msvc` toolchain, so no usable stack dump was produced.
- `flamegraph` was rejected because `cargo flamegraph` / `cargo-flamegraph` is not installed on this machine.
- `tokio-console` was rejected because this is a blocking CPU/runtime investigation and the current-source path completed in milliseconds.

The scratch test file was `tests/batch_rename_perf.rs`; it was used only for profiling and is not part of the intended commit.

## Findings  root cause

Root cause class: **unbounded-traversal**, specifically the evaluator-era supplemental qualified-usage scan in `batch_rename(dry_run=true)`.

Evidence:

- `src/protocol/edit.rs:1567-1605` snapshots every indexed file's content for Phase 2b and passes it to `qualified_usages::collect_qualified_usages`.
- Current `src/live_index/qualified_usages.rs:39-66` has a cheap byte prefilter:
  - `src/live_index/qualified_usages.rs:45-47` skips files that do not contain `identifier` adjacent to `::`.
  - Only then does `src/live_index/qualified_usages.rs:48-52` UTF-8 decode and call `find_qualified_usages`.
- `git blame -L 39,66 -- src/live_index/qualified_usages.rs` shows this prefilter landed in `42c8e16` on `2026-05-14`.
- The pre-`42c8e16` implementation in `42c8e16^:src/protocol/edit.rs` ran `find_qualified_usages(&input.name, source)` for every UTF-8 indexed file after cloning all file contents, with no adjacency prefilter.
- Runtime evidence matches that failure mode:
  - Installed/live daemon, no `code_only`: `120.0176s` / `120.0148s` timeout.
  - Installed/live daemon, `code_only=true`: `0.0680s`.
  - Current HEAD with the prefilter: `0.0537s` dispatch and `0.063s` real MCP stdio rename.

The current `main @ 766c72c` source therefore appears to already contain the bottleneck fix for the evaluator-era timeout. The remaining live timeout is consistent with an installed/shared daemon binary that predates the `42c8e16` prefilter, not with a remaining current-HEAD code path.

## Findings  contributing factors (if any)

- The installed shared daemon executable is `C:\Users\rakovnik\.symforge\bin\symforge.exe`, `symforge 7.6.2`, last modified `2026-05-06 08:13:13`, while the relevant source prefilter landed on `2026-05-14`.
- The live daemon process serving the timed-out calls was PID `18380`, listening on `127.0.0.1:52636`, with project `symforge` rooted at `//?/E:/project/symforge`.
- After the MCP proxy degraded to local fallback, local health showed `target/` generated files in the index (`1072 indexed`, including `target/debug/...` entries). This is a separate watcher/admission hygiene issue, but freshly built current HEAD still completed the primary dry-run in `0.067s` even after a scratch MCP server admitted 907 files including generated `target/` content.
- `code_only=true` avoids much of the non-source scan surface and returned quickly in the live path, but the evaluator input does not set `code_only`, so the default path must remain bounded.

## Proposed patch scope

Recommended patch-phase scope:

- No production source patch should be authorized yet for `main @ 766c72c` unless the orchestrator can reproduce the timeout with a freshly built current binary.
- Authorize a regression/perf guard only:
  - Add `tests/batch_rename_perf.rs` or equivalent integration coverage for the primary repro.
  - Assert current-source dry-run wall time is below 5s after `LiveIndex::load` / MCP `index_folder`.
  - Estimated LOC: 80-140 test LOC.

If a source hardening patch is still desired after a rebuilt-current repro:

- Minimal candidate files:
  - `src/live_index/qualified_usages.rs` (keep/strengthen byte prefilter and add targeted tests): estimated 10-30 LOC.
  - `src/protocol/edit.rs` (avoid cloning/scanning unneeded file content for dry-run): estimated 20-50 LOC.
  - Optional separate task, not H.7 patch: `src/watcher/mod.rs` or admission code to prevent gitignored `target/` watcher events from entering the live index.
- Risks:
  - Over-filtering qualified usages can miss `Type::method` / import-path rename sites.
  - Tightening `code_only` defaults would change documented default semantics and should not be bundled.
  - Watcher admission changes are broader than the batch_rename timeout and should be carved out unless they reproduce the timeout on current HEAD.
- Regression-test sketch:
  - Build a fixture or use the repo primary repro with `src/daemon.rs`, `health -> get_health`, `dry_run=true`.
  - Include docs/non-code files containing `::health` so non-code semantics stay covered.
  - Include many non-matching generated/ignored files to prove the prefilter keeps dry-run under 5s.
  - Assert output still includes the known confident docs/source matches when `code_only` is omitted.

## Stop-condition check

Is bottleneck in a third-party crate (libgit2 etc.)?

No. The bottleneck is not git-temporal or libgit2. Current-source runs show indexing/git-temporal are not on the hot path for the dry-run:

- Fresh MCP stdio: `index_folder` `4.684s`, `batch_rename` `0.063s`.
- Scratch trace: `LiveIndex::load` `4.5106546s`, `batch_rename` dispatch `53.6788ms`.

Recommendation:

- Proceed only to a narrowed patch phase: commit this profile report, then add a regression/perf test and refresh/restart the installed SymForge daemon binary before asking for source changes.
- Do not authorize a production source patch for H.7 until the timeout is reproduced against a freshly built `main @ 766c72c` binary.
