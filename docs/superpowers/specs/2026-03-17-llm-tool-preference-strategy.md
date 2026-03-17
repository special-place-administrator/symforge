# LLM Tool Preference Strategy: Making SymForge the Path of Least Resistance

**Date**: 2026-03-17  
**Goal**: When users install SymForge MCP, every LLM client — Claude, Codex, Gemini, Kilo Code, Windsurf, Cursor, etc. — should **naturally prefer** SymForge tools over raw file reads, with zero ongoing configuration required after initial `SymForge init`.

---

## Executive Summary

SymForge already has strong foundations for tool preference steering. The current system uses 5 complementary vectors:

1. **Tool descriptions** with "NOT for X" anti-patterns and "Start here" directives
2. **Init guidance blocks** written to `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`
3. **Hook enrichment** that intercepts `Read`/`Grep`/`Edit`/`Glob` and suggests SymForge alternatives
4. **`alwaysAllow`** lists for frictionless tool invocation
5. **Token savings footers** appended to tool responses

However, there are significant gaps. This strategy identifies concrete improvements across all 5 vectors plus new vectors to establish SymForge as the **default code inspection tool** regardless of which LLM client is being used.

---

## Current State Audit

### What Works Well

| Vector | Current State | Effectiveness |
|--------|--------------|---------------|
| Tool descriptions | "NOT for X" cross-references guide LLMs away from wrong tools | High — LLMs read these carefully |
| `explore` tool | "Start here when you don't know where to look" — strong entry point | High |
| `get_repo_map` | "Start here" — good onboarding signal | High |
| Hook enrichment | Intercepts Read/Grep/Edit/Glob with alternative suggestions | Medium — Claude-only |
| Token savings footer | `~81977 tokens saved vs raw file read` on responses | Medium — reinforces value |
| `alwaysAllow` | 24 read-only tools auto-approved in Claude settings | High — eliminates friction |

### What Needs Improvement

| Gap | Impact | Priority |
|-----|--------|----------|
| Tool descriptions don't mention token savings or efficiency | LLMs don't know the cost benefit | **Critical** |
| Guidance blocks are too brief — 3 bullet points | Doesn't give enough behavioral rules | **Critical** |
| No Kilo Code init support | Kilo is a growing client, completely unsupported | **High** |
| Edit tools missing from `SYMFORGE_TOOL_NAMES` | `replace_symbol_body`, `edit_within_symbol`, etc. require manual approval | **High** |
| Hook enrichment is Claude-only | Codex, Gemini, Kilo get no interception | **Medium** |
| No "tool routing guide" in tool descriptions | LLMs don't know which tool to pick for what task | **Medium** |
| `get_file_content` description doesn't strongly discourage full-file reads | LLMs still default to reading entire files | **Medium** |
| Health tool doesn't suggest next actions | Missed opportunity for onboarding | **Low** |
| No guidance for Cursor, Windsurf, or other emerging clients | Market coverage gap | **Low** |

---

## Vector A: Tool Descriptions — MCP Protocol Level

### Current Pattern Analysis

Current descriptions follow a pattern of:
1. **What it does** — "Rich file summary: symbol outline, imports, consumers..."
2. **Usage hints** — "Use sections='outline' for..."
3. **Anti-patterns** — "NOT for X (use Y)"

This is good but missing two critical signals:

### Recommendation A1: Add Token Efficiency Signals to Key Tools

LLMs are increasingly cost-aware. When a description mentions concrete savings, the model factors this into tool selection.

**Current** (`get_file_context`):
```
"Rich file summary: symbol outline, imports, consumers, references, and git activity."
```

**Proposed**:
```
"Rich file summary: symbol outline, imports, consumers, references, and git activity. 
Typically 60-90% smaller than raw file content — saves thousands of tokens on large files. 
Use sections=['outline'] for symbol-only outline (names, kinds, line ranges)..."
```

**Apply to**: `get_file_context`, `get_symbol_context`, `get_repo_map`, `find_references`, `find_dependents`

### Recommendation A2: Add "Prefer Over" Phrases

Explicit "prefer this over X" signals are the strongest tool-selection nudge.

**Current** (`get_file_content`):
```
"Read raw file content. Only use when you need actual source text that other tools don't provide."
```

**Proposed**:
```
"Read raw file content. Modes: full file, line range, around_line/around_match/around_symbol, or chunked paging. 
IMPORTANT: For code understanding, prefer get_file_context (60-90% smaller) or get_symbol (targeted lookup) first. 
Only use this when you need actual source text that structured tools don't provide."
```

