---
doc_type: task
task_id: 107
title: P1 prompt_context qualified module alias shell
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 106-T-p1-prompt-context-qualified-module-alias-contract-research.md
next_task: 108-T-p1-prompt-context-module-alias-file-hint-contract-research.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 107: P1 Prompt Context Qualified Module Alias Shell

## Objective

- let prompt-context consume exact qualified module aliases like `crate::db:line` for combined file+symbol prompts while preserving the existing path-shaped routes

## Why This Exists

- task 106 chooses exact qualified module aliases as the next safe prompt-context boundary after path-shaped hints
- some prompts refer to logical modules instead of file paths, but this should stay explicit and deterministic

## Read Before Work

- [106-R-p1-prompt-context-qualified-module-alias-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/106-R-p1-prompt-context-qualified-module-alias-contract-research.md)
- [106-T-p1-prompt-context-qualified-module-alias-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/106-T-p1-prompt-context-qualified-module-alias-contract-research.md)
- [105-T-p1-prompt-context-qualified-extensionless-path-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/105-T-p1-prompt-context-qualified-extensionless-path-shell.md)

## Expected Touch Points

- `src/sidecar/handlers.rs`
- `tests/sidecar_integration.rs`

## Deliverable

- a prompt-context shell that accepts exact qualified module aliases like `crate::db:2` and routes them into the exact-selector symbol-context lane

## Done When

- exact qualified module aliases resolve through `symbol_line`
- partial or fuzzy module aliases do not activate exact selection
- existing exact-path, basename, stem, and qualified path behavior stay intact
- focused tests cover the new module-alias route and its guardrail behavior

## Completion Notes

- extended prompt-context file-hint matching to recognize exact qualified module aliases such as `crate::db:2`
- reused language-derived module-path aliases as the active `:line` alias for exact selector routing
- preserved exact-path, basename, extensionless-path, bare-stem, and explicit `line N` behavior
- added focused unit and endpoint coverage for the new module-alias line-hint route plus a partial-module guardrail

## Carry Forward To Next Task

Next task:

- `108-T-p1-prompt-context-module-alias-file-hint-contract-research.md`

Carry forward:

- keep accepted module aliases exact and explicitly qualified
- preserve current path-shaped fallback behavior
- avoid broadening this slice into generic module guessing

Open points:

- OPEN: whether exact qualified module aliases should also activate file hints without `:line`
