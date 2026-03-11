# Client Maximization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Push Tokenizor as far as documented Claude Code and Codex integrations allow without relying on undocumented client behavior.

**Architecture:** Keep the shared MCP daemon/tools/resources/prompts as the authoritative product surface. Layer client-native setup on top through `tokenizor init`: Codex gets stronger MCP config plus global AGENTS guidance, and Claude gets global CLAUDE guidance plus one additional documented hook surface where it materially improves UX.

**Tech Stack:** Rust, rmcp, serde_json, toml_edit, cargo test

---

### Task 1: Add failing init tests for client-native guidance assets

**Files:**
- Modify: `tests/init_integration.rs`
- Modify: `src/cli/init.rs`

**Step 1: Write the failing tests**

- Add a Claude init test that expects `~/.claude/CLAUDE.md` to contain a bounded Tokenizor guidance block after `run_init_with_context(InitClient::Claude, ...)`.
- Add a Codex init test that expects `~/.codex/AGENTS.md` to contain a bounded Tokenizor guidance block after `run_init_with_context(InitClient::Codex, ...)`.
- Add preservation/idempotency tests that keep unrelated existing guidance content intact across re-runs.

**Step 2: Run the targeted tests to verify they fail**

Run: `cargo test init_integration -- --nocapture`

Expected: FAIL because init does not yet create either guidance file.

**Step 3: Implement the minimal guidance-file support**

- Extend `InitPaths` with:
  - `claude_memory`
  - `codex_agents`
- Add a shared helper that upserts a clearly delimited Tokenizor guidance block into markdown text files without replacing unrelated content.
- Call the helper from `run_init_with_context` for Claude and Codex only when those clients are selected.

**Step 4: Run the targeted tests to verify they pass**

Run: `cargo test init_integration -- --nocapture`

Expected: PASS for the new guidance-file tests.

### Task 2: Add failing tests for Codex config tuning

**Files:**
- Modify: `tests/init_integration.rs`
- Modify: `src/cli/init.rs`

**Step 1: Write the failing tests**

- Add a Codex init test that expects the Tokenizor server entry in `~/.codex/config.toml` to include documented runtime knobs:
  - `startup_timeout_sec`
  - `tool_timeout_sec`
- Add a Codex init test that expects `project_doc_fallback_filenames` to include `CLAUDE.md` while preserving any existing fallback list items.

**Step 2: Run the targeted tests to verify they fail**

Run: `cargo test init_integration -- --nocapture`

Expected: FAIL because the current TOML merge only writes `command`.

**Step 3: Implement the minimal TOML merge changes**

- Update `merge_tokenizor_codex_server` to set conservative Tokenizor defaults for `startup_timeout_sec` and `tool_timeout_sec`.
- Add a helper that merges `CLAUDE.md` into `project_doc_fallback_filenames` idempotently.
- Preserve existing unrelated Codex config and comments.

**Step 4: Run the targeted tests to verify they pass**

Run: `cargo test init_integration -- --nocapture`

Expected: PASS for the new Codex config tests.

### Task 3: Add one extra Claude-only automation surface

**Files:**
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/init.rs`
- Modify: `src/cli/hook.rs`
- Modify: `tests/init_integration.rs`
- Modify: `tests/sidecar_integration.rs`

**Step 1: Write the failing tests**

- Add tests for one additional documented Claude hook surface.
- Prefer `UserPromptSubmit` if the payload shape and output contract fit the existing fast fail-open command hook model cleanly.
- If `UserPromptSubmit` would be too speculative, stop and select the next safest documented hook instead of forcing it.

**Step 2: Run the targeted tests to verify they fail**

Run: `cargo test hook -- --nocapture`

Expected: FAIL because only `PostToolUse` and `SessionStart` are currently installed/routed.

**Step 3: Implement the minimal hook expansion**

- Extend init hook merge logic to install the new hook entry.
- Extend `run_hook` routing only if the event is fully documented and can safely reuse existing daemon-backed endpoints.
- Keep hook runtime fail-open and low-latency.

**Step 4: Run the targeted tests to verify they pass**

Run: `cargo test hook -- --nocapture`

Expected: PASS for the new hook behavior.

### Task 4: Update docs and verify end to end

**Files:**
- Modify: `README.md`

**Step 1: Document the new client-native enhancements**

- Describe Codex MCP config tuning and the `CLAUDE.md` fallback behavior.
- Describe Claude global guidance and the additional hook automation.
- Keep the remaining Codex/Claude differences explicit and honest.

**Step 2: Run verification**

Run: `cargo test`

Expected: PASS.

**Step 3: Commit**

```bash
git add README.md docs/plans/2026-03-11-client-maximization.md src/cli/init.rs src/cli/hook.rs src/cli/mod.rs tests/init_integration.rs tests/sidecar_integration.rs
git commit -m "feat: maximize codex and claude integration"
```
