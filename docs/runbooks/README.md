# Runbooks

Step-by-step procedures for repeatable SymForge operations. Each runbook is a
checklist that an engineer (or agent) can follow without re-deriving the
sequence from first principles.

## When to write a runbook

Write one when a procedure:

- touches more than one file in lockstep (e.g., adding an MCP tool requires
  edits in `tools.rs`, `format.rs`, `cli/init.rs`, and tests in concert)
- has an ordering constraint that, if violated, breaks clients silently
  (e.g., removing a `SYMFORGE_TOOL_NAMES` entry without first adding a
  backward-compat alias in `src/daemon.rs`)
- requires running both `cargo` and `npm` test suites to verify
- has been performed at least twice from memory, suggesting it deserves to be
  written down

Do **not** write runbooks for one-off investigations (use a regular doc) or
for design rationale (use [`docs/decisions/`](../decisions/)).

## Suggested runbooks (not yet written)

These are the procedures most worth capturing, ranked by how much pain they
prevent:

1. **Add a new MCP tool** — handler in `src/protocol/tools.rs` (input struct +
   `impl SymForgeServer` method with `#[tool]`), output formatter in
   `src/protocol/format.rs`, register in `SYMFORGE_TOOL_NAMES`
   (`src/cli/init.rs:262-294`), wire daemon routing in `execute_tool_call`
   (`src/daemon.rs:1529-1648`), add tests in the `mod tests` block at the
   bottom of `tools.rs`. Verify with `cargo test --all-targets --
   --test-threads=1`.

2. **Consolidate tool A into tool B** — follow the seven-step pattern in
   `CLAUDE.md` ("Tool Consolidation Pattern"). Critical ordering: add
   backward-compat alias in `src/daemon.rs::execute_tool_call` **before**
   removing A from `SYMFORGE_TOOL_NAMES`, otherwise existing client sessions
   break on the next tool call.

3. **Debug daemon backward-compat aliases** — the alias contract lives in
   `src/daemon.rs::execute_tool_call` (`L1529-1648`). When a client reports a
   "tool not found" for a removed tool, check (a) is there an alias branch in
   `execute_tool_call`, (b) does the alias dispatch to the consolidated
   tool's handler with correct mode/params. Test with
   `test_daemon_executes_session_scoped_tool_calls` in `src/daemon.rs` test
   module.

4. **Run npm tests** — `cd npm && npm test` (uses `node --test
   tests/*.test.js`, requires Node 18+). The npm wrapper drives the binary
   installed at `~/.symforge/bin/symforge` (or `%USERPROFILE%\.symforge\bin\
   symforge.exe` on Windows). For local development against a freshly built
   binary, set `SYMFORGE_HOME` to a temp dir and copy
   `target/release/symforge` into `$SYMFORGE_HOME/bin/`.

5. **Cut a release** — see [`../release-process.md`](../release-process.md)
   for the existing process; complement it with the steps in
   `.claude/skills/release/SKILL.md` for synchronized version bumps.

6. **Add a tree-sitter language** — register the grammar in `Cargo.toml`,
   wire it through `src/parsing/`, add language detection in `LanguageId`,
   and confirm symbol extraction with a fixture in `tests/`.

## Format

```markdown
# Runbook: <Goal>

Use this when: <one-line trigger condition>

## Preconditions

- [ ] <state the world must be in>

## Steps

1. <verb-first action> — verify: <what to check>
2. ...

## Verification

- [ ] `cargo check`
- [ ] `cargo test --all-targets -- --test-threads=1`
- [ ] `cd npm && npm test` (if `npm/` was touched)

## Rollback

<how to undo, or "no rollback needed">
```
