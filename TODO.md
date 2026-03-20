# TODO — RTK-Style Adoption Follow-Up

Date: 2026-03-20
**Status: RESOLVED** — All items addressed in v1.6 RTK milestone (shipped 2026-03-20)

## Resolution Summary

| Original Task | Resolution |
|---------------|------------|
| 1. Reproduce runtime lifecycle | Verified in d13e76b — daemon fallback, sidecar port discovery working |
| 2. Missing sidecar files — bug or expected? | Expected — hooks fail-open by design, diagnostics now explain why |
| 3. Improve missing-sidecar diagnostics | HOOK-01/02/03 shipped — verbose mode, port-missing vs stale, one-time hint |
| 4. Evaluate daemon fallback | Shipped in d13e76b — `DaemonFallback` outcome with 500ms deadline |
| 5. Separate Codex ceiling from SymForge gaps | DOCS-01 shipped — `docs/codex-integration-ceiling.md` (297 lines) |
| 6. Add regression tests | 15+ new tests across symbol disambiguation and hook diagnostics |

## Additional Fixes (from reviewer bugs)

| Bug | Resolution |
|-----|------------|
| validate_file_syntax broken | Fixed in d13e76b — dispatch arm in daemon.rs |
| search_text regex misses | Fixed in d13e76b — noise policy relaxed for regex mode |
| ranked=true over-filtering | Fixed in d13e76b — total_limit boosted to 200 for ranked |
| max_tokens budget ignored | Fixed in d13e76b — enforce_token_budget in all modes |
| C# class/constructor ambiguity | Fixed in v1.6 RTK — kind_disambiguation_tier with 4-tier priority |
| Callee flooding | Fixed in d13e76b — deduplication with occurrence counts |
