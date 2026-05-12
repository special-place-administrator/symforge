# SymForge MCP Evaluation Report - 2026-05-11

Audience: coding agent assigned to repair SymForge MCP regressions.

Evaluator context:

- Repository under test: `C:\AI_STUFF\PROGRAMMING\Agent_Army_Professionals`
- Current branch: `main`
- Current dirty worktree before this report: untracked `.github/`, `.specify/`, and `specs/` content from Speckit.
- SymForge mode under test: MCP tools exposed in this Codex session.
- Scope: black-box MCP behavior against a real repository, cross-checked with `rg`, `git`, and `cargo` where useful.
- Source edits during evaluation: none. Only dry-run edit tools were invoked. This file is the first write.

## Executive Summary

SymForge is usable for quick orientation, literal search, exact file/range reads, symbol lookup, and dry-run edit previews. Those paths were fast and mostly agreed with shell cross-checks.

The risky regressions are in higher-level graph and context features:

1. Symbol context and file context can hang or ignore practical token limits on large files.
2. File dependency/reference output has severe false positives caused by common symbol names like `new()`.
3. `find_references` misses fully-qualified Rust uses that `search_text` and `batch_rename --dry-run` can find.
4. Rust parse diagnostics can mark compiler-valid files as partial with misleading line-1 diagnostics.
5. Health output is internally inconsistent and the watcher changed from active to off during the session.
6. Untracked-file discovery is inconsistent unless the exact path is explicitly read or reindexed.
7. Usage filtering leaks documentation/comment matches.

For coding-agent safety, treat `search_text`, `get_file_content`, `search_symbols`, and dry-run rename/edit as the more trustworthy paths. Treat `find_dependents`, `get_symbol_context` on large files, and `get_file_context` `Used by`/`Key references` sections as suspect until fixed.

## Baseline State

### Current health snapshot

Repro:

```text
mcp__symforge__.health_compact({})
```

Observed:

```text
Status: Ready | Files: 1136 indexed (1123 parsed, 13 partial, 0 failed) | Symbols: 33075 | Loaded: 392ms
Watcher: off | Admission tiers: 1136/0/0 (indexed/metadata/skipped)
Parse issues: 13 partial, 0 failed; use full health for path lists
Git temporal: ready (500 commits over 90d, computed in 305ms) | Worktree misuse/hour: 0
```

Earlier in the same evaluation session, before heavier probes and a `cargo check`, `health_compact` reported:

```text
Status: Ready | Files: 1135 indexed (1122 parsed, 13 partial, 0 failed) | Symbols: 33050 | Loaded: 397ms
Watcher: active (events: 0, overflows: 0, repairs: 20)
```

The count changed to 1136 after manually disk-refreshing/reindexing one untracked markdown file with `get_file_context` and `analyze_file_impact(new_file=true)`.

### Dirty worktree baseline

Repro:

```powershell
git status --short --branch
```

Observed:

```text
## main...origin/main
?? .github/agents/
?? .github/copilot-instructions.md
?? .github/prompts/
?? .specify/
?? specs/
```

SymForge `what_changed(uncommitted=true)` saw these same untracked file groups, so git path-diff integration is working for dirty state.

## Findings

### SF-001 - `get_symbol_context` times out on a specific large-file symbol

Severity: P1

Risk: high for coding agents. This is a normal workflow for "understand this function before editing"; a 120s timeout breaks the flow and leaves the agent without an actionable fallback except manual range reads.

Repro:

```text
mcp__symforge__.get_symbol_context({
  "name": "handle_embedding_batch_completed",
  "path": "crates/aap-agents/src/actors/orchestrator.rs",
  "symbol_kind": "fn",
  "symbol_line": 5605,
  "verbosity": "signature",
  "sections": ["dependents", "siblings"],
  "max_tokens": 5000
})
```

Observed:

```text
tool call error: tool call failed for `symforge/get_symbol_context`

Caused by:
    timed out awaiting tools/call after 120s
```

