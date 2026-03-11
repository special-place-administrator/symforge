# Hook Tool Parity Implementation Plan

**Goal:** Expose the useful intelligence currently delivered automatically through Claude hooks as standard MCP tools so Codex and other local MCP clients can access the same backend capabilities explicitly.

**Why this phase exists:** The shared daemon and stdio proxying are in place, but some high-value behavior still lives behind the sidecar hook HTTP surface. That creates a UX gap between Claude and clients that only support standard MCP tools. It also leaves one correctness hole: sidecar impact analysis currently falls back to process cwd instead of an explicit project root, which is unsafe for a shared multi-project daemon.

## Scope

- Add standard MCP tools for:
  - compact repo map
  - enriched file context
  - symbol context
  - file impact analysis
- Preserve existing Claude hook behavior.
- Make impact analysis project-root aware instead of cwd-dependent.

## Design

1. Extract the current sidecar logic into shared helper functions that can be called from:
   - HTTP hook handlers
   - MCP tool handlers
   - daemon-backed sessions
2. Extend sidecar state with an explicit `repo_root` so daemon sessions never rely on process cwd.
3. Add tool methods to `TokenizorServer`:
   - `get_repo_map`
   - `get_file_context`
   - `get_symbol_context`
   - `analyze_file_impact`
4. Keep Claude hooks as automation wrappers over the same shared logic.

## Verification

- Unit tests for each new tool.
- A daemon test proving impact analysis reads from the session project root, not daemon cwd.
- Full `cargo test`.
