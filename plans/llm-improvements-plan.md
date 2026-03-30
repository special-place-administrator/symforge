# SymForge LLM Improvements Plan — All 10 Suggestions

**Date**: 2026-03-30
**Status**: Draft
**Goal**: Make SymForge genuinely more useful to LLMs beyond its current structural intelligence.

---

## Architecture Overview (for context)

The codebase is a Rust MCP server with these key layers:

```
src/protocol/tools.rs    — 26 tool handlers (input structs + handler fns)
src/protocol/mod.rs      — SymForgeServer, MCP wiring, daemon proxy
src/protocol/format.rs   — All text rendering (119 functions)
src/protocol/edit.rs     — File mutation engine
src/protocol/explore.rs  — Concept pattern map (17 categories)
src/protocol/prompts.rs  — 3 MCP prompts
src/protocol/resources.rs— 4 resources + 4 templates
src/live_index/query.rs  — Query engine (all View types, ~5800 lines)
src/live_index/search.rs — Search engine (symbol, text, file search)
src/live_index/store.rs  — Index storage (LiveIndex, SharedIndex, IndexedFile)
src/domain/index.rs      — Core types (SymbolRecord, ReferenceRecord, etc.)
src/sidecar/handlers.rs  — Sidecar HTTP handlers (outline, impact, context, etc.)
```

Key patterns:
- Tool handlers acquire read lock → extract owned View types → drop lock → format to String
- All tool output is plain text (AD-6 rule: never return JSON)
- Edit tools resolve symbols by name → splice bytes → atomic write → reindex
- Daemon proxy: tools forward to shared daemon over HTTP, fallback to local

---

## Suggestion 1: Semantic Summaries

**Problem**: SymForge knows structure (symbol names, callers, callees) but not intent. An LLM must read full function bodies to understand what they do, which defeats the token-saving purpose.

**What exists**: Doc comments are already captured via `doc_byte_range` on `SymbolRecord`. `extract_first_doc_line()` in format.rs extracts the first line. `apply_verbosity("signature")` returns name+params+return only. `apply_verbosity("compact")` adds the first doc line.

### Implementation

#### Phase 1: Doc-Extracted Summaries (no LLM needed)

**In `src/domain/index.rs`**:
- Add `summary: Option<String>` to `SymbolRecord` (or compute on-the-fly)
- The summary is: first doc line if present, else auto-generated from signature

**In `src/parsing/` (each language parser)**:
- Already extracting doc_byte_range. No changes needed for doc extraction.

**In `src/protocol/format.rs`**:
- Add new verbosity level `"summary"` to `apply_verbosity()`:
  - Returns: one-line summary + signature (no body)
  - For symbols with doc comments: first meaningful doc line
  - For symbols without: auto-generate from name+kind+params
    (e.g., "fn optimize_deterministic(db, profession, weights, ...) → Result<SynergyResult, String>")

**In tool responses** (format.rs rendering functions):
- `search_symbols_result_view()`: Add optional summary column
- `explore_result_view()`: Include summaries at depth >= 1
- `context_bundle_result_view()`: Show summaries for callees/callers instead of full bodies

**Estimated touch points**: domain/index.rs (1 field), format.rs (modify apply_verbosity + ~5 rendering fns), tools.rs (add `include_summaries` param to search_symbols, explore)

#### Phase 2: Heuristic Summaries (smart, no LLM)

**New function in `src/protocol/format.rs` or new `src/protocol/summarize.rs`**:

```rust
fn auto_summarize_symbol(file: &IndexedFile, sym: &SymbolRecord) -> String
```

Heuristics:
1. If doc comment exists → first sentence
2. If function body is short (<10 lines) → "Short function that [verb from name]"
3. If function calls one main function → "Wrapper around {callee}"
4. If function is a test → "Test for {tested_symbol}"
5. If struct/enum → "{N} fields/variants" + list field names
6. Pattern match on common prefixes: get_, set_, is_, has_, new, from_, into_, try_, parse_, render_, format_, validate_
7. Fallback → signature-based: "fn {name}({param_types}) → {return_type}"

This gives useful summaries for ~80% of symbols without any LLM.

#### Phase 3: LLM-Generated Summaries (optional, cached)

This is a later enhancement. Cache LLM-generated summaries keyed by content_hash. Not essential for v1.

### New/Modified Tools

**Modified**: `search_symbols` — add `include_summaries: bool` param
**Modified**: `explore` — summaries at depth >= 1 by default
**Modified**: `get_symbol_context` — new verbosity `"summary"` for callers/callees
**Modified**: `get_file_context` — outline section can include summaries