Control proving the target exists and can be read cheaply:

```text
mcp__symforge__.get_file_content({
  "path": "crates/aap-agents/src/actors/orchestrator.rs",
  "start_line": 5605,
  "end_line": 5863,
  "show_line_numbers": true,
  "header": true
})
```

Observed control excerpt:

```rust
5605: fn handle_embedding_batch_completed(
5606:     state: &mut OrchestratorState,
5607:     task_id: TaskId,
5608:     batch_result: &crate::actors::embedder_worker::BatchResult,
5609: ) {
...
5842: fn handle_vector_upsert_failed(state: &mut OrchestratorState, task_id: TaskId, reason: &str) {
```

Impact:

- `get_file_content` succeeds instantly for the exact function range, so the timeout is not caused by file access.
- The likely expensive path is the trace/context expansion layer, probably dependents/siblings or dependency traversal on a huge file.
- The call used `verbosity="signature"` and `max_tokens=5000`; those controls did not prevent the expensive computation.

Likely fix direction:

- Enforce hard time/size budgets before expanding dependents and siblings.
- In trace mode, compute the requested symbol first and return partial context with an explicit "dependents omitted due budget/time" diagnostic.
- Add a regression test using a large file with many tests/siblings and a symbol near the lower half of the file.

Suggested acceptance test:

```text
Given a Rust file with >250 symbols and nested test module content,
when get_symbol_context is called for one function with verbosity=signature and max_tokens=5000,
then it returns within a bounded time and either includes compact context or explicitly reports omitted sections.
```

### SF-002 - `get_file_context` can take ~97s and over-return giant outlines

Severity: P1

Risk: high. `get_file_context` is the recommended "read this file first" path. On large files it should be a compact outline, not a near-hanging operation.

Repro:

```text
mcp__symforge__.get_file_context({
  "path": "crates/aap-agents/src/actors/orchestrator.rs",
  "sections": ["outline", "imports", "consumers"],
  "max_tokens": 6000
})
```

Observed:

- Wall time from tool wrapper: `97.1429 seconds`.
- Output included the full large file outline and a massive nested test-module outline.
- It reported:

```text
crates/aap-agents/src/actors/orchestrator.rs (269 symbols, Rust)
...
mod tests L8963-16074
  const TEST_USER_ID L8984-8984
  fn test_ports L8986-8991
  ...
  fn e5d_ema_blends_existing_value_on_subsequent_call L16033-16073
...
~158686 tokens saved vs raw file read
```

Impact:

- The practical response was not bounded enough for interactive MCP use.
- `max_tokens` did not suppress the enormous nested test outline.
- This makes the documented "prefer get_file_context before reading" rule risky on exactly the large files where token savings matter.

Likely fix direction:

- Respect `max_tokens` during outline rendering, not only after data gathering.
- Add per-section caps, especially for nested test modules.
- Consider default-collapsing test modules in production source outlines unless `include_tests` or a test section is explicitly requested.

Suggested acceptance test:

```text
Given a file with >200 symbols and a test module with >100 test functions,
when get_file_context(sections=["outline","imports","consumers"], max_tokens=6000) is called,
then the response is produced quickly and collapses excess symbols with a clear "N more omitted" marker.
```

### SF-003 - `find_dependents` produces severe false positives for file dependencies

Severity: P1

Risk: high. This tool claims to answer "what breaks if I change this file?" False positives at this scale make planning noisy and can send agents into unrelated code.

Target file:

```text
crates/aap-agents/src/adapters/store_knowledge_upsert.rs
```

Repro:

```text
mcp__symforge__.find_dependents({
  "path": "crates/aap-agents/src/adapters/store_knowledge_upsert.rs",
  "compact": true,
  "limit": 50,
  "max_per_file": 5,
  "max_tokens": 5000
})
```

Observed excerpt:

