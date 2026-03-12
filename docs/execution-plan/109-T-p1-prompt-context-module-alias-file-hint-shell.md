---
doc_type: task
task_id: 109
title: P1 prompt_context module alias file hint shell
status: done
sprint: tokenizor-upgrade-foundation
parent_plan: 05-P-validation-and-backlog.md
prev_task: 108-T-p1-prompt-context-module-alias-file-hint-contract-research.md
next_task: 110-T-p1-prompt-context-qualified-symbol-alias-contract-research.md
created: 2026-03-12
updated: 2026-03-12
---
# Task 109: P1 Prompt Context Module Alias File Hint Shell

## Objective

- let prompt-context consume exact qualified module aliases like `crate::db` as file hints for combined file+symbol prompts while preserving the current `:line` lane

## Why This Exists

- task 108 chooses exact no-line module aliases as the next small prompt-context improvement after `crate::db:2`
- exact module aliases should behave like existing exact file hints when they identify one indexed file

## Read Before Work

- [108-R-p1-prompt-context-module-alias-file-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/108-R-p1-prompt-context-module-alias-file-hint-contract-research.md)
- [108-T-p1-prompt-context-module-alias-file-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/108-T-p1-prompt-context-module-alias-file-hint-contract-research.md)
- [107-T-p1-prompt-context-qualified-module-alias-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/107-T-p1-prompt-context-qualified-module-alias-shell.md)

## Expected Touch Points

- `src/sidecar/handlers.rs`
- `tests/sidecar_integration.rs`

## Deliverable

- a prompt-context shell that accepts exact qualified module aliases like `crate::db` as file hints and routes combined prompts into the exact-selector symbol-context lane

## Done When

- exact qualified module aliases activate file hints without `:line`
- partial or fuzzy module aliases do not activate exact selection
- existing line-hint and path-shaped behavior stay intact
- focused tests cover the new no-line module-alias route and its boundary guardrail

## Completion Notes

- extended prompt-context so exact qualified module aliases like `crate::db` can activate file hints without `:line`
- added a boundary-aware matcher so prefixes like `crate::dbx` and continued paths like `crate::db::connect` do not collapse to `crate::db`
- preserved the existing `crate::db:line` path and all prior path-shaped and fallback behavior
- added focused unit and endpoint coverage for the new no-line module-alias route and its guardrail

## Carry Forward To Next Task

Next task:

- `110-T-p1-prompt-context-qualified-symbol-alias-contract-research.md`

Carry forward:

- keep module aliases exact and boundary-aware
- preserve the existing `:line` exact-selector path
- avoid broadening this slice into fuzzy module guessing

Open points:

- OPEN: whether prompt-context should accept fully qualified symbol aliases like `crate::db::connect` directly
