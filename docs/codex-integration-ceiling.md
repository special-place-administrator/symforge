# Codex Integration Ceiling: What's Fixable vs What's Blocked

Status: internal reference document
Date: 2026-03-20
SymForge version context: v1.6.0+

## Purpose

This document separates what SymForge can do to maximize Codex behavior (within our control) from what's blocked until Codex exposes deeper integration surfaces (outside our control). It's for internal decision-making about where to invest engineering effort.

---

## 1. Current State (as of v1.6.0+)

### What SymForge does for Codex today

**MCP server registration.** `symforge init codex` registers SymForge as an MCP server in `~/.codex/config.toml` with startup and tool timeouts. Codex can discover and call all SymForge tools through its native MCP support. Implementation: [`merge_symforge_codex_server()`](src/cli/init.rs:514) and [`register_codex_mcp_server()`](src/cli/init.rs:483).

**AGENTS.md guidance.** Init writes a SymForge guidance block into `~/.codex/AGENTS.md` with decision rules, tool preference lists, and "when to fall back" guidance. This is the same guidance content used for Claude's `CLAUDE.md`. Implementation: [`codex_guidance_block()`](src/cli/init.rs:810) delegates to [`claude_guidance_block()`](src/cli/init.rs:712).

**Allowed tools list.** The Codex config includes an `allowed_tools` array containing all SymForge tool names (without the `mcp__symforge__` prefix Codex doesn't use). This removes per-call approval friction. Implementation: the loop in [`merge_symforge_codex_server()`](src/cli/init.rs:514).

**Project doc fallback filenames.** Init ensures `project_doc_fallback_filenames` in Codex config includes both `AGENTS.md` and `CLAUDE.md`, so Codex reads project-level guidance that may reference SymForge. Implementation: [`merge_codex_project_doc_fallbacks()`](src/cli/init.rs:548).

**Tool descriptions with "prefer over" language.** Key tool descriptions explicitly say "Prefer this over reading an entire file", "Prefer this over raw file reads", "Prefer this over grep/ripgrep" — steering LLMs toward SymForge tools at the MCP protocol level. Implementation: tool descriptions in [`src/protocol/tools.rs`](src/protocol/tools.rs:1497).

**Next-step hints in tool responses.** Every tool response includes a contextual "Tip:" footer suggesting logical next tools, reinforcing the SymForge workflow loop. Implementation: [`compact_next_step_hint()`](src/protocol/format.rs:3064).

**MCP resources.** SymForge exposes subscribable resources (repo health, repo outline, repo map, uncommitted changes, file context, file content, symbol detail, symbol context) that Codex can query. Implementation: [`src/protocol/resources.rs`](src/protocol/resources.rs).

**MCP prompts.** SymForge exposes structured prompts (code review, architecture map, failure triage) that Codex can invoke. Implementation: [`src/protocol/prompts.rs`](src/protocol/prompts.rs).

**Hook adoption metrics.** The `health` tool reports hook adoption metrics, which show up for any client — though Codex won't generate hook events since it has no hook system. Implementation: health tool in [`src/protocol/tools.rs`](src/protocol/tools.rs:2378).

### What actually works in practice

Codex reads `AGENTS.md` at session start and learns about SymForge tools. When the guidance is well-written and the task aligns, Codex voluntarily uses `get_file_context` instead of reading files directly, `search_text` instead of `grep`, and `search_symbols` instead of manual scanning. But this is probabilistic — Codex may fall back to shell commands at any time based on its own reasoning, and there is no mechanism to enforce or even detect when it does.

The init system already prints an honest diagnostic:

> "note: Codex gets MCP tools only. No documented Codex hook/session-start enrichment interface was found, so transparent enrichment remains Claude-only."

---

## 2. What's In Our Control (actionable improvements)

These are investments that can improve Codex behavior without any changes from OpenAI:

### Codex-specific guidance content

Currently [`codex_guidance_block()`](src/cli/init.rs:810) is a thin delegation to [`claude_guidance_block()`](src/cli/init.rs:712). We could write Codex-optimized guidance that:

- Uses Codex's reasoning style and instruction-following patterns
- References Codex-specific concepts (skills, project docs) rather than Claude-specific ones
- Is tuned for Codex's typical failure modes (e.g., Codex's stronger tendency to shell out)
- Includes more assertive "always do X before Y" framing if that works better with Codex models

### Codex Skills integration

Codex has a first-class Skills system that discovers skills from repository, user, admin, and system locations. SymForge could ship as a Codex skill or generate skill definitions during init, giving Codex a more native integration surface than raw MCP alone. This was identified in the [provider CLI integration research](docs/provider_cli_integration_research.md:74) but not yet implemented.

### Stronger tool descriptions

Tool descriptions could include even more explicit routing language, perhaps with concrete examples:

- "When you want to understand a file, call this BEFORE using cat/read"
- "Returns the same information as grep but with structural context"

### Self-reinforcing tool responses

Every SymForge tool response already has "Tip:" footers. These could be strengthened to:

- Include workflow-completion suggestions ("You searched for symbols — now use `get_symbol` to read the top match")
- Embed context about what SymForge saved vs what a raw approach would cost
- Suggest the next logical SymForge tool rather than leaving Codex to decide

### Richer MCP resource surfaces

Codex can subscribe to MCP resources. We could add:

- A "recommended next actions" resource based on recent tool calls
- A session-scoped "what have I used SymForge for" resource
- Project-specific resource surfaces that Codex auto-discovers

### Project-level `.codex/config.toml` generation

Currently init writes only the global `~/.codex/config.toml`. We could also generate a project-scoped `.codex/config.toml` that:

- Registers SymForge with project-specific settings
- Contains project-aware tool preferences

### Error messages that guide

When a SymForge tool fails or returns empty results, the error message should suggest alternative SymForge tools rather than leaving Codex to fall back to shell commands.

---

## 3. What's Blocked by Codex (outside our control)

### No hook/session-start surface

Claude Code exposes a hook system where SymForge can intercept tool calls at `PreToolUse` and `PostToolUse` events. When Claude is about to run `cat file.rs`, the hook fires, SymForge intercepts it, and returns enriched context from `get_file_context` instead.

Codex has no equivalent. There is no documented mechanism to:

- Run code when a Codex session starts
- Intercept or redirect a tool call before it executes
- Enrich a tool's result after it executes
- Inject context into the conversation at session start

This is the single largest gap. The entire [`HookWorkflow`](src/cli/hook.rs:58) system — which handles `SourceRead`, `SourceSearch`, `RepoStart`, `PromptContext`, `PostEditImpact`, and `CodeEdit` interceptions — is Claude-only.

### No way to force MCP tool preference over built-in tools

Codex decides which tools to use based on its own reasoning. Even with `AGENTS.md` guidance and well-written tool descriptions, Codex can choose to run `cat`, `grep`, `find`, or any shell command instead of the equivalent SymForge tool. There is no mechanism to:

- Mark an MCP tool as "preferred over" a built-in equivalent
- Disable or de-prioritize specific built-in tools
- Force tool routing decisions

### Codex's shell execution can't be intercepted

Codex runs shell commands directly. When it decides to `grep -r "pattern" src/`, that command goes straight to the shell. SymForge cannot:

- See that the command was issued
- Redirect it to `search_text`
- Enrich the result with structural context
- Log the bypass for adoption metrics

### No plugin/extension API for behavior modification

While Codex has Skills, SDK, and app-server surfaces, none of these provide:

- A mechanism to modify Codex's tool selection behavior
- A way to intercept and redirect built-in operations
- A hook into Codex's file-reading pipeline

Skills are more like "additional capabilities" than "behavior modifiers."

### No hook adoption telemetry for Codex

Since Codex doesn't fire hook events, the adoption metrics system has no data for Codex sessions. We can't measure:

- How often Codex uses SymForge tools vs shell equivalents
- Which workflows Codex routes through SymForge
- Whether guidance changes improve adoption

---

## 4. Maximum Achievable Behavior Today

### Best-case workflow

1. User runs `symforge init codex` — MCP server registered, `AGENTS.md` written
2. User starts a Codex session in a project
3. Codex reads `AGENTS.md` and learns SymForge's tool preference rules
4. Codex sees SymForge tools in its MCP tool list with descriptive "prefer over" language
5. For file understanding: Codex calls `get_file_context` instead of `cat` — **when guided**
6. For code search: Codex calls `search_text` instead of `grep` — **when guided**
7. For symbol lookup: Codex calls `search_symbols` instead of manual scanning — **when guided**
8. For editing: Codex calls `replace_symbol_body` instead of manual find-replace — **when guided**
9. Each tool response includes a "Tip:" suggesting the next logical SymForge tool
10. Codex follows the suggestion chain, staying in the SymForge workflow

### What this looks like in practice

Codex's SymForge usage is **voluntary and probabilistic**. In observed sessions:

- Codex often uses SymForge tools when the `AGENTS.md` guidance is clear and the task naturally calls for search/read/edit operations
- Codex sometimes falls back to shell commands, especially for quick one-off operations or when its built-in patterns are strong
- Codex does not consistently use SymForge for every file read — it may `cat` a file directly if it seems simpler
- There is no enforcement boundary — Codex can always choose the non-SymForge path

### The gap: "guidance-steered" vs "transparently intercepted"

| Dimension | Claude (intercepted) | Codex (guided) |
|---|---|---|
| First file read | Hook fires → `get_file_context` automatically | Codex may or may not call `get_file_context` |
| `cat file.rs` intent | PreToolUse intercepts → enriched result | Shell command executes directly, unintercepted |
| `grep pattern src/` intent | PreToolUse intercepts → `search_text` result | Shell command executes directly, unintercepted |
| Session start context | Hook injects repo map automatically | Codex reads AGENTS.md, may or may not call `get_repo_map` |
| Post-edit impact | PostToolUse fires → `analyze_file_impact` automatically | Codex must voluntarily call it (rarely does) |
| Adoption measurement | Full hook event telemetry | No telemetry — cannot measure Codex adoption |
| Enforcement | SymForge controls the result | SymForge provides guidance, Codex decides |