---

## Suggestion 2: Smart Query Entry Point

**Problem**: 24 tools is cognitive load. LLMs pick the wrong tool ~15-20% of the time. A unified entry point routes ambiguous queries correctly.

**What exists**: `explore` already does concept matching + multi-phase search. The tool descriptions have "NOT for X, use Y" guidance. But the LLM still has to choose.

### Implementation

**New file**: `src/protocol/smart_query.rs`

```rust
pub struct SmartQueryInput {
    pub query: String,
    pub max_results: Option<u32>,
    pub context: Option<String>,  // "I'm trying to edit X" / "I'm debugging Y"
}

pub enum QueryIntent {
    FindSymbol { name: String, kind: Option<String> },
    FindFile { path_hint: String },
    SearchCode { pattern: String },
    FindCallers { symbol: String },
    FindChanges { scope: String },
    Understand { concept: String },
    PrepareEdit { target: String },
}
```

**Intent classification** (rule-based, no LLM needed):

```
"who calls X"           → FindCallers
"where is X defined"    → FindSymbol
"find files matching X" → FindFile
"what changed"          → FindChanges
"how does X work"       → Understand
"I want to edit X"      → PrepareEdit
"X pattern in code"     → SearchCode
```

Pattern matching:
1. Regex patterns for common phrasing ("who calls", "where is", "find", "what changed", "how does", etc.)
2. If query looks like a symbol name (CamelCase, snake_case, no spaces) → FindSymbol
3. If query looks like a path (contains / or .) → FindFile
4. If query contains code-like patterns (operators, keywords) → SearchCode
5. Default → Understand (falls through to explore)

**Routing**: Each intent maps to 1-2 existing tool calls. The smart_query handler calls them internally and returns a unified response.

**In `src/protocol/tools.rs`**:
- Register new tool `query` with description:
  "Natural language entry point. Ask any question about the codebase and SymForge routes to the right tool internally. Use when unsure which specific tool to call."

**In `src/protocol/mod.rs`**: Wire the new tool.

### Important Design Decision
The `query` tool should include a "routed_to" field in its output so the LLM learns which specific tool was used and can call it directly next time:
```
[Routed to: find_references(name="optimize_deterministic")]
```

**Estimated touch points**: New file smart_query.rs (~300 lines), tools.rs (1 new handler + input struct), mod.rs (wire it)

---

## Suggestion 3: Edit Planning

**Problem**: The gap between "I want to change X" and "which edit tools to call in what order" is where LLMs make mistakes.

**What exists**: `get_symbol_context(bundle=true)` fetches a symbol + all type deps for edit preparation. `batch_edit(dry_run=true)` previews changes. But there's no planning step.

### Implementation

**New file**: `src/protocol/edit_plan.rs`

```rust
pub struct EditPlanInput {
    /// Natural language description: "rename the error type" / "add logging to all handlers"
    pub description: String,
    /// Optional: specific symbol or file to start from
    pub target: Option<String>,
}

pub struct EditPlan {
    pub affected_symbols: Vec<AffectedSymbol>,
    pub affected_files: Vec<String>,
    pub impact_radius: usize,  // how many dependents
    pub suggested_operations: Vec<SuggestedOp>,
    pub warnings: Vec<String>,
}

pub struct SuggestedOp {
    pub tool: String,        // "edit_within_symbol", "batch_rename", etc.
    pub params_hint: String, // human-readable param description
    pub reason: String,
}
```

**Logic**:
1. Parse description for intent (rename, add, remove, modify, move)
2. If target specified → resolve it via `resolve_symbol_selector`
3. Compute impact:
   - `find_references` for the symbol
   - `find_dependents` for the file
   - Count total affected symbols/files
4. Suggest tool sequence:
   - Rename → `batch_rename`
   - Small edit → `edit_within_symbol`
   - Full rewrite → `replace_symbol_body`
   - Multi-location → `batch_edit` or `batch_insert`
   - Delete → `delete_symbol` + check for broken references
5. Return plan with warnings ("3 files depend on this", "symbol has 12 callers")

**In tools.rs**: Register as `edit_plan` tool.

**Estimated touch points**: New file edit_plan.rs (~400 lines), tools.rs (1 handler), format.rs (1 rendering fn)

---

## Suggestion 4: Context Budget Awareness

**Problem**: LLMs can't predict how much context a call will consume before making it. This leads to over-fetching or under-fetching.