### Recommendation A3: Add a Decision-Tree Opening to `explore`

The `explore` tool should explicitly claim the "I don't know where to start" use case more aggressively.

**Proposed**:
```
"Start here when you don't know where to look. Accepts a natural-language concept and returns 
related symbols, patterns, and files — like a semantic search over the entire codebase. 
Much faster and more targeted than reading multiple files. Set depth=2 for signatures and 
dependents (~1500 tokens). Set depth=3 for implementations and type chains (~3000 tokens). 
NOT for finding a specific symbol by name (use search_symbols). NOT for text content search (use search_text)."
```

### Recommendation A4: Tool Names Are Already Optimal

Current tool names (`get_file_context`, `search_symbols`, `get_symbol`, `explore`) are clear and follow established naming conventions. No name changes recommended — renaming would break backward compatibility with existing guidance files and muscle memory.

### Recommendation A5: Add "Workflow" Hints to Search Tools

**Current** (`search_symbols`):
```
"Find symbols by name substring across the project — returns name, kind, file, line range."
```

**Proposed addition at the end**:
```
"...Typical workflow: search_symbols → get_symbol (for body) or get_symbol_context (for usage analysis)."
```

Similarly for `search_text`:
```
"...Typical workflow: search_text → inspect_match (for symbol context around a hit)."
```

---

## Vector B: Init Guidance Files

### Current State

The guidance blocks written to `~/.claude/CLAUDE.md`, `~/.codex/AGENTS.md`, and `~/.gemini/GEMINI.md` are minimal — just 3-4 bullet points:

```markdown
## SymForge MCP
- Prefer the SymForge MCP for codebase navigation when the `SymForge` server is connected.
- Start with `get_repo_map`, `get_repo_outline`, `get_file_context`, or `get_symbol_context` before broad raw file scans.
- Use `analyze_file_impact` after edits and `what_changed` when resuming work.
```

### Recommendation B1: Expand Guidance With Concrete Routing Rules

The guidance should be assertive but not annoying. The key insight: **LLMs follow explicit rules better than vague suggestions**. The current guidance is too vague to consistently change behavior.

**Proposed expanded guidance block** (shared across all clients, customized where needed):

```markdown
## SymForge MCP

When SymForge MCP is connected, use it as your PRIMARY tool for all code navigation and inspection.
SymForge tools are typically 60-90% more token-efficient than raw file reads.

### Decision rules:

1. **First contact with a project**: Call `get_repo_map` or `explore` — never start by reading random files.
2. **Understanding a file**: Use `get_file_context` instead of reading the whole file.
   It returns the symbol outline, imports, and key references in ~10% of the tokens.
3. **Reading a specific function/class**: Use `get_symbol` with the symbol name — 
   do NOT read the entire file to find one function.
4. **Finding code by name**: Use `search_symbols` — do NOT grep or read files to scan for names.
5. **Finding code by content**: Use `search_text` — it shows enclosing symbol context automatically.
6. **Understanding callers/dependencies**: Use `find_references` or `get_symbol_context`.
7. **After editing a file**: ALWAYS call `analyze_file_impact` to update the index.
8. **Resuming work**: Call `what_changed` to see uncommitted modifications.
9. **Code review**: Use `diff_symbols` to see which functions changed between refs.

### When raw file reads are still appropriate:
- Documentation files (README, docs/) where exact wording matters
- Configuration files (Cargo.toml, package.json) where raw content is the goal
- Non-source files that SymForge doesn't index

### Edit tools (when available):
- `replace_symbol_body` — replace an entire function/class by name (no need to read first)
- `edit_within_symbol` — find-and-replace scoped to a symbol's range
- `batch_edit` — multiple edits across files atomically
- `batch_rename` — rename a symbol and update all references
```

### Recommendation B2: Different Guidance Levels for Different Clients

Not all clients handle guidance the same way:

| Client | Guidance File | Notes |
|--------|--------------|-------|
| Claude Code | `~/.claude/CLAUDE.md` | Reads on session start. High compliance. Also reads project-level `CLAUDE.md`. |
| Codex | `~/.codex/AGENTS.md` | Reads `AGENTS.md`. Also reads `CLAUDE.md` in project root. |
| Gemini CLI | `~/.gemini/GEMINI.md` | Reads on session start. |
| Kilo Code | `~/.kilocode/rules/` or project `.kilocode/rules/` | Reads rule files. Custom instructions. |
| Cursor | `.cursor/rules` directory or `.cursorrules` file | Project-level rules. |
| Windsurf | `.windsurfrules` file | Project-level rules. |
| Cline/Roo | `.clinerules` file | Project-level rules. |