The fundamental difference: with Claude, SymForge is in the critical path. With Codex, SymForge is an optional alternative.

---

## 5. What Codex Would Need to Expose

To reach parity with Claude's integration depth, Codex would need:

### A hook/session-start callback

An equivalent to Claude's `PreToolUse` / `PostToolUse` / `Notification` hooks — a mechanism where an external process can:

- Be notified when Codex is about to execute a tool
- Return an alternative result (intercept the tool call)
- Be notified after a tool executes (enrich the result)
- Be notified when a session starts (inject initial context)

### An MCP tool preference mechanism

A way to declare that an MCP tool should be preferred over a built-in equivalent. For example:

- "When the user/agent wants to read a file, prefer `mcp_symforge_get_file_context` over the built-in file read"
- "When the user/agent wants to search, prefer `mcp_symforge_search_text` over shell grep"

This could be a config-level declaration or an MCP protocol extension.

### A file-read interception layer

A mechanism where MCP servers can register as "file read providers" — when Codex reads a source file, the MCP server gets first opportunity to provide the content (potentially enriched with structural context).

### A prompt/context enrichment hook

A mechanism to inject structured context into the conversation at session start or at specific points, similar to how Claude's `Notification` hook type allows SymForge to inject a repo map when a session opens.

### An adoption telemetry surface

Even without full hooks, a mechanism to:

- Report which tools Codex actually called in a session
- Report which built-in operations Codex used instead of available MCP tools
- Provide a feedback loop for measuring guidance effectiveness

---

## 6. Recommended Strategy Until Then

### Focus on guidance quality, not interception attempts

Since we cannot intercept Codex's operations, every investment should go toward making SymForge's guidance as effective as possible:

- Write Codex-specific `AGENTS.md` content (not just reusing Claude's)
- Test different guidance framings empirically with Codex
- Iterate on tool descriptions based on observed Codex behavior

### Make every tool response self-reinforcing

Each SymForge tool response should teach Codex to use SymForge again:

- "Tip:" footers should suggest the next logical SymForge tool
- Results should demonstrate their value (e.g., "3,847 tokens saved vs raw file read")
- Error messages should guide toward other SymForge tools, not shell fallbacks

### Explore Codex Skills as a deeper integration surface

Skills may allow SymForge to be more natively integrated than plain MCP. Investigate:

- Can a skill definition influence Codex's tool selection?
- Can a skill wrap multiple MCP tools into higher-level operations?
- Can skills provide session-start behavior that plain MCP cannot?

### Explore Codex SDK for orchestration scenarios

The Codex SDK allows programmatic control of Codex agents. For internal or advanced use cases:

- SymForge could orchestrate Codex sessions through the SDK
- SDK-driven sessions could enforce SymForge tool usage programmatically
- This is a strategic option, not a near-term priority

### Track what we can measure

Without hook telemetry, we can still measure:

- MCP tool call frequency through SymForge's own request logs
- Which tools Codex actually calls and how often
- Session-level patterns (does Codex use SymForge more at the start of a session?)

### Be ready to activate interception when Codex exposes hooks

The hook system architecture in [`src/cli/hook.rs`](src/cli/hook.rs) is already workflow-oriented with [`HookWorkflow`](src/cli/hook.rs:58) supporting `SourceRead`, `SourceSearch`, `RepoStart`, `PromptContext`, `PostEditImpact`, and `CodeEdit`. If Codex exposes a hook surface, SymForge's existing workflow routing can be adapted quickly — the logic already exists, only the client adapter is missing.

---

## Summary

| Category | Claude Code | Codex |
|---|---|---|
| MCP tools available | ✅ | ✅ |
| Guidance files | ✅ CLAUDE.md | ✅ AGENTS.md |
| Tool descriptions steer | ✅ | ✅ |
| Hook interception (PreToolUse) | ✅ | ❌ blocked by Codex |
| Hook enrichment (PostToolUse) | ✅ | ❌ blocked by Codex |
| Session-start context injection | ✅ | ❌ blocked by Codex |
| Shell command interception | ✅ via hooks | ❌ blocked by Codex |
| Adoption telemetry | ✅ hook events | ❌ blocked by Codex |
| Tool preference enforcement | ✅ via interception | ❌ guidance only |
| Next-step hints in responses | ✅ | ✅ |
| MCP resources | ✅ | ✅ |
| MCP prompts | ✅ | ✅ |
| Skills integration | N/A | 🔲 not yet explored |
| SDK orchestration | N/A | 🔲 strategic option |

**Bottom line:** SymForge can make Codex a good MCP consumer through guidance, tool descriptions, and response design. But without hook surfaces, Codex integration will remain "guidance-steered" rather than "transparently intercepted" — a real ceiling that only Codex can lift.