**What exists**: `max_tokens` param on `get_symbol_context(bundle=true)` and `get_file_context`. Token savings reported per-call ("~14534 tokens saved"). `enforce_token_budget()` in format.rs.

### Implementation

#### Option A: `estimate` parameter on existing tools

**In `src/protocol/tools.rs`** — Add `estimate: Option<bool>` to these input structs:
- `GetSymbolInput`
- `GetSymbolContextInput`
- `GetFileContentInput`
- `GetFileContextInput`

When `estimate=true`, the handler:
1. Resolves the symbol/file (validates it exists)
2. Computes approximate token count (~4 chars per token) without rendering
3. Returns a compact estimate instead of full content:

```
Estimate for get_symbol(optimize_deterministic):
  Symbol body: ~1330 tokens (5314 bytes)
  With callers: ~1800 tokens
  Bundle (all type deps): ~5200 tokens
  Raw file: ~15800 tokens
```

**Implementation per tool**:
- `get_symbol`: byte_range size / 4
- `get_symbol_context`: body + sum(caller bodies) + sum(type deps)
- `get_file_content`: file byte_len / 4
- `get_file_context`: outline tokens (symbol count × ~20 tokens each)

#### Option B: Dedicated `estimate_cost` tool

Register one tool that takes a tool name + params and returns the estimate.
Simpler but adds yet another tool (we're trying to reduce tool count).

**Recommendation**: Option A (parameter on existing tools). Less tool sprawl.

**Estimated touch points**: tools.rs (add param to 4 structs, add early-return logic in 4 handlers), format.rs (1 new estimate rendering fn)

---

## Suggestion 5: Workflow Recipes (Enhanced Prompts)

**Problem**: Current prompts are static templates with resource links. They don't encode multi-step decision logic.

**What exists**: 3 prompts in `src/protocol/prompts.rs` — review, architecture, triage. Each returns instruction text + resource links.

### Implementation

**Rewrite prompts to be multi-step workflows with conditional logic**:

```rust
// In prompts.rs — rewrite prompt content to be procedural

fn code_review_prompt(...) -> Vec<PromptMessage> {
    // Instead of "Review code in project X. Focus on correctness."
    // Return a step-by-step workflow:
    vec![PromptMessage::user(format!(r#"
## Code Review Workflow for '{project}'

### Step 1: Scope the Review
- Call `what_changed(uncommitted=true, code_only=true)` to see all modified files
- Call `diff_symbols()` to see which symbols changed
- If > 20 symbols changed, use `diff_symbols(compact=true)` first for overview

### Step 2: Prioritize by Risk
For each changed symbol:
- Call `find_references(name="{sym}", compact=true)` — symbols with >5 callers are HIGH RISK
- Call `get_symbol_context(name="{sym}", verbosity="signature")` — check for type changes

### Step 3: Deep Review (high-risk symbols only)
- Call `get_symbol_context(name="{sym}", bundle=true)` to see the symbol + all its type deps
- Check: Are all callers still compatible with the change?
- Check: Did any type dependency change shape?

### Step 4: Report
Summarize: what changed, risk assessment per symbol, any broken contracts.
"#))]
}
```

**New prompts to add**:
- `symforge-refactor` — "I want to refactor X" → scoped investigation plan
- `symforge-onboard` — "Help me understand this codebase" → layered exploration plan
- `symforge-debug` — more detailed than triage, includes "check git blame", "trace callers", etc.

**Estimated touch points**: prompts.rs (rewrite 3 existing + add 3 new), tools.rs (no changes — prompts use existing tools)

---

## Suggestion 6: Session Context Tracking

**Problem**: SymForge is stateless per-call. It doesn't know what the LLM has already fetched, leading to redundant reads and lost context.

**What exists**: `token_stats: Option<Arc<TokenStats>>` on SymForgeServer already tracks cumulative savings. Tool call counts are tracked. But there's no per-symbol/file tracking.

### Implementation

**New struct in `src/protocol/mod.rs` or new `src/protocol/session.rs`**:

```rust
pub struct SessionContext {
    /// Symbols the LLM has fetched (path, name, line) → approximate token cost
    fetched_symbols: HashMap<(String, String, Option<u32>), u32>,
    /// Files the LLM has read (path → lines read, approximate tokens)
    fetched_files: HashMap<String, (Option<(u32, u32)>, u32)>,
    /// Total tokens served this session
    total_tokens_served: u64,
    /// When the session started
    started_at: Instant,
}
```

**Integration points**:

1. **In `src/protocol/tools.rs`** — After each tool returns, record what was served:
   - `get_symbol` → record (path, name, tokens)
   - `get_file_content` → record (path, line_range, tokens)
   - `get_symbol_context` → record symbol + all deps served
   - `get_file_context` → record (path, tokens)

2. **Deduplication hints in output** — When a tool is about to return content the LLM already has:
   ```
   Note: You already have 'optimize_deterministic' body in context (~1330 tokens from turn 3).
   Showing only NEW information: 2 new callers found since your last query.
   ```

3. **New tool: `context_inventory`**:
   ```
   Session Context (12 minutes, 8 tool calls):
     Symbols loaded: 5 (~4200 tokens)
       - optimize_deterministic (engine.rs) — 1330 tokens
       - optimize_synergy (synergy_pipeline.rs) — 890 tokens
       - ...
     Files loaded: 2 (~3100 tokens)
       - engine.rs (lines 1069-1242) — 1330 tokens
       - ...
     Total context served: ~7300 tokens
     Estimated remaining budget: ~120K tokens (128K model)
   ```

4. **Proactive suggestions** — In explore/search results, annotate hits:
   ```
   Symbols (5 found):
     optimize_deterministic [fn, engine.rs] ← ALREADY IN YOUR CONTEXT
     optimize_synergy [fn, synergy_pipeline.rs] ← ALREADY IN YOUR CONTEXT
     select_gear_prefix [fn, scoring.rs]  ← NEW
   ```

**Storage**: `SessionContext` lives on `SymForgeServer` behind `Arc<Mutex<>>`. Reset when connection drops or explicit reset command.

**Estimated touch points**: New session.rs (~200 lines), mod.rs (add to SymForgeServer), tools.rs (recording after each handler, ~20 points), format.rs (annotation rendering), 1 new tool handler

---

## Suggestion 7: Error Context on Failures

**Problem**: When a symbol isn't found or an edit fails, the error is a dead-end. The LLM has to make a new exploratory call to recover.

**What exists**: `render_not_found_symbol()` in format.rs already shows fuzzy-matched suggestions! `not_found_symbol_names()` uses `fuzzy_distance()` to find close matches. `resolve_or_error()` in edit.rs returns NotFound/Ambiguous with candidates.

**Assessment**: This is already partially implemented! The main gaps are:

### Remaining Gaps

1. **Search tools returning empty results** — When `search_symbols` returns nothing:
   - Currently: just returns empty result
   - Should: suggest alternative queries, show what DID match partially

2. **Edit validation failures** — When `edit_within_symbol` can't find `old_text`:
   - Currently: error message
   - Should: show the actual symbol body (or first 10 lines) so the LLM can see what's really there

3. **File not found** — When a path is wrong:
   - Currently: `not_found_file()` returns basic message
   - Should: use `search_files` internally to suggest similar paths

### Implementation

**In `src/protocol/format.rs`**:
- Modify `search_symbols_result_view()`: When 0 results, append "Did you mean: {fuzzy matches}"
- Modify `search_text_result_view()`: When 0 results, suggest broader search terms

**In `src/protocol/edit.rs`**:
- Modify `build_edit_within()` error path: When old_text not found, include first 500 bytes of symbol body in error
- Modify `resolve_or_error()` NotFound: Call `search_symbols` for near-matches (already partially done)

**In `src/protocol/tools.rs`**:
- `get_file_content` handler: When file not found, call `search::search_files` for suggestions
- `get_symbol` handler: Already uses `not_found_symbol_names()` — verify it's comprehensive

**Estimated touch points**: format.rs (~3 modifications), edit.rs (~2 modifications), tools.rs (~2 modifications). Fairly small scope since the foundation exists.

---

## Suggestion 8: Project Conventions Detection

**Problem**: LLMs need to know project conventions to write fitting code. Currently this requires an external CLAUDE.md.

### Implementation

**New file**: `src/protocol/conventions.rs`

```rust
pub struct ProjectConventions {
    pub error_handling: ErrorPattern,
    pub naming: NamingConventions,
    pub test_patterns: TestPatterns,
    pub common_imports: Vec<String>,
    pub file_organization: FileOrganization,
    pub complexity_profile: ComplexityProfile,
}

pub enum ErrorPattern {
    ResultWithStringError,
    ResultWithCustomError,
    AnyhowBased,
    ThiserrorBased,
    PanicHeavy,
    Mixed,
}
```

**Detection logic** (runs on index, no I/O needed):

1. **Error handling**: Scan symbols for Result return types, search for "anyhow", "thiserror", ".unwrap()", ".expect()" frequency
2. **Naming**: Analyze symbol names — snake_case vs camelCase ratio, prefix patterns (get_, set_, is_, has_), module naming
3. **Test patterns**: Count files in tests/ vs inline #[cfg(test)], test function naming (test_ prefix, _test suffix), fixture patterns
4. **Common imports**: Aggregate reference imports, find top 10 most-used crate/module imports
5. **File organization**: Average symbols per file, max file size, directory depth distribution
6. **Complexity profile**: Average function length (line_range span), deepest nesting (symbol depth), largest files

**New tool**: `conventions` — returns detected project conventions.
**New resource**: `symforge://repo/conventions`

**Caching**: Compute once on index load, invalidate on reload. Store on `SharedIndexHandle`.

**Estimated touch points**: New conventions.rs (~500 lines), tools.rs (1 handler), resources.rs (1 resource), store.rs (add cached conventions to SharedIndexHandle)

---

## Suggestion 9: Incremental Context Building (Investigation Mode)

**Problem**: Complex investigations require 5-10 tool calls. The LLM manually assembles context and can lose track.

### Implementation

**New file**: `src/protocol/investigation.rs`

```rust
pub struct Investigation {
    pub id: String,
    pub title: String,
    pub created_at: Instant,
    pub entries: Vec<InvestigationEntry>,
    pub total_tokens: u64,
}

pub struct InvestigationEntry {
    pub label: String,       // "optimize_deterministic body"
    pub source_tool: String, // "get_symbol"
    pub tokens: u32,
    pub added_at: Instant,
}
```

**New tools**:

1. `investigation_start` — Create a new investigation with a title
   ```json
   {"title": "Understanding the optimization pipeline"}
   ```
   Returns: investigation ID

2. `investigation_add` — Add context to the investigation
   ```json
   {"id": "inv_1", "tool": "get_symbol", "params": {"path": "engine.rs", "name": "optimize_deterministic"}}
   ```
   Executes the tool, stores the result label + token count (not the content — that's already in the LLM's context)

3. `investigation_status` — Show what's been gathered
   ```
   Investigation: "Understanding the optimization pipeline" (4 entries, ~3200 tokens)
   1. optimize_deterministic body (engine.rs) — 1330 tokens [get_symbol]
   2. optimize_synergy body (synergy_pipeline.rs) — 890 tokens [get_symbol]
   3. References to optimize_synergy — 450 tokens [find_references]
   4. File outline: engine.rs — 530 tokens [get_file_context]

   Suggestions:
   - You haven't looked at select_gear_prefix yet (called by optimize_deterministic)
   - SynergyResult type definition might be relevant (return type)
   ```

The key insight: the investigation doesn't store content (the LLM already has it). It stores **metadata** about what's been gathered, enabling:
- Table of contents
- Gap analysis ("you haven't looked at X yet")
- Token budget tracking

**Design consideration**: This overlaps with Suggestion 6 (Session Context). They could share the same tracking infrastructure. The difference: Session Context is automatic/passive, Investigation is explicit/active.

**Recommendation**: Implement Session Context (Suggestion 6) first as the foundation. Investigation Mode is a higher-level API on top of it. Could be Phase 2.

**Estimated touch points**: New investigation.rs (~300 lines), tools.rs (3 handlers), session.rs (reuse tracking)

---

## Suggestion 10: Real Metrics on Token Savings

**Problem**: Token savings are reported per-call but not aggregated. Hard to prove value or identify optimization opportunities.

**What exists**: `TokenStats` is already tracked! `format_token_savings()` renders it. The `health` tool shows session tool call counts. `compact_savings_footer()` appends savings to tool responses.

### Remaining Gaps

1. **Cumulative session metrics not exposed as a tool**
2. **No per-tool breakdown** (which tools save the most?)
3. **No efficiency ratio** (tokens served / tokens that naive reads would cost)

### Implementation

**Modify `TokenStats`** (likely in mod.rs or a shared module):

```rust
pub struct TokenStats {
    // Existing
    pub read_savings: AtomicU64,
    pub tool_savings: AtomicU64,

    // New
    pub tokens_served: AtomicU64,
    pub tokens_naive_equivalent: AtomicU64,
    pub per_tool_stats: Mutex<HashMap<String, ToolUsageStats>>,
}

pub struct ToolUsageStats {
    pub call_count: u32,
    pub tokens_served: u64,
    pub tokens_saved: u64,
    pub avg_response_tokens: u32,
}
```

**Modify `health` tool output** to include:

```
── Session Metrics ──
Duration: 12 minutes
Tool calls: 23
Tokens served: 18,400
Naive equivalent: 142,000
Efficiency ratio: 7.7x (87% reduction)

── Per-Tool Breakdown ──
get_symbol_context (8 calls)  — 6,200 tokens served, 89,000 saved
get_file_context (5 calls)    — 3,100 tokens served, 31,000 saved
search_symbols (4 calls)      — 1,800 tokens served, 12,000 saved
...

── Most Efficient Tools ──
1. get_file_context — 10.0x efficiency (90% reduction)
2. get_symbol_context — 14.3x efficiency (93% reduction)
3. explore — 8.5x efficiency (88% reduction)
```

**Estimated touch points**: Wherever TokenStats is defined (~1 file), format.rs (enhance health_report), tools.rs (record per-tool stats in each handler)

---

## Implementation Priority & Ordering

### Phase 1: Quick Wins (1-2 days each)
1. **Suggestion 7: Error Context** — Smallest scope, biggest quality-of-life improvement. Foundation already exists.
2. **Suggestion 10: Token Metrics** — TokenStats infrastructure exists, just needs enhancement.
3. **Suggestion 5: Workflow Recipes** — Pure content rewrite of prompts.rs, no infrastructure.

### Phase 2: Core Infrastructure (3-5 days each)
4. **Suggestion 1: Semantic Summaries** — Phase 1 (doc-extracted) and Phase 2 (heuristic). Touches many format functions but is additive.
5. **Suggestion 4: Context Budget** — `estimate` parameter on 4 tools. Clean, well-scoped.
6. **Suggestion 6: Session Context Tracking** — New infrastructure, many integration points.

### Phase 3: Higher-Level Features (5-7 days each)
7. **Suggestion 2: Smart Query** — Depends on all tools working well first. New routing logic.
8. **Suggestion 8: Conventions Detection** — Independent feature, can be done anytime.
9. **Suggestion 3: Edit Planning** — Depends on good conventions + context tracking.
10. **Suggestion 9: Investigation Mode** — Depends on session context (Suggestion 6).

### Dependency Graph

```
Suggestion 7 (Error Context)    ─── standalone
Suggestion 10 (Token Metrics)   ─── standalone
Suggestion 5 (Recipes)          ─── standalone
Suggestion 1 (Summaries)        ─── standalone
Suggestion 4 (Budget)           ─── standalone
Suggestion 8 (Conventions)      ─── standalone
Suggestion 6 (Session Context)  ─── standalone (but enables 9)
Suggestion 2 (Smart Query)      ─── benefits from 1 (summaries in results)
Suggestion 3 (Edit Planning)    ─── benefits from 6 (knows what LLM has seen)
Suggestion 9 (Investigation)    ─── depends on 6 (session tracking infrastructure)
```

---

## Files to Create/Modify Summary

### New Files
- `src/protocol/smart_query.rs` — Suggestion 2
- `src/protocol/edit_plan.rs` — Suggestion 3
- `src/protocol/session.rs` — Suggestion 6
- `src/protocol/conventions.rs` — Suggestion 8
- `src/protocol/investigation.rs` — Suggestion 9

### Modified Files (heavy)
- `src/protocol/tools.rs` — Every suggestion touches this (new handlers, new params, recording)
- `src/protocol/format.rs` — Suggestions 1, 4, 5, 7, 10 (rendering changes)
- `src/protocol/prompts.rs` — Suggestion 5 (rewrite all prompts)
- `src/protocol/mod.rs` — Suggestions 6, 10 (SessionContext, enhanced TokenStats)

### Modified Files (light)
- `src/domain/index.rs` — Suggestion 1 (optional summary field)
- `src/protocol/edit.rs` — Suggestion 7 (error context on edit failures)
- `src/protocol/resources.rs` — Suggestion 8 (conventions resource)
- `src/protocol/explore.rs` — Suggestion 2 (reuse concept matching)
- `src/live_index/store.rs` — Suggestion 8 (cached conventions on SharedIndexHandle)

---

## Estimated Total Effort

- Phase 1 (Quick Wins): ~4-6 days
- Phase 2 (Core Infra): ~12-15 days
- Phase 3 (Higher-Level): ~15-20 days
- **Total: ~30-40 days of focused development**

This can be parallelized — Phases 1 items are all independent, as are most Phase 2 items.