### Recommendation B3: Write Project-Level Guidance Automatically

In addition to global `~/.client/` guidance, `SymForge init` should detect the project root and write/update a **project-level guidance file** that works across clients.

**Strategy**: Write a `.symforge/TOOL_GUIDANCE.md` file and symlink or include it from client-specific rule files.

Better yet: write guidance into each client's project-level file format:
- `.claude/settings.local.json` or project `CLAUDE.md` 
- `.cursor/rules/*.mdc` files
- `.windsurfrules`
- `.clinerules`
- `.kilocode/rules/*.md`

### Recommendation B4: Assertive But Not Annoying

The guidance should be:
- **Assertive** for code navigation: "Use `get_file_context` instead of reading files" ✅
- **Suggestive** for optional features: "Consider using `diff_symbols` for code review" ✅
- **Never blocking**: Don't say "NEVER use read_file" — there are legitimate uses ✅
- **Include rationale**: "60-90% smaller" explains *why*, not just *what* ✅

---

## Vector C: Tool Response Design

### Current State

Tool responses already include:
- `~N tokens saved vs raw file read` footer on `get_file_context` responses
- Token savings summary in `health` tool output
- Tool call counts in `health` output

### Recommendation C1: Add Contextual "Next Step" Hints to Key Tools

When an LLM calls `get_file_context`, the response could include:

```
💡 To read a specific symbol's source, use: get_symbol(path="...", name="symbol_name")
💡 To see who calls these symbols, use: find_references(name="symbol_name")
```

**Implementation**: Add a `next_steps_footer` function in `format.rs` that appends 1-2 contextual hints based on the tool called and the result content.

**Apply selectively to**:
- `get_file_context` → suggest `get_symbol` for specific symbols
- `search_symbols` → suggest `get_symbol` or `get_symbol_context`
- `search_text` → suggest `inspect_match` for deeper context
- `health` → suggest `get_repo_map` to start exploring

**Caution**: Don't add hints to every response — it becomes noise. Only add when the hint is genuinely helpful and the next step isn't obvious.

### Recommendation C2: Error Messages Should Guide

When tools fail, the error message should redirect:

**Current**: `"File not found in index: src/foo.rs"`  
**Proposed**: `"File not found in index: src/foo.rs. Try search_files(query='foo') to find the correct path, or index_folder if the file was recently added."`

### Recommendation C3: Health Tool as Onboarding

The `health` tool should include a "Quick Start" section when it detects first use (no tool call counts yet):

```
── Quick Start ──
This is your first session. Recommended workflow:
1. get_repo_map — see project structure  
2. explore("your question") — find relevant code
3. get_file_context("path") — understand a specific file
4. get_symbol("path", "name") — read a specific function
```

---

## Vector D: MCP Config — alwaysAllow

### Current State

`SYMFORGE_TOOL_NAMES` in `init.rs` lists 24 tools for `allowedTools` in Claude settings. These are all read-only tools. Notably **missing** are the edit tools:
- `replace_symbol_body`
- `insert_symbol`
- `delete_symbol`
- `edit_within_symbol`
- `batch_edit`
- `batch_rename`
- `batch_insert`
- `explore` (present via backward compat but not in list)

### Recommendation D1: Add Edit Tools to alwaysAllow

Edit tools should be auto-approved because:
1. They're equivalent to the built-in `Edit` tool which is already auto-approved
2. Requiring approval friction makes LLMs prefer the built-in tool
3. They have safety guarantees (symbol resolution, auto-indentation) the built-in doesn't

**Add to `SYMFORGE_TOOL_NAMES`**:
```rust
"mcp__SYMFORGE__replace_symbol_body",
"mcp__SYMFORGE__insert_symbol",
"mcp__SYMFORGE__delete_symbol",
"mcp__SYMFORGE__edit_within_symbol",
"mcp__SYMFORGE__batch_edit",
"mcp__SYMFORGE__batch_rename",
"mcp__SYMFORGE__batch_insert",
"mcp__SYMFORGE__explore",
```

### Recommendation D2: Client-Specific alwaysAllow

