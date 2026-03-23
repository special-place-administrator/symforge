# CLAUDE.md — Global Operating System

> Follow precisely. This is the minimal always-loaded instruction set.
> Reference content lives in `~/.claude/skills/` and loads on demand.

---

### 1. Plan Node Default
- Enter plan mode for ANY non-trivial task (3+ steps or architectural decisions)
- If something goes sideways, STOP and re-plan immediately - don't keep pushing
- Use plan mode for verification steps, not just building
- Write detailed specs upfront to reduce ambiguity

---

### 2. Subagent Strategy
- Use subagents liberally to keep main context window clean
- Offload research, exploration, and parallel analysis to subagents
- For complex problems, throw more compute at it via subagents
- One task per subagent for focused execution

---

### 3. Self-Improvement Loop
- After ANY correction from the user: update `tasks/lessons.md` with the pattern
- Write rules for yourself that prevent the same mistake
- Ruthlessly iterate on these lessons until mistake rate drops
- Review lessons at session start for relevant project

---

### 4. Verification Before Done
- Never mark a task complete without proving it works
- Diff behavior between main and your changes when relevant
- Ask yourself: "Would a staff engineer approve this?"
- Run tests, check logs, demonstrate correctness

---

### 5. Demand Elegance (Balanced)
- For non-trivial changes: pause and ask "is there a more elegant way?"
- If a fix feels hacky: "Knowing everything I know now, implement the elegant solution"
- Skip this for simple, obvious fixes - don't over-engineer
- Challenge your own work before presenting it

---

### 6. Autonomous Bug Fixing
- When given a bug report: just fix it. Don't ask for hand-holding
- Point at logs, errors, failing tests - then resolve them
- Zero context switching required from the user
- Go fix failing CI tests without being told how

---

## Task Management
1. **Plan First**: Write plan to `tasks/todo.md` with checkable items
2. **Verify Plan**: Check in before starting implementation
3. **Track Progress**: Mark items complete as you go
4. **Explain Changes**: High-level summary at each step
5. **Document Results**: Add review section to `tasks/todo.md`
6. **Capture Lessons**: Update `tasks/lessons.md` after corrections

---

## Core Principles
* **Simplicity First**: Make every change as simple as possible. Impact minimal code.
* **No Laziness**: Find root causes. No temporary fixes. Senior developer standards.
* **Minimal Impact**: Changes should only touch what's necessary. Avoid introducing bugs.

## Architecture

Rust MCP server providing symbol-aware code navigation and editing tools. Currently 24 tools exposed via MCP `tools/list`, with backward-compat aliases for removed tools in `src/daemon.rs`.

Key source files:
- `src/protocol/tools.rs` — Tool handlers, input structs, tests
- `src/protocol/format.rs` — Output formatters
- `src/daemon.rs` — Daemon proxy with backward-compat aliases
- `src/cli/init.rs` — Tool name list for client init
- `src/live_index/query.rs` — Index query functions
- `src/protocol/resources.rs` — MCP resource handlers

## Tool Consolidation Pattern

When merging tools A into B:
1. Add new params to B's input struct (with `#[serde(default)]`)
2. Add mode branch in B's handler
3. Remove `#[tool]` attribute from A (keep the method for internal use)
4. Add backward-compat alias in `src/daemon.rs` `execute_tool_call`
5. Remove A from `SYMFORGE_TOOL_NAMES` in `src/cli/init.rs`
6. Update cross-reference descriptions in other tools
7. Update tests: add new field initializers, add mode-specific tests

## Codex Integration

For Codex-specific integration guidance and limitations, see [docs/codex-integration-ceiling.md](docs/codex-integration-ceiling.md).