```text
File-level dependency graph: 25 files depend on crates/aap-agents/src/adapters/store_knowledge_upsert.rs
  crates/aap-agents/src/actors/interview_actor.rs  (150 refs: call)
  crates/aap-agents/src/actors/orchestrator.rs  (1612 refs: call)
  crates/aap-backend/src/main.rs  (122 refs: call)
  crates/aap-backend/tests/agent_flow.rs  (263 refs: call)
```

Cross-check:

```powershell
rg -n "store_knowledge_upsert|MemoryStoreKnowledgeUpsertAdapter|AsyncKnowledgeUpsert|KnowledgeUpsert" `
  crates/aap-agents/src/actors/interview_actor.rs `
  crates/aap-agents/src/actors/orchestrator.rs `
  crates/aap-backend/src/main.rs -S
```

Observed shell output for `interview_actor.rs`: no matches.

Relevant shell output for `orchestrator.rs` showed references to the trait/entry/receipt, not the adapter file:

```text
crates/aap-agents/src/actors/orchestrator.rs:52:use crate::ports::knowledge_upsert::{KnowledgeUpsertEntry, KnowledgeUpsertReceipt};
crates/aap-agents/src/actors/orchestrator.rs:301:    /// Production builds wire `MemoryStoreKnowledgeUpsertAdapter`. P0-01
crates/aap-agents/src/actors/orchestrator.rs:306:    pub knowledge_upsert: Option<Arc<dyn crate::ports::knowledge_upsert::AsyncKnowledgeUpsert>>,
```

Extra proof from `get_file_context` on `store_knowledge_upsert.rs`:

```text
Used by (25 files):
  crates/aap-agents/src/actors/orchestrator.rs (1612 refs)
  crates/aap-backend/tests/agent_flow.rs (263 refs)
  crates/aap-agents/tests/pipeline_flow.rs (162 refs)
  crates/aap-agents/src/actors/interview_actor.rs (150 refs)
...
Key references:
  new()
    crates/aap-agents/src/actors/interview_actor.rs line 108
    crates/aap-agents/src/actors/interview_actor.rs line 173
    crates/aap-agents/src/actors/interview_actor.rs line 174