Different clients handle auto-approval differently:
- **Claude Code**: `allowedTools` in `~/.claude/settings.json` ✅ already done
- **Codex**: `allow` list in `~/.codex/config.toml` ✅ already done  
- **Gemini CLI**: `trust: true` at server level ✅ already done
- **Kilo Code**: `alwaysAllow` in `mcp_settings.json` — **not yet implemented**
- **Cursor**: auto-approved if in MCP config — **needs investigation**

### Recommendation D3: Kilo Code Config

For Kilo Code, `SymForge init --client kilo` should write to the appropriate Kilo Code MCP settings location. Kilo Code reads MCP server config and can have tool-level `alwaysAllow`. The init command should:

1. Register the MCP server in Kilo's MCP settings
2. Set `alwaysAllow` for all SymForge read tools
3. Write guidance to Kilo's custom instructions location

---

## Vector E: Hook Enrichment

### Current State

The hook system is **Claude Code-only** and works through `claude_desktop_config.json` hooks:

| Event | What Happens |
|-------|-------------|
| `SessionStart` | Injects `get_repo_map` output as context |
| `PostToolUse: Read` | Injects `get_file_context` outline for the read file |
| `PostToolUse: Edit/Write` | Injects `analyze_file_impact` for the edited file |
| `PostToolUse: Grep` | Injects `get_symbol_context` for the search pattern |
| `PreToolUse: Read` | Suggests using `get_file_context` instead |
| `PreToolUse: Grep` | Suggests using `search_text` instead |
| `PreToolUse: Edit` | Suggests using `replace_symbol_body` instead |
| `PreToolUse: Glob` | Suggests using `search_files` instead |
| `UserPromptSubmit` | Injects `prompt_context` with file/symbol hints from the prompt |

### Recommendation E1: The Hook System Is the Most Powerful Vector

The `PreToolUse` hook is the **single most effective** mechanism for steering tool preference because it fires **at decision time** — right when the LLM is about to call a built-in tool. The suggestion text appears in context and directly influences the next tool call.

**Priority**: Port hook-like behavior to more clients.

### Recommendation E2: Strengthen Pre-Tool Suggestions

Current pre-tool suggestions are informational. They could be more directive:

**Current** (Read):
```
"SymForge MCP is connected. Prefer get_file_context (outline + imports + consumers) or 
get_symbol/get_symbol_context (targeted symbol lookup) over Read for source code inspection."
```

**Proposed**:
```
"⚡ SymForge can answer this more efficiently:
• get_file_context(path='...') — 60-90% smaller than reading the full file
• get_symbol(path='...', name='...') — read just the function you need
The Read tool will still work, but these save significant tokens."
```

### Recommendation E3: Post-Tool Enrichment Is Free Reinforcement

After a Read/Grep/Edit, the PostToolUse hook injects SymForge context. This is "free" because it doesn't block the user's intent — it adds value on top. The current implementation is already good. One enhancement: add a subtle footer to PostToolUse responses:

```
"(Enriched by SymForge MCP — use get_file_context or search_text directly for faster results)"
```

### Recommendation E4: Expand Hook Support Beyond Claude

For clients that don't support hooks natively, consider alternative approaches:

1. **Tool description self-promotion**: Already covered in Vector A
2. **Server-sent notifications**: MCP supports notifications — could send usage tips
3. **Resource-based guidance**: Expose a `SymForge://guidance/tool-routing` resource that clients can read at session start
4. **Prompt template integration**: The existing `code_review`, `architecture_map`, and `failure_triage` prompts could include tool routing instructions

---

## Vector F: Automatic Discovery and Smart Initialization (New)

The user specified: "easy adoption, automatic setup, intelligent discovery."

### Recommendation F1: Auto-Init on First Tool Call

When the MCP server starts and detects no guidance files exist for the current project:
1. Auto-create `.symforge/` marker directory
2. Write project-level guidance to the detected client's format
3. Log: "SymForge: auto-initialized guidance for [client]. Run `SymForge init` for full setup."

This makes SymForge "just work" without requiring `SymForge init`.

### Recommendation F2: Multi-Client Detection in `SymForge init`

Instead of requiring `--client claude` or `--client codex`, `SymForge init` should:

1. Scan for ALL known client configuration directories:
   - `~/.claude/` → Claude Code
   - `~/.codex/` → OpenAI Codex
   - `~/.gemini/` → Gemini CLI
   - `~/.kilocode/` or Kilo's config location → Kilo Code
   - `.cursor/` in project → Cursor
   - `.windsurfrules` in project → Windsurf
   - `.clinerules` in project → Cline/Roo
