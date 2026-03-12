# Research: P1 Prompt Context Extensionless Alias Line Hint Contract

Related plan:

- [05-P-validation-and-backlog.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/05-P-validation-and-backlog.md)
- [100-R-p1-prompt-context-basename-line-hint-contract-research.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/100-R-p1-prompt-context-basename-line-hint-contract-research.md)
- [101-T-p1-prompt-context-basename-line-hint-shell.md](/E:/project/tokenizor_agentic_mcp/docs/execution-plan/101-T-p1-prompt-context-basename-line-hint-shell.md)

Goal:

- choose the smallest prompt-context follow-up contract that lets unique extensionless aliases like `db:2` feed the combined file+symbol exact-selector lane without turning prompt parsing into fuzzy module-name guessing

## Current Code Reality

After task 101, prompt-context accepts:

1. `line N`
2. exact `<resolved-path>:<line>`
3. unique basename `<file.rs>:<line>`

That still leaves one adjacent prompt shape unsupported:

- `db:2 connect`

In many repos this is how developers refer to `db.rs`, but support is riskier than basename `db.rs:2` because extensionless aliases are less explicit and more likely to collide with symbols, directories, or module names.

## Candidate Approaches

### Option 1: stop at basename `file.rs:line`

- safest boundary
- leaves a common shorthand unsupported

### Option 2: accept extensionless alias `name:line` only when it maps uniquely onto the already resolved file hint

- reuses existing file-hint resolution instead of adding a new alias resolver
- keeps the parser narrow and explicit
- avoids activating on ambiguous or unrelated bare tokens

### Option 3: accept any bare `name:line` token globally

- more permissive
- too risky because it blurs files, modules, symbols, and unrelated labels

## Decision: Add A Unique Extensionless-Alias Bridge

Recommendation:

- when prompt-context already resolves a concrete file hint
- derive the extensionless stem from that resolved file name
- allow `<stem>:<line>` only if the original file hint was already trustworthy under the existing file-hint logic

Keep current behavior intact:

- exact-path and basename-derived `:line` support stay
- explicit `line N` stays
- ambiguous or non-file bare tokens still do not activate line disambiguation

## Why This Is The Smallest Useful Slice

- it extends a natural developer shorthand without building a new resolver family
- it stays anchored to the file hint prompt-context already trusts
- it avoids treating arbitrary bare `name:line` tokens as actionable

## Recommended Next Implementation Slice

- extend prompt-context line-hint extraction to accept `<stem>:<line>` when the resolved file hint's basename stem is unique enough to have produced that hint
- keep existing `path:line`, `file.rs:line`, and `line N` behavior unchanged
- add focused tests for:
  - unique extensionless alias `db:2` disambiguates combined prompt routing
  - ambiguous or unrelated bare aliases do not activate line hints
  - existing basename `file.rs:line` behavior still works

Expected touch points:

- `src/sidecar/handlers.rs`
- `tests/sidecar_integration.rs`

## Carry Forward

- only activate extensionless aliases through the already-resolved file hint
- preserve basename-derived and exact-path `:line` behavior
- avoid broadening this slice into generic module-name parsing