```

The alleged `interview_actor.rs` references are ordinary calls/constructors unrelated to the adapter:

```rust
108:         let context_mgr = ConversationManager::new(
...
172:                     project_id,
173:                     turns: Vec::new(),
174:                     topics_covered: Vec::new(),
```

Impact:

- The dependency graph appears to conflate same-named symbols such as `new()` across unrelated types.
- This is not a mild precision issue. It reports dozens of unrelated files as dependents and very large bogus call counts.
- Coding agents should not use this output to choose blast radius or required tests until fixed.

Likely fix direction:

- File-level dependents should be based on module imports/path references, not generic call-name collisions.
- If symbol resolution is uncertain, label it as lexical/heuristic and separate it from true import/dependency edges.
- Common methods like `new`, `default`, `clone`, `from`, `into`, `handle`, `on_start`, and `build` need special treatment; they should not create cross-file dependency edges without resolved receiver/type evidence.

Suggested acceptance test:

```text
Given file A defines TypeA::new and unrelated files call TypeB::new, Vec::new, and TypeC::new,
when find_dependents(path=A) is called,
then unrelated constructor calls are not reported as file dependents.
```

### SF-004 - `find_references` misses fully-qualified Rust uses

Severity: P1

Risk: high for refactors and reviews. A reference finder that misses production uses can cause incomplete changes.

Target symbol:

```text
MemoryStoreKnowledgeUpsertAdapter
```

Definition:

```text
crates/aap-agents/src/adapters/store_knowledge_upsert.rs:24
```

Repro:

```text
mcp__symforge__.find_references({
  "name": "MemoryStoreKnowledgeUpsertAdapter",
  "path": "crates/aap-agents/src/adapters/store_knowledge_upsert.rs",
  "symbol_kind": "struct",
  "symbol_line": 24,
  "compact": true,
  "limit": 50,
  "max_per_file": 20,
  "max_tokens": 5000
})
```

Observed:

```text
7 references to "MemoryStoreKnowledgeUpsertAdapter" in 4 files
crates/aap-agents/src/adapters/mod.rs
crates/aap-agents/src/adapters/store_knowledge_upsert.rs
crates/aap-agents/tests/write_behind_knowledge_live.rs
crates/aap-backend/src/services/knowledge_memory.rs
```

Cross-check with `rg`:

```powershell
rg -n "MemoryStoreKnowledgeUpsertAdapter" crates -S
```

Observed shell output included additional real uses:

```text
crates\aap-backend\src\lib.rs:66:            aap_agents::adapters::store_knowledge_upsert::MemoryStoreKnowledgeUpsertAdapter::new(
crates\aap-backend\tests\common\mod.rs:73:        aap_agents::adapters::store_knowledge_upsert::MemoryStoreKnowledgeUpsertAdapter::new(
```

Control proving another SymForge path can find them:

```text
mcp__symforge__.search_text({
  "query": "MemoryStoreKnowledgeUpsertAdapter",
  "include_tests": true,
  "context": 0,
  "limit": 50,
  "max_per_file": 20,
  "max_tokens": 6000
})
```

Observed control excerpt:

```text
18 matches in 9 files
crates/aap-backend/src/lib.rs
> 66:             aap_agents::adapters::store_knowledge_upsert::MemoryStoreKnowledgeUpsertAdapter::new(
crates/aap-backend/tests/common/mod.rs
> 73:         aap_agents::adapters::store_knowledge_upsert::MemoryStoreKnowledgeUpsertAdapter::new(
```

Control proving edit tooling is more complete than `find_references`:

```text
mcp__symforge__.batch_rename({
  "path": "crates/aap-agents/src/adapters/store_knowledge_upsert.rs",
  "name": "MemoryStoreKnowledgeUpsertAdapter",
  "new_name": "MemoryStoreKnowledgeUpsertAdapterProbe",
  "symbol_line": 24,
  "kind": "struct",
  "dry_run": true,
  "code_only": true,
  "working_directory": "C:\\AI_STUFF\\PROGRAMMING\\Agent_Army_Professionals"
})
```

Observed:

```text
Confident matches (will be applied) - 13 site(s) across 6 file(s)
  crates/aap-agents/src/adapters/mod.rs
  crates/aap-agents/src/adapters/store_knowledge_upsert.rs
  crates/aap-agents/tests/write_behind_knowledge_live.rs
  crates/aap-backend/src/lib.rs
  crates/aap-backend/src/services/knowledge_memory.rs
  crates/aap-backend/tests/common/mod.rs
```

Impact:

- `find_references` likely catches imports and unqualified uses better than fully-qualified paths.
- Refactor agents should prefer `batch_rename --dry-run` or `search_text` to audit references until `find_references` is fixed.

Likely fix direction:

- Ensure reference indexing handles fully-qualified paths where the terminal identifier is the target type.
- Reuse the match collector used by `batch_rename`, or at minimum expose the same confident-match set in `find_references`.

Suggested acceptance test:

```text
Given TypeName is referenced as crate::module::TypeName::new(), imported TypeName, and re-exported TypeName,
when find_references(TypeName) is called,
then all three forms are returned or explicitly categorized by confidence.
```

### SF-005 - Rust parser diagnostics mark compiler-valid file as partial at line 1

Severity: P2

Risk: medium. Parse resilience is useful, but false "syntax error near line 1" diagnostics send agents to the wrong place.

Repro:

```text
mcp__symforge__.validate_file_syntax({
  "path": "crates/aap-code-intel/src/adapter.rs"
})
```

Observed:

```text
Syntax validation: crates/aap-code-intel/src/adapter.rs
Language: Rust
Status: partial
Diagnostic: tree-sitter: syntax error near `//! SymForge adapter -- wraps LiveIndex ` (line 1, column 1)
Byte span: 0..101894
Symbols extracted: 95
```

Cross-check file header:

```powershell
Get-Content crates\aap-code-intel\src\adapter.rs -TotalCount 5
```

Observed:

```rust
//! SymForge adapter -- wraps LiveIndex for in-process code intelligence.
//!
//! This is the single integration point between SymForge and AAP. When SymForge
//! updates break internal APIs, only this file needs to change.
```

Rust compiler cross-check:

```powershell
cargo check -p aap-code-intel --lib
```

Observed result:

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.39s
```

There were workspace warnings about non-root package profiles/patches, but no Rust syntax or type error in `aap-code-intel`.

Impact:

- Either tree-sitter Rust parsing has a false positive or SymForge is misreporting/mislocalizing the partial parse reason.
- The diagnostic points at valid crate-level inner doc comments, which is misleading.

Likely fix direction:

- Distinguish parser partial state from compiler syntax validity in wording.
- Improve diagnostic localization to the actual error node, not byte span 0 or first doc comment.
- Consider suppressing "syntax error near line 1" if symbol extraction succeeded and rustc accepts the file in tests.

Suggested acceptance test:

```text
Given a valid Rust file beginning with //! inner doc comments,
when validate_file_syntax is called,
then it must not report a syntax error at line 1.
```

### SF-006 - Health output reports inconsistent partial counts and watcher state

Severity: P2

Risk: medium. Health output is the first diagnostic agents use to decide whether to trust the index. Inconsistency reduces trust.

Repro:

```text
mcp__symforge__.health_compact({})
mcp__symforge__.health({})
```

Observed compact:

```text
Files: 1136 indexed (1123 parsed, 13 partial, 0 failed)
Watcher: off
Parse issues: 13 partial, 0 failed
```

Observed full health:

```text
Files:  1135 indexed (1122 parsed, 13 partial, 0 failed)
...
Partial parse files (10):
  1. crates/aap-agents/src/adapters/http_a2a_client.rs
  2. crates/aap-agents/src/eval/judge.rs
  3. crates/aap-browser/src/native/extraction.rs
  4. crates/aap-code-intel/src/adapter.rs
  5. crates/aap-db/src/stores/projects.rs
  6. crates/aap-prompts/tests/ui/raw_input_not_prompt_safe.rs
  7. crates/symforge/src/live_index/persist.rs
  8. crates/symforge/src/worktree.rs
  9. crates/symforge/vendor/tree-sitter-scss/src/parser.c
  10. crates/symforge/vendor/tree-sitter-scss/src/tree_sitter/alloc.h
```

Earlier in the same session, full health showed:

```text
Watcher: active (idle; debounce: 200ms, overflows: 0, reconcile repairs: 20, last reconcile: 8s ago)
```

Later health showed:

```text
Watcher: off
```

Impact:

- `13 partial` but only 10 listed means the health report is incomplete or stale.
- Watcher going from active to off without reason leaves agents unable to know whether live edits will be picked up.

Likely fix direction:

- Make compact and full health derive counts and lists from the same snapshot.
- If the partial list is capped at 10, say `Partial parse files (first 10 of 13)`.
- If watcher is disabled by config, say so. If it crashed or stopped, expose the reason and last error.

Suggested acceptance tests:

```text
Given N partial files and a display cap M,
health must report "first M of N" when N > M.
```

```text
Given watcher state changes from active to off,
health must include a reason: disabled-by-config, stopped, crashed, unsupported-platform, or unknown.
```

### SF-007 - Untracked-file discovery is inconsistent until exact-path read/reindex

Severity: P2

Risk: medium to high in agent workflows. Coding agents frequently create new files and then expect search to find them.

Initial dirty worktree proof:

```text
mcp__symforge__.what_changed({
  "uncommitted": true,
  "include_symbol_diff": false,
  "max_tokens": 2500
})
```

Observed:

```text
Scope: uncommitted working tree
Evidence: paths `.github/agents/speckit.analyze.agent.md`, `.github/agents/speckit.checklist.agent.md`, `.github/agents/speckit.clarify.agent.md`
...
.github/agents/speckit.analyze.agent.md
...
```

Initial search miss:

```text
mcp__symforge__.search_files({
  "query": "speckit.analyze",
  "limit": 20,
  "max_tokens": 2000
})
```

Observed:

```text
No indexed source files matching 'speckit.analyze'
```

Initial text search miss:

```text
mcp__symforge__.search_text({
  "query": "speckit.analyze",
  "path_prefix": ".github",
  "include_tests": true,
  "limit": 20,
  "max_tokens": 3000
})
```

Observed:

```text
No matches for 'speckit.analyze'
```

Exact-path read succeeded:

```text
mcp__symforge__.get_file_context({
  "path": ".github/agents/speckit.analyze.agent.md",
  "sections": ["outline"],
  "max_tokens": 2000
})
```

Observed:

```text
Source authority: disk-refreshed
-- .github/agents/speckit.analyze.agent.md (25 symbols, Markdown) --
section User Input L2-10
section Pre-Execution Checks L10-44
...
```

After exact-path read and explicit indexing:

```text
mcp__symforge__.analyze_file_impact({
  "path": ".github/agents/speckit.analyze.agent.md",
  "new_file": true,
  "include_co_changes": false
})
```

Observed:

```text
Language: Markdown
Symbols: 25 section
[Indexed, 0 callers yet]
```

Subsequent search succeeded:

```text
mcp__symforge__.search_files({
  "query": "speckit.analyze",
  "limit": 20,
  "max_tokens": 2000
})
```

Observed:

```text
1 matching file
  .github/agents/speckit.analyze.agent.md [0.40]
```

Impact:

- `what_changed` can see untracked files through git/disk, but search tools operate only on the current index.
- Exact path reads can disk-refresh a file, but broad discovery does not pull untracked paths in automatically.
- This is understandable architecturally, but dangerous without explicit diagnostics. The search miss did not say "untracked files are not indexed; run analyze_file_impact(new_file=true)".

Likely fix direction:

- When `what_changed` sees untracked files, search tools should either include them opportunistically or warn that untracked files may be absent from the index.
- Provide a repo-wide `refresh_untracked` or `index_dirty_files` helper, or make `what_changed(include_symbol_diff=true)` index metadata for new files.

Suggested acceptance test:

```text
Given an untracked file named foo.md exists in the repo,
when search_files(query="foo") is called before manual indexing,
then either foo.md is found or the response explicitly says untracked files are excluded and gives the exact indexing command.
```

### SF-008 - `search_text(group_by="usage")` still returns docs and comments

Severity: P3

Risk: low to medium. The tool is still useful, but the label is misleading for agents expecting code-only usage.

Repro:

```text
mcp__symforge__.search_text({
  "query": "MemoryStoreKnowledgeUpsertAdapter",
  "group_by": "usage",
  "include_tests": true,
  "limit": 50,
  "max_tokens": 5000
})
```

Observed excerpt:

```text
docs/audit/research/F2-memory-vector-pipeline.md
  section ... Reality check ...
    > 74: | Adapter | `crates/aap-agents/src/adapters/store_knowledge_upsert.rs` | `MemoryStoreKnowledgeUpsertAdapter` ...
crates/aap-agents/src/actors/orchestrator.rs
  in struct OrchestratorState ...
    > 301:     /// Production builds wire `MemoryStoreKnowledgeUpsertAdapter`. P0-01
crates/aap-memory/src/memory_store.rs
  24: /// outside this module (e.g. the agents-side `MemoryStoreKnowledgeUpsertAdapter`)
```

The response also said:

```text
(1 import/comment match(es) excluded by usage filter)
```

Impact:

- Some comments/imports were filtered, but not all comments/docs.
- Markdown documents were included even though `group_by="usage"` suggests code usage.

Likely fix direction:

- Define `usage` precisely.
- If `usage` means "non-import code usages only", exclude markdown and Rust doc comments.
- If docs are intentionally included, rename the mode or add a `code_usage` group.

Suggested acceptance test:

```text
Given a symbol appears in code, Rust doc comments, imports, and markdown docs,
when search_text(group_by="usage") is called,
then only executable/type-position code usages are returned, or docs/comments are separately labeled.
```

## Positive Findings

These paths behaved well enough to keep using during repairs.

### Literal search matches `rg` on known symbol text

Repro:

```text
mcp__symforge__.search_text({
  "query": "MemoryStoreKnowledgeUpsertAdapter",
  "include_tests": true,
  "context": 0,
  "limit": 50,
  "max_per_file": 20,
  "max_tokens": 6000
})
```

Observed:

```text
18 matches in 9 files
```

Shell cross-check:

```powershell
rg -n "MemoryStoreKnowledgeUpsertAdapter" crates docs -S
```

The file set and important lines agreed with `rg`, including fully-qualified backend uses.

### Exact range reads are fast and reliable

`get_file_content` returned [orchestrator.rs](../../crates/aap-agents/src/actors/orchestrator.rs) lines 5605-5863 immediately, with line numbers and no truncation issues.

### Basic symbol lookup is accurate

Repro:

```text
mcp__symforge__.search_symbols({
  "query": "MemoryStoreKnowledgeUpsertAdapter",
  "limit": 20,
  "max_tokens": 3000
})
```

Observed:

```text
3 matches in 1 files
24: struct MemoryStoreKnowledgeUpsertAdapter
29: impl MemoryStoreKnowledgeUpsertAdapter
46: impl AsyncKnowledgeUpsert for MemoryStoreKnowledgeUpsertAdapter
```

### `batch_rename --dry-run` found more complete references than `find_references`

This is important: edit tooling appears to have a better reference collection path than `find_references`. That collector may be reusable for repair.

Observed dry-run:

```text
Confident matches (will be applied) - 13 site(s) across 6 file(s)
```

### Structural search works for specific patterns

Initial broad pattern returned no matches inside the adapter file:

```text
query: "fn $NAME($$$) { $$$ }"
path_prefix: "crates/aap-agents/src/adapters/store_knowledge_upsert.rs"
```

But narrower structural patterns did work:

```text
query: "pub fn $NAME($$$) -> Self { $$$ }"
```

Observed:

```text
1 matches in 1 files
> 31: pub fn new(store: Arc<MemoryStore>, pool: Arc<SingleStorePool>) -> Self {  // $NAME=new
```

And repo-wide:

```text
query: "fn $NAME($$$) { $$$ }"
language: "Rust"
include_tests: true
```

Observed:

```text
20 matches in 6 files
```

Conclusion: structural search is not simply dead, but its matching behavior is narrow enough that docs/examples should be clearer.

## Recommended Repair Order

1. Fix dependency graph false positives (`find_dependents`, `get_file_context` `Used by`, and `Key references`).
   - This is the most dangerous correctness issue because it creates false blast-radius claims.

2. Fix `find_references` completeness for fully-qualified Rust paths.
   - Reuse or align with the dry-run rename collector if possible.

3. Add hard time/token budgets to `get_symbol_context` and `get_file_context`.
   - Return partial results with explicit omitted sections instead of timing out.

4. Repair health diagnostics.
   - Make counts, list caps, and watcher state reasons explicit.

5. Improve parser diagnostics for Rust partial files.
   - Especially files beginning with valid `//!` crate docs.

6. Add dirty-worktree/untracked-file guidance or indexing support.
   - Either include untracked files in search after `what_changed`, or produce a clear diagnostic.

7. Tighten `group_by="usage"` semantics.
   - Separate executable code usage, imports, comments, and docs.

## Minimal Regression Suite Proposal

Create a synthetic fixture repo or add focused integration tests covering these cases:

1. `constructor_name_collision_does_not_create_dependents`
   - File A defines `RealAdapter::new`.
   - Files B/C call `Vec::new`, `ConversationManager::new`, and unrelated `Thing::new`.
   - `find_dependents(A)` must not report B/C.

2. `fully_qualified_type_reference_is_found`
   - `crate::module::TypeName::new()` must be found by `find_references(TypeName)`.

3. `large_file_context_respects_budget`
   - A file with >250 symbols and >100 tests must return under a bounded time with collapsed output.

4. `symbol_context_large_file_no_timeout`
   - A function near the bottom of a large file must return partial context instead of timing out.

5. `rust_inner_doc_comment_not_parse_error`
   - A valid Rust file starting with `//!` must not get a line-1 syntax error.

6. `health_partial_count_list_cap_is_labeled`
   - If there are 13 partial files and only 10 displayed, output says "first 10 of 13".

7. `untracked_file_search_diagnostic`
   - Search behavior for untracked files is deterministic: found, or explicitly excluded with remediation.

8. `usage_filter_excludes_docs_or_labels_them`
   - A symbol in markdown, doc comments, imports, and code must be categorized predictably.

## Coding-Agent Operating Guidance Until Fixed

Use these SymForge paths confidently:

- `search_text` for literal symbol/string audits.
- `search_symbols` for definitions.
- `get_file_content` for exact ranges around narrowed targets.
- `batch_rename --dry-run` for reference audit before renames.
- `validate_file_syntax` as a hint, not as authoritative Rust syntax proof.

Avoid or cross-check these paths:

- Cross-check `find_dependents` with `search_text`/`rg` before trusting it.
- Cross-check `find_references` with `search_text` for fully-qualified Rust paths.
- Avoid `get_symbol_context` on huge files until timeout behavior is fixed.
- Treat `get_file_context` `Used by` and `Key references` as advisory, not proof.
- Check `health` watcher state before assuming the index is live.

## Evidence Files and Anchors Used

- `crates/aap-agents/src/adapters/store_knowledge_upsert.rs`
  - `MemoryStoreKnowledgeUpsertAdapter` at line 24
  - `impl MemoryStoreKnowledgeUpsertAdapter` at line 29
  - `impl AsyncKnowledgeUpsert for MemoryStoreKnowledgeUpsertAdapter` at line 46
- `crates/aap-agents/src/actors/orchestrator.rs`
  - `handle_embedding_batch_completed` at line 5605
  - `handle_vector_upsert_failed` at line 5842
  - doc comment mentioning `MemoryStoreKnowledgeUpsertAdapter` at line 301
- `crates/aap-agents/src/actors/interview_actor.rs`
  - unrelated `ConversationManager::new` at line 108
  - unrelated `Vec::new` calls at lines 173-174
- `crates/aap-backend/src/lib.rs`
  - fully-qualified `MemoryStoreKnowledgeUpsertAdapter::new` at line 66
- `crates/aap-backend/tests/common/mod.rs`
  - fully-qualified `MemoryStoreKnowledgeUpsertAdapter::new` at line 73
- `crates/aap-code-intel/src/adapter.rs`
  - valid crate-level `//!` docs at line 1

## Final Verdict

SymForge is already valuable as a fast code-intelligence sidecar, but the current MCP build is not reliable enough for autonomous refactor planning without cross-checks. The main issue is not basic indexing; it is overconfident high-level reasoning output. The repair focus should be precision, budget enforcement, and diagnostic honesty:

- no bogus dependency edges from common method names,
- no missing fully-qualified references,
- no 120s context calls,
- no misleading parser/health status,
- and clear behavior around untracked files.