2. Auto-detect and configure ALL found clients
3. Report what was configured: "Configured: Claude ✅, Codex ✅, Kilo ✅, Cursor ✅"

### Recommendation F3: npm Install Hook

The npm `postinstall` script in `npm/scripts/install.js` should:
1. Download the binary (already done)
2. Run `SymForge init --auto-detect` to configure all found clients
3. Print clear instructions for any clients that need manual config

---

## Implementation Priority

### Phase 1: High-Impact, Low-Effort

| Change | Files | Impact |
|--------|-------|--------|
| Add edit tools to `SYMFORGE_TOOL_NAMES` | `src/cli/init.rs` | Eliminates approval friction for edits |
| Add explore to `SYMFORGE_TOOL_NAMES` | `src/cli/init.rs` | Explore is a key entry point |
| Expand guidance blocks from 3 to 9 decision rules | `src/cli/init.rs` | Strongest behavioral influence |
| Add token efficiency signals to tool descriptions | `src/protocol/tools.rs` | LLMs see this on every tool list |
| Add "prefer over" to `get_file_content` description | `src/protocol/tools.rs` | Discourages raw file reads |

### Phase 2: Medium-Impact, Medium-Effort

| Change | Files | Impact |
|--------|-------|--------|
| Add Kilo Code init support | `src/cli/init.rs` | Covers a growing client |
| Add contextual next-step hints to tool responses | `src/protocol/format.rs` | Guides tool chains |
| Strengthen pre-tool hook suggestions | `src/cli/hook.rs` | More directive interception |
| Error messages redirect to SymForge tools | `src/protocol/tools.rs`, `format.rs` | Catches failure paths |
| Add health quick-start section | `src/protocol/tools.rs` | Onboarding for new sessions |

### Phase 3: Broad Coverage, Higher Effort

| Change | Files | Impact |
|--------|-------|--------|
| Auto-detect all clients in `SymForge init` | `src/cli/init.rs` | Universal setup |
| Write project-level guidance per client | `src/cli/init.rs` | Per-project steering |
| Auto-init on first MCP connection | `src/main.rs` or `src/daemon.rs` | Zero-config experience |
| npm postinstall auto-init | `npm/scripts/install.js` | Seamless npm adoption |
| Cursor/Windsurf/Cline rule file generation | `src/cli/init.rs` | Market coverage |

---

## Concrete Code Changes

### Change 1: Update `SYMFORGE_TOOL_NAMES` in `src/cli/init.rs`

Add the missing tools:
```rust
const SYMFORGE_TOOL_NAMES: &[&str] = &[
    // ... existing 24 tools ...
    "mcp__SYMFORGE__explore",
    "mcp__SYMFORGE__replace_symbol_body",
    "mcp__SYMFORGE__insert_symbol",
    "mcp__SYMFORGE__delete_symbol",
    "mcp__SYMFORGE__edit_within_symbol",
    "mcp__SYMFORGE__batch_edit",
    "mcp__SYMFORGE__batch_rename",
    "mcp__SYMFORGE__batch_insert",
];
```

### Change 2: Expand Guidance Blocks in `src/cli/init.rs`

Replace the 3-bullet guidance with the 9-rule decision guide from Recommendation B1.

### Change 3: Update Tool Descriptions in `src/protocol/tools.rs`

For `get_file_context`:
```rust
description = "Rich file summary: symbol outline, imports, consumers, references, and git activity. 
Typically 60-90% smaller than raw file content. Use sections=['outline'] for symbol-only outline. 
Best tool for understanding a file before editing — prefer this over reading the full file. 
NOT for reading actual source code (use get_file_content or get_symbol)."
```

For `get_file_content`:
```rust
description = "Read raw file content. IMPORTANT: For code understanding, prefer get_file_context 
(60-90% smaller) or get_symbol (targeted lookup) first. Only use this when you need actual source 
text that structured tools don't provide. Modes: full file, line range, around_line/around_match/
around_symbol, or chunked paging."
```

For `get_symbol_context`:
```rust
description = "Symbol usage analysis with three modes — the most powerful code understanding tool. 
(1) Default: definition + callers + callees + type usages. 
(2) bundle=true: symbol body + all referenced types resolved recursively — best for edit preparation. 
(3) sections=[...]: comprehensive trace — dependents, siblings, implementations, git activity. 
Set verbosity='signature' for ~80% smaller output. NOT for just the symbol body (use get_symbol)."
```

### Change 4: Add Next-Step Hints to `src/protocol/format.rs`

```rust
pub fn next_step_hint(tool_name: &str, result: &str) -> String {
    // Only show hints when the result is substantial enough to be useful
    if result.len() < 100 { return String::new(); }
    
    match tool_name {
        "get_file_context" => {
            "\n\n💡 To read a specific symbol: get_symbol(path, name)\n💡 To see callers: find_references(name)".to_string()
        }
        "search_symbols" => {
            "\n\n💡 To read the body: get_symbol(path, name)\n💡 For usage analysis: get_symbol_context(name)".to_string()
        }
        "health" if /* first session check */ => {
            "\n\n── Quick Start ──\n1. get_repo_map — project overview\n2. explore(\"topic\") — find relevant code\n3. get_file_context(\"path\") — understand a file".to_string()
        }
        _ => String::new(),
    }
}
```

---

## Anti-Patterns to Avoid

1. **Don't be hostile to built-in tools**. Saying "NEVER use Read" will annoy users and may be ignored by LLMs that detect adversarial instructions.

2. **Don't add hints to every response**. A hint on every tool call becomes noise that LLMs learn to ignore. Be selective — only hint when the next step isn't obvious.

3. **Don't make tool descriptions too long**. MCP clients have token budgets for tool listings. Long descriptions may get truncated. Keep descriptions under ~200 words.

4. **Don't require `SymForge init` for the system to work**. The MCP server should function well without init — init just makes it work *better*.

5. **Don't duplicate guidance across tool descriptions and guidance files**. Tool descriptions should focus on "what this tool does" and "when to use it". Guidance files should focus on "overall workflow" and "decision rules".

---

## Success Metrics

After implementation, measure:

1. **Tool call distribution**: What % of code navigation uses SymForge tools vs built-in tools? Target: >80% SymForge for source code.
2. **Token efficiency**: Average tokens consumed per code understanding task. Target: 50% reduction.
3. **First-session adoption**: Does the LLM use SymForge tools in the first 5 tool calls? Target: yes in >90% of sessions.
4. **Cross-client coverage**: How many clients auto-configure on `SymForge init`? Target: 5+ clients.

---

## Summary of All Recommendations

| ID | Recommendation | Vector | Priority |
|----|---------------|--------|----------|
| A1 | Add token efficiency signals to tool descriptions | Tool Descriptions | Phase 1 |
| A2 | Add "prefer over" phrases to `get_file_content` | Tool Descriptions | Phase 1 |
| A3 | Strengthen `explore` as entry point | Tool Descriptions | Phase 1 |
| A4 | Keep current tool names (no changes) | Tool Descriptions | N/A |
| A5 | Add workflow hints to search tool descriptions | Tool Descriptions | Phase 1 |
| B1 | Expand guidance to 9 decision rules | Init Guidance | Phase 1 |
| B2 | Client-specific guidance customization | Init Guidance | Phase 2 |
| B3 | Write project-level guidance automatically | Init Guidance | Phase 3 |
| B4 | Assertive but not annoying tone | Init Guidance | Phase 1 |
| C1 | Add contextual next-step hints to responses | Tool Responses | Phase 2 |
| C2 | Error messages redirect to SymForge tools | Tool Responses | Phase 2 |
| C3 | Health tool quick-start for new sessions | Tool Responses | Phase 2 |
| D1 | Add edit tools to `SYMFORGE_TOOL_NAMES` | alwaysAllow | Phase 1 |
| D2 | Client-specific alwaysAllow configuration | alwaysAllow | Phase 2 |
| D3 | Kilo Code MCP config support | alwaysAllow | Phase 2 |
| E1 | Hook system is highest-impact — prioritize porting | Hook Enrichment | Phase 2 |
| E2 | Strengthen pre-tool suggestions | Hook Enrichment | Phase 2 |
| E3 | Add enrichment footer to post-tool responses | Hook Enrichment | Phase 2 |
| E4 | Alternative approaches for non-hook clients | Hook Enrichment | Phase 3 |
| F1 | Auto-init on first tool call | Discovery | Phase 3 |
| F2 | Multi-client detection in `SymForge init` | Discovery | Phase 3 |
| F3 | npm postinstall auto-init | Discovery | Phase 3 |
