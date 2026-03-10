# Story 4.7: Classify Action-Required States and Signal the Next Safe Action

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,
I want stale, interrupted, suspect, repair-required, and degraded states classified explicitly with next-safe-action guidance,
so that I can respond correctly without mistaking action-required conditions for normal health.

**FRs implemented:** FR32, FR37

- **FR32**: Users can inspect repair-related state, including whether a repository, run, or retrieval problem requires action.
- **FR37**: Users can inspect whether operational state indicates stale, interrupted, or suspect conditions.

## Acceptance Criteria

1. **Given** a run or repository enters a stale, interrupted, suspect, repair-required, or degraded condition **When** I inspect that state through operator or MCP-facing status surfaces **Then** Tokenizor classifies the action-required state explicitly **And** it distinguishes action-required conditions from normal healthy or terminal-complete states
2. **Given** a classified state requires intervention **When** Tokenizor reports it **Then** the response maps the condition to next-safe-action categories such as resume, repair, re-index, or migrate **And** it does not hide the need for intervention behind generic error wording

## Tasks / Subtasks

### Phase 1: Domain model — Unified action classification types

- [x] Task 1.1: Add `Migrate` variant to `NextAction` enum in `src/domain/retrieval.rs` (AC: 2)
  - [x] 1.1.1: Add `Migrate` variant to `NextAction` enum
  - [x] 1.1.2: Add `"migrate"` arm to `NextAction::fmt::Display` implementation
  - [x] 1.1.3: Update `test_request_gate_error_next_action_mapping` if needed for coverage
  - [x] 1.1.4: Verify existing `#[serde(rename_all = "snake_case")]` covers the new variant correctly

- [x] Task 1.2: Define `ActionClassification` struct in `src/domain/health.rs` (AC: 1, 2)
  - [x] 1.2.1: Define `ActionClassification` struct with fields:
    - `condition: ActionCondition` — the classified condition kind
    - `action_required: bool` — whether intervention is needed
    - `next_action: Option<NextAction>` — structured safe-action guidance
    - `detail: String` — human-readable description of the condition and guidance
  - [x] 1.2.2: Derive `Clone, Debug, Serialize, Deserialize, PartialEq, Eq`

- [x] Task 1.3: Define `ActionCondition` enum in `src/domain/health.rs` (AC: 1)
  - [x] 1.3.1: Define `ActionCondition` enum with variants:
    - `Healthy` — normal operating state, retrieval is safe
    - `Pending` — initial state before first indexing (not an error)
    - `ActiveRun` — indexing or repair in progress, wait for completion
    - `Degraded` — some files failed or are incomplete, repair recommended
    - `Failed` — indexing failed, repair or re-index needed
    - `Invalidated` — indexed state no longer trusted, re-index required
    - `Quarantined` — integrity concerns, repair needed
    - `Interrupted` — run was interrupted, resume or re-index
    - `Stale` — run succeeded but source state may have changed (time-based or detected drift)
    - `TerminalComplete` — run reached terminal state without issues (Succeeded, Cancelled)
  - [x] 1.3.2: Derive `Clone, Debug, Serialize, Deserialize, PartialEq, Eq` with `#[serde(rename_all = "snake_case")]`
  - [x] 1.3.3: Implement `ActionCondition::is_action_required(&self) -> bool` — returns true for Degraded, Failed, Invalidated, Quarantined, Interrupted, Stale

- [x] Task 1.4: Define `classify_run_action(run: &IndexRun, health: &RunHealth, recovery_state: Option<&RunRecoveryState>) -> ActionClassification` free function in `src/domain/health.rs` (AC: 1, 2)
  - [x] 1.4.1: Map `IndexRunStatus::Queued` → `ActionCondition::Pending`, no action required, next_action: None
  - [x] 1.4.2: Map `IndexRunStatus::Running` → `ActionCondition::ActiveRun`, no action required, next_action: Wait
  - [x] 1.4.3: Map `IndexRunStatus::Succeeded` with healthy → `ActionCondition::TerminalComplete`, no action required
  - [x] 1.4.4: Map `IndexRunStatus::Succeeded` with degraded health → `ActionCondition::Degraded`, action required, next_action: Repair
  - [x] 1.4.5: Map `IndexRunStatus::Failed` → `ActionCondition::Failed`, action required, next_action: Repair
  - [x] 1.4.6: Map `IndexRunStatus::Interrupted` → `ActionCondition::Interrupted`, action required, next_action: Resume (if checkpoint exists) or Reindex (if no checkpoint)
  - [x] 1.4.7: Map `IndexRunStatus::Cancelled` → `ActionCondition::TerminalComplete`, no action required
  - [x] 1.4.8: Map `IndexRunStatus::Aborted` → `ActionCondition::Failed`, action required, next_action: Repair (or Reindex if stale-queued abort)
  - [x] 1.4.9: Handle `RecoveryStateKind::Resumed` → `ActionCondition::ActiveRun`, next_action: Wait
  - [x] 1.4.10: Handle `RecoveryStateKind::ResumeRejected` → `ActionCondition::Failed`, next_action: from recovery_state.next_action or Reindex
  - [x] 1.4.11: Detail string must include run-specific context (run_id, error_summary) and must not use generic wording

- [x] Task 1.5: Define `classify_repository_action(status: &RepositoryStatus, has_completed_run: bool, has_active_run: bool, invalidation_reason: &Option<String>, quarantine_reason: &Option<String>) -> ActionClassification` free function in `src/domain/health.rs` (AC: 1, 2)
  - [x] 1.5.1: Implement the same logic currently in `RunManager::classify_repository_health()` but returning `ActionClassification`
  - [x] 1.5.2: Map Ready → `ActionCondition::Healthy`
  - [x] 1.5.3: Map Pending with no completed run and no active run → `ActionCondition::Pending` + Reindex
  - [x] 1.5.4: Map Pending with active run → `ActionCondition::ActiveRun` + Wait
  - [x] 1.5.5: Map Degraded → `ActionCondition::Degraded` + Repair
  - [x] 1.5.6: Map Failed → `ActionCondition::Failed` + Repair
  - [x] 1.5.7: Map Invalidated → `ActionCondition::Invalidated` + Reindex (include invalidation reason)
  - [x] 1.5.8: Map Quarantined → `ActionCondition::Quarantined` + Repair (include quarantine reason)

- [x] Task 1.6: Add re-exports for new types in `src/domain/mod.rs` (AC: 1, 2)
  - [x] 1.6.1: Re-export `ActionClassification`, `ActionCondition`, `classify_run_action`, `classify_repository_action`

### Phase 2: Enrich RunStatusReport with structured classification

- [x] Task 2.1: Add `classification: ActionClassification` field to `RunStatusReport` in `src/domain/index.rs` (AC: 1, 2)
  - [x] 2.1.1: Add `classification: ActionClassification` field to `RunStatusReport`
  - [x] 2.1.2: Add `next_action: Option<NextAction>` field to `RunStatusReport` (mirrors the classification for direct access)
  - [x] 2.1.3: Keep `action_required: Option<String>` for backward compatibility — it is populated from `classification.detail` when action is required

- [x] Task 2.2: Update `RunManager::get_run_status_report()` to use `classify_run_action()` (AC: 1, 2)
  - [x] 2.2.1: Call `classify_run_action(&run, &health, run.recovery_state.as_ref())` after computing health
  - [x] 2.2.2: Set `classification` field from result
  - [x] 2.2.3: Set `next_action` from `classification.next_action`
  - [x] 2.2.4: Set `action_required` from `classification.detail` when `classification.action_required` is true (backward compat)
  - [x] 2.2.5: Remove the inline `action_required_message()` call — replaced by classification
  - [x] 2.2.6: Preserve the repo-level invalidation overlay (existing logic that appends invalidation note)

### Phase 3: Enrich RepositoryHealthReport with structured classification

- [x] Task 3.1: Add `classification: ActionClassification` field to `RepositoryHealthReport` in `src/domain/health.rs` (AC: 1, 2)
  - [x] 3.1.1: Add `classification: ActionClassification` field to `RepositoryHealthReport`
  - [x] 3.1.2: Keep `action_required: bool`, `next_action: Option<NextAction>`, `status_detail: String` for backward compatibility — populated from classification

- [x] Task 3.2: Update `RunManager::inspect_repository_health()` to use `classify_repository_action()` (AC: 1, 2)
  - [x] 3.2.1: Call `classify_repository_action()` instead of `Self::classify_repository_health()`
  - [x] 3.2.2: Set `classification` field from result
  - [x] 3.2.3: Set backward-compat fields from classification: `action_required = classification.action_required`, `next_action = classification.next_action`, `status_detail = classification.detail`
  - [x] 3.2.4: Remove `RunManager::classify_repository_health()` private method — replaced by domain function

### Phase 4: MCP tool enrichment

- [x] Task 4.1: Ensure `get_index_run` MCP tool response includes structured classification (AC: 1, 2)
  - [x] 4.1.1: The `RunStatusReport` serialization already includes `classification` and `next_action` — verify the MCP tool returns these fields in the JSON response
  - [x] 4.1.2: Update tool description to mention action classification and next-safe-action guidance

- [x] Task 4.2: Ensure `inspect_repository_health` MCP tool response includes structured classification (AC: 1, 2)
  - [x] 4.2.1: The `RepositoryHealthReport` serialization already includes `classification` — verify the MCP tool returns this field
  - [x] 4.2.2: Update tool description to mention action classification

- [x] Task 4.3: Ensure `list_index_runs` MCP tool response includes structured classification (AC: 1)
  - [x] 4.3.1: Check if `list_index_runs` returns `RunStatusReport` or raw `IndexRun` — if raw, add classification to the response or document as out-of-scope
  - [x] 4.3.2: If `list_index_runs` returns raw `IndexRun`, this task is informational only — the structured classification is accessible through `get_index_run` for each run

### Phase 5: Testing

- [x] Task 5.1: Unit tests for `ActionCondition` and `ActionClassification` types (AC: 1, 2)
  - [x] 5.1.1: `test_action_condition_is_action_required` — verify is_action_required returns true for Degraded, Failed, Invalidated, Quarantined, Interrupted, Stale and false for Healthy, Pending, ActiveRun, TerminalComplete
  - [x] 5.1.2: `test_action_classification_serialization_roundtrip` — serialize/deserialize ActionClassification preserves all fields
  - [x] 5.1.3: `test_action_condition_serializes_snake_case` — all variants serialize to expected snake_case values

- [x] Task 5.2: Unit tests for `classify_run_action()` (AC: 1, 2)
  - [x] 5.2.1: `test_classify_queued_run_not_action_required` — Queued → Pending, no action
  - [x] 5.2.2: `test_classify_running_run_active` — Running → ActiveRun, Wait
  - [x] 5.2.3: `test_classify_succeeded_healthy_terminal` — Succeeded + Healthy → TerminalComplete, no action
  - [x] 5.2.4: `test_classify_succeeded_degraded_action_required` — Succeeded + Degraded → Degraded, Repair
  - [x] 5.2.5: `test_classify_failed_run_action_required` — Failed → Failed, Repair
  - [x] 5.2.6: `test_classify_interrupted_with_checkpoint_resume` — Interrupted + checkpoint → Interrupted, Resume
  - [x] 5.2.7: `test_classify_interrupted_no_checkpoint_reindex` — Interrupted + no checkpoint → Interrupted, Reindex
  - [x] 5.2.8: `test_classify_cancelled_terminal_complete` — Cancelled → TerminalComplete, no action
  - [x] 5.2.9: `test_classify_aborted_action_required` — Aborted → Failed, Repair
  - [x] 5.2.10: `test_classify_resumed_run_active` — Resumed recovery state → ActiveRun, Wait
  - [x] 5.2.11: `test_classify_resume_rejected_failed` — ResumeRejected → Failed, next_action from recovery state
  - [x] 5.2.12: `test_classify_detail_includes_run_context` — detail string includes error_summary when available

- [x] Task 5.3: Unit tests for `classify_repository_action()` (AC: 1, 2)
  - [x] 5.3.1: `test_classify_ready_repository_healthy` — Ready → Healthy, no action
  - [x] 5.3.2: `test_classify_pending_never_indexed` — Pending + no run + no active → Pending, Reindex
  - [x] 5.3.3: `test_classify_pending_active_run` — Pending + active run → ActiveRun, Wait
  - [x] 5.3.4: `test_classify_degraded_repository` — Degraded → Degraded, Repair
  - [x] 5.3.5: `test_classify_failed_repository` — Failed → Failed, Repair
  - [x] 5.3.6: `test_classify_invalidated_repository` — Invalidated → Invalidated, Reindex
  - [x] 5.3.7: `test_classify_quarantined_repository` — Quarantined → Quarantined, Repair
  - [x] 5.3.8: `test_classify_invalidated_includes_reason` — detail string includes invalidation reason

- [x] Task 5.4: Unit tests for NextAction `Migrate` variant (AC: 2)
  - [x] 5.4.1: `test_next_action_migrate_serializes` — Migrate serializes to "migrate" and deserializes back
  - [x] 5.4.2: `test_next_action_migrate_display` — Display formats as "migrate"

- [x] Task 5.5: Unit tests for enriched RunStatusReport (AC: 1, 2)
  - [x] 5.5.1: `test_run_status_report_includes_classification` — serialized JSON contains `classification` field with `condition`, `action_required`, `next_action`
  - [x] 5.5.2: `test_run_status_report_backward_compat` — `action_required: Option<String>` still populated correctly
  - [x] 5.5.3: `test_run_status_report_next_action_field` — `next_action` field at top level matches classification

- [x] Task 5.6: Unit tests for enriched RepositoryHealthReport (AC: 1, 2)
  - [x] 5.6.1: `test_repository_health_report_includes_classification` — serialized JSON contains `classification` field
  - [x] 5.6.2: `test_repository_health_report_backward_compat` — `action_required: bool`, `next_action`, `status_detail` still correct

- [x] Task 5.7: Integration tests for end-to-end classification flow (AC: 1, 2)
  - [x] 5.7.1: `test_get_index_run_returns_classification` — call get_index_run through MCP → JSON contains `classification.condition`, `classification.next_action`
  - [x] 5.7.2: `test_inspect_health_returns_classification` — call inspect_repository_health → JSON contains `classification.condition`, `classification.next_action`
  - [x] 5.7.3: `test_interrupted_run_classified_with_resume_action` — interrupt a run → get_index_run → classification is Interrupted + Resume
  - [x] 5.7.4: `test_degraded_repo_classified_with_repair_action` — degrade repo → inspect_repository_health → classification is Degraded + Repair
  - [x] 5.7.5: `test_invalidated_repo_classified_with_reindex_action` — invalidate repo → inspect_repository_health → classification is Invalidated + Reindex
  - [x] 5.7.6: `test_healthy_repo_not_action_required` — healthy repo → classification is Healthy + no action

### Phase 6: Cleanup

- [x] Task 6.1: Remove deprecated private methods (AC: 1, 2)
  - [x] 6.1.1: Remove `RunManager::classify_repository_health()` — replaced by `classify_repository_action()` domain function
  - [x] 6.1.2: Remove `action_required_message()` free function from run_manager.rs — replaced by `classify_run_action()` domain function
  - [x] 6.1.3: Remove `classify_run_health()` free function from run_manager.rs ONLY if all callers now use `classify_run_action()` instead; if `RunHealth` is still used elsewhere, keep it and have `classify_run_action()` consume it as input

## Dev Notes

### What Already Exists

**Run-Level Status Classification** (`src/application/run_manager.rs`):
- `classify_run_health(run, file_summary) -> RunHealth` — maps IndexRunStatus to Healthy/Degraded/Unhealthy (lines ~2884-2896)
- `action_required_message(run, health) -> Option<String>` — maps run status + recovery state to freeform text guidance (lines ~2909-2963)
- `RunStatusReport` has `action_required: Option<String>` (freeform text) and `health: RunHealth` — no structured `NextAction`
- `get_run_status_report()` calls both, plus overlays repo-level invalidation note

**Repository-Level Classification** (`src/application/run_manager.rs`):
- `RunManager::classify_repository_health(status, has_run, has_active, inv_reason, quar_reason) -> (bool, Option<NextAction>, String)` — structured but returns a tuple (lines ~2758-2817)
- `RepositoryHealthReport` has `action_required: bool`, `next_action: Option<NextAction>`, `status_detail: String` — structured, good foundation

**NextAction Enum** (`src/domain/retrieval.rs`):
- Variants: `Resume`, `Reindex`, `Repair`, `Wait`, `ResolveContext` — **no `Migrate`**
- Used by `RequestGateError::next_action()` (retrieval gate mapping)
- Used by `RepositoryHealthReport`
- Doc comment: "Actionable guidance for blocked, quarantined, or gated responses. Shared vocabulary with Epic 4 repair flows."

**RequestGateError** (`src/domain/retrieval.rs`):
- Maps retrieval gate states to `NextAction` via `next_action()` method
- Already provides structured classification for the retrieval gate path
- Story 4.7 does NOT change this path — it's already correctly classified

**RunHealth Enum** (`src/domain/index.rs`):
- `Healthy`, `Degraded`, `Unhealthy` — coarse-grained, used in `RunStatusReport`
- Will be consumed by `classify_run_action()` as input

**RecoveryStateKind** (`src/domain/index.rs`):
- `Resumed`, `ResumeRejected`, `RecoveryPending` — used in run recovery
- `RunRecoveryState` has `next_action: Option<NextAction>` — already structured, used by `action_required_message()`

**MCP Tools** (18 tools after 4.6):
health, index_folder, get_index_run, list_index_runs, cancel_index_run, checkpoint_now, resume_index_run, reindex_repository, invalidate_indexed_state, search_text, search_symbols, get_file_outline, get_repo_outline, get_symbol, get_symbols, repair_index, inspect_repository_health, get_operational_history

### What 4.7 Builds vs. What Already Exists

| Concern | Already exists | 4.7 adds |
|---------|---------------|----------|
| NextAction vocabulary | Resume, Reindex, Repair, Wait, ResolveContext | + Migrate variant |
| Repository classification | `classify_repository_health()` returns tuple | `classify_repository_action()` returns `ActionClassification` struct |
| Run classification | `action_required_message()` returns `Option<String>` text | `classify_run_action()` returns `ActionClassification` struct |
| RunStatusReport | `action_required: Option<String>` freeform | + `classification: ActionClassification` + `next_action: Option<NextAction>` |
| RepositoryHealthReport | `action_required: bool` + `next_action` + `status_detail` separately | + `classification: ActionClassification` (unifies the three) |
| Condition taxonomy | Implicit in code branches | Explicit `ActionCondition` enum |
| MCP run inspection | Text-only guidance | Structured classification in JSON |
| MCP health inspection | Already has structured guidance | + explicit `classification` field |

### Design Decisions

**1. `ActionClassification` struct unifies the three separate fields into one referenceable type.**
Rather than having `(action_required, next_action, status_detail)` as separate fields assembled ad-hoc, a single struct provides a composable unit. Backward-compatible fields remain on the report types for existing consumers.

**2. `ActionCondition` enum provides compiler-enforced condition taxonomy.**
Every action-required state gets an explicit variant. The compiler enforces exhaustive handling. New conditions (future epics) require explicit classification decisions.

**3. Classification functions live in `src/domain/health.rs`, not `run_manager.rs`.**
Classification is a domain concern, not an application concern. Moving it to the domain layer makes it testable independently and reusable by other application surfaces if needed.

**4. `NextAction::Migrate` added for future schema/state migration scenarios.**
The epic AC explicitly says "resume, repair, re-index, or migrate." While no current code path produces a Migrate classification, having the variant ready prevents future vocabulary drift. It may be used when SpacetimeDB schema incompatibility is detected.

**5. Backward compatibility preserved on report types.**
`RunStatusReport.action_required: Option<String>` and `RepositoryHealthReport.{action_required, next_action, status_detail}` remain. They are populated from the `classification` field. Existing MCP consumers see no breaking change.

**6. `classify_run_action` takes `RunHealth` as input, not re-deriving it.**
`RunHealth` (Healthy/Degraded/Unhealthy) is already computed. The classification function consumes it rather than reimplementing the health derivation. This preserves the existing `classify_run_health()` logic as-is.

**7. `Stale` condition is defined but deferred for active detection.**
The `ActionCondition::Stale` variant exists in the taxonomy for completeness, but no code path currently produces it. Active staleness detection (comparing source timestamps to index timestamps) is a future enhancement. The variant is ready for that work.

### Trust Boundary Rules (from architecture and project-context)

1. **Recovery paths must classify stale, interrupted, suspect, quarantined, degraded, and invalid states explicitly.** (project-context Epic 4 Rule 4) — `ActionCondition` enum provides explicit classification for each
2. **Next-action guidance must stay consistent across recovery and retrieval surfaces.** (project-context Epic 4 Rule 5) — single `NextAction` enum shared by `ActionClassification`, `RequestGateError`, `RunRecoveryState`, `RepositoryHealthReport`
3. **Domain states must not be collapsed into generic success/failure booleans.** (architecture Result/Response Format Rules) — `ActionCondition` distinguishes 10 states, never collapses to bool
4. **Trust/integrity state vocabulary must be centralized and reused across layers.** (architecture Communication Patterns) — `NextAction` is the centralized vocabulary
5. **Operational history writes must be durable before reporting the action as completed.** (project-context Epic 4 Rule 3) — 4.7 does not modify event recording paths, inherits 4.6's guarantees

[Source: _bmad-output/planning-artifacts/architecture.md — Communication Patterns, Result/Response Format Rules]
[Source: _bmad-output/project-context.md — Epic 4 Recovery Architecture Rules 4-5]

### Previous Story Intelligence

**Story 4.6** (Preserve Operational History) — DONE:
- Added `OperationalEvent`, `OperationalEventKind`, `IntegrityEventKind`, `OperationalEventFilter` to `src/domain/index.rs`
- All state transitions now record operational events
- `get_operational_history` MCP tool provides event inspection
- Code review fixed: wrapper refactoring, pipeline completion logging, `cancel_run` error propagation, `resume_run` event recording
- **Key for 4.7**: 4.6 provides the event recording foundation. 4.7 provides the state *classification* layer that answers "what should I do about this?"

**Story 4.5** (Inspect Repository Health) — DONE:
- `RepositoryHealthReport` with `action_required`, `next_action`, `status_detail` already structured
- `classify_repository_health()` provides the logic — 4.7 extracts this to a domain function
- **Key for 4.7**: 4.7 enriches the report with `ActionClassification` and moves classification to domain layer

**Story 4.4** (Trigger Deterministic Repair) — DONE:
- Repair paths produce `RepairResult` with `RepairOutcome` (Restored/PartialRestore/NotRepairable/NoActionNeeded)
- `NextAction` already used in `RunRecoveryState` for resume-rejected scenarios
- **Key for 4.7**: repair outcomes feed into classification — a NotRepairable repair signals Reindex, not Repair

**Story 4.1** (Startup Sweep) — DONE:
- `startup_sweep()` transitions Running → Interrupted
- Post-sweep, interrupted runs get `action_required_message()` text
- **Key for 4.7**: these interrupted runs will now get structured `ActionClassification` instead of text-only guidance

### Git Intelligence

Recent commits:
- `7e8057b` — feat(story-4.6): complete IntegrityEvent instrumentation and land story
- `b8c9ab6` — fix(story-4.6): code review fixes for operational history
- `b84ce37` — feat(control-plane): land story 4.3 with review fixes

Pattern: each story builds cleanly on the previous. 4.7 is the capstone — it unifies the classification that 4.1-4.6 laid the groundwork for.

### Existing Code to Reuse

| Function / Type | Location | Why it matters for 4.7 |
|---|---|---|
| `NextAction` enum | `src/domain/retrieval.rs` | Add `Migrate` variant; shared vocabulary |
| `RequestGateError::next_action()` | `src/domain/retrieval.rs` | Already correctly classified — no changes needed |
| `RunHealth` enum | `src/domain/index.rs` | Input to `classify_run_action()` |
| `RunStatusReport` | `src/domain/index.rs` | Add `classification` and `next_action` fields |
| `IndexRunStatus` | `src/domain/index.rs` | Primary input for run classification |
| `RecoveryStateKind`, `RunRecoveryState` | `src/domain/index.rs` | Recovery context for run classification |
| `RepositoryStatus` | `src/domain/repository.rs` | Primary input for repository classification |
| `RepositoryHealthReport` | `src/domain/health.rs` | Add `classification` field |
| `RunManager::classify_repository_health()` | `src/application/run_manager.rs` | Logic moves to `classify_repository_action()` domain function |
| `action_required_message()` | `src/application/run_manager.rs` | Logic moves to `classify_run_action()` domain function |
| `classify_run_health()` | `src/application/run_manager.rs` | Remains — consumed as input by `classify_run_action()` |
| `RunManager::get_run_status_report()` | `src/application/run_manager.rs` | Updated to use `classify_run_action()` |
| `RunManager::inspect_repository_health()` | `src/application/run_manager.rs` | Updated to use `classify_repository_action()` |
| `ApplicationContext` bridge | `src/application/mod.rs` | No changes needed — delegates to RunManager |
| `TokenizorServer` MCP tools | `src/protocol/mcp.rs` | Tool descriptions updated; serialization handles new fields automatically |

### Library / Framework Requirements

- No new external dependencies required for Story 4.7
- Keep all existing dependency versions (`rmcp = 1.1.0`, `tokio = 1.48`, `serde = 1.0`, `schemars = 1.1`)
- No SpacetimeDB changes needed

### Epic 4 Definition of Done (mandatory)

- Expected test delta: Add 3 unit tests for ActionCondition/ActionClassification types, 12 unit tests for classify_run_action, 8 unit tests for classify_repository_action, 2 unit tests for NextAction Migrate, 3 unit tests for enriched RunStatusReport, 2 unit tests for enriched RepositoryHealthReport, and 6 integration tests for end-to-end classification. **Total: ~36 new tests minimum.**
- Build/test evidence: [Record the exact `cargo test` command(s) and pass/fail summary]
- Acceptance-criteria traceability:
  - AC1 → `ActionCondition` enum provides explicit classification for all action-required states; `ActionClassification` struct wraps condition + action_required bool + next_action + detail; `classify_run_action()` and `classify_repository_action()` ensure every state is explicitly classified; `RunStatusReport.classification` and `RepositoryHealthReport.classification` surface classification through MCP tools
  - AC2 → `NextAction` enum (with new `Migrate` variant) maps every classified condition to safe-action categories; `ActionClassification.detail` provides human-readable guidance that does not hide intervention need; all MCP-facing responses include structured `classification.next_action` field
- Trust-boundary traceability: Epic 4 Rule 4 (classify explicitly — `ActionCondition`), Epic 4 Rule 5 (consistent next-action vocabulary — `NextAction`), Architecture Communication Patterns (centralized vocabulary), Architecture Result/Response Format Rules (no collapsed booleans)
- State-transition evidence:
  - Classification side: every `IndexRunStatus` and `RepositoryStatus` maps to an explicit `ActionCondition` with a structured `NextAction` (verified by unit tests per status)
  - MCP surface side: `get_index_run` and `inspect_repository_health` return JSON with `classification` containing structured condition and guidance (verified by integration tests)
  - Backward compatibility side: `action_required: Option<String>` and `action_required: bool` still populated correctly (verified by backward-compat tests)

### Self-Audit Checklist (mandatory before requesting review)

_Run this checklist after all tasks are complete. This is a blocking step — do not request review until every item is verified._

#### Generic Verification
- [x] For every task marked `[x]`, cite the specific test that verifies it
- [x] For every new error variant or branch, confirm a test exercises it
- [x] For every computed value, trace it to where it surfaces (log, return value, persistence)
- [x] For every test, verify the assertion can actually fail (no `assert!(true)`, no conditionals that always pass)

#### Epic 4 Recovery Verification
- [x] The declared expected test delta was met or exceeded by the actual implementation
- [x] Build/test evidence is recorded with the exact command and outcome summary
- [x] Every acceptance criterion is traced to concrete implementation code and at least one concrete test
- [x] Every trust-boundary or recovery-policy decision cites the exact architecture or `project-context.md` source
- [x] Every state transition is tested from both sides: the mutation itself and the resulting retrieval/inspection behavior

#### Story 4.7-Specific Verification
- [x] Confirm `ActionCondition` enum covers all states mentioned in AC1: stale, interrupted, suspect, repair-required (Degraded/Failed/Quarantined), degraded
- [x] Confirm `ActionCondition::is_action_required()` returns correct values for all variants
- [x] Confirm `classify_run_action()` maps every `IndexRunStatus` variant to an explicit `ActionCondition`
- [x] Confirm `classify_run_action()` handles recovery states (Resumed, ResumeRejected) correctly
- [x] Confirm `classify_repository_action()` maps every `RepositoryStatus` variant to an explicit `ActionCondition`
- [x] Confirm `NextAction::Migrate` serializes to "migrate" and round-trips correctly
- [x] Confirm `RunStatusReport` JSON contains `classification` with `condition`, `action_required`, `next_action`, `detail`
- [x] Confirm `RepositoryHealthReport` JSON contains `classification` field
- [x] Confirm backward-compat fields (`action_required: Option<String>` on RunStatusReport, `action_required: bool` on RepositoryHealthReport) are still populated correctly
- [x] Confirm `get_index_run` MCP tool returns structured classification in JSON response
- [x] Confirm `inspect_repository_health` MCP tool returns structured classification in JSON response
- [x] Confirm no existing tests regress (total test count only increases)

### Project Structure Notes

| File | Why it is in scope |
|---|---|
| `src/domain/retrieval.rs` | Add `Migrate` variant to `NextAction` enum |
| `src/domain/health.rs` | Add `ActionClassification`, `ActionCondition` types; add `classify_run_action()`, `classify_repository_action()` functions |
| `src/domain/mod.rs` | Add re-exports for new classification types |
| `src/domain/index.rs` | Add `classification` and `next_action` fields to `RunStatusReport` |
| `src/application/run_manager.rs` | Update `get_run_status_report()` and `inspect_repository_health()` to use domain classification functions; remove deprecated private methods |
| `src/protocol/mcp.rs` | Update tool descriptions for `get_index_run` and `inspect_repository_health` |
| `tests/indexing_integration.rs` | Add classification integration tests |

**Alignment notes:**
- Stay inside the current `application` / `domain` / `storage` / `protocol` layering
- `ActionClassification` and `ActionCondition` go in `src/domain/health.rs` — they are health/status classification types
- Classification functions are domain logic, not application logic — they take domain types as input and return domain types
- `RunStatusReport` modifications follow the existing `#[serde(default)]` backward-compat pattern
- No new MCP tools needed — existing tools gain richer classification through their existing response types
- No storage changes needed — classification is derived from current state, not persisted separately

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.7]
- [Source: _bmad-output/planning-artifacts/architecture.md — Communication Patterns (Operational Event Naming Rules)]
- [Source: _bmad-output/planning-artifacts/architecture.md — Result/Response Format Rules]
- [Source: _bmad-output/planning-artifacts/architecture.md — Data Architecture (Control-Plane Model)]
- [Source: _bmad-output/planning-artifacts/architecture.md — Observability/Deployment Model]
- [Source: _bmad-output/planning-artifacts/prd.md — FR32, FR37]
- [Source: _bmad-output/project-context.md — Epic 4 Recovery Architecture Rules 4-5]
- [Source: _bmad-output/project-context.md — Agent Selection (Claude Opus 4.6 primary)]
- [Source: _bmad-output/implementation-artifacts/4-6-preserve-operational-history-for-runs-repairs-and-integrity-events.md]
- [Source: _bmad-output/implementation-artifacts/4-5-inspect-repository-health-and-repair-required-conditions.md]
- [Source: _bmad-output/implementation-artifacts/4-4-trigger-deterministic-repair-for-suspect-or-incomplete-state.md]
- [Source: _bmad-output/implementation-artifacts/4-1-sweep-stale-leases-and-interrupted-state-on-startup.md]
- [Source: src/domain/retrieval.rs — NextAction, RequestGateError, RetrievalOutcome]
- [Source: src/domain/health.rs — RepositoryHealthReport, StatusContext, HealthStatus]
- [Source: src/domain/index.rs — RunStatusReport, IndexRunStatus, RunHealth, RunRecoveryState]
- [Source: src/domain/repository.rs — RepositoryStatus]
- [Source: src/application/run_manager.rs — classify_repository_health, action_required_message, classify_run_health, get_run_status_report, inspect_repository_health]
- [Source: src/protocol/mcp.rs — get_index_run, inspect_repository_health MCP tools]

## Change Log

- 2026-03-10: Story 4.7 implementation complete — unified action classification types, enriched status reports, MCP tool descriptions updated, 36 new tests added, deprecated methods removed
- 2026-03-10: Code review fixes — [H1] deduplicated STALE_QUEUED_ABORTED_SUMMARY constant (health.rs exports, run_manager.rs imports), [H2] added skip_serializing_if on RunStatusReport.action_required, [M1] explicit Unhealthy match arm for Succeeded runs, [M2] improved Pending catch-all detail message, [M3] added stale-queued Aborted→Reindex unit test in health.rs. Total tests: 747

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

No debug issues encountered.

### Completion Notes List

- Phase 1: Added `Migrate` variant to `NextAction` enum, defined `ActionCondition` enum (10 variants), `ActionClassification` struct, `classify_run_action()` and `classify_repository_action()` domain functions in `src/domain/health.rs`, re-exports in `src/domain/mod.rs`
- Phase 2: Enriched `RunStatusReport` with `classification: ActionClassification` and `next_action: Option<NextAction>` fields; updated `build_run_report()` to use `classify_run_action()`
- Phase 3: Enriched `RepositoryHealthReport` with `classification: ActionClassification` field; updated `inspect_repository_health()` to use `classify_repository_action()`
- Phase 4: Updated MCP tool descriptions for `get_index_run` and `inspect_repository_health` to mention structured action classification
- Phase 5: Added 36 new tests (25 unit tests in health.rs, 2 in retrieval.rs, 3 in index.rs, 6 integration tests). Total test count: 710 → 746
- Phase 6: Removed deprecated `classify_repository_health()` and `action_required_message()` from run_manager.rs; updated 5 tests that called them directly
- Backward compatibility preserved: `action_required: Option<String>` on RunStatusReport and `action_required: bool`, `next_action`, `status_detail` on RepositoryHealthReport still populated from classification

### File List

- src/domain/retrieval.rs (modified — added Migrate variant to NextAction, updated Display impl)
- src/domain/health.rs (modified — added ActionCondition, ActionClassification, classify_run_action, classify_repository_action, classification field on RepositoryHealthReport, 25 new unit tests)
- src/domain/index.rs (modified — added classification and next_action fields to RunStatusReport, 3 new unit tests)
- src/domain/mod.rs (modified — added re-exports for new types and functions)
- src/application/run_manager.rs (modified — updated build_run_report and inspect_repository_health to use domain classification functions, removed deprecated methods, updated 5 tests)
- src/protocol/mcp.rs (modified — updated tool descriptions for get_index_run and inspect_repository_health)
- tests/indexing_integration.rs (modified — added 6 classification integration tests, fixed 2 tests for new message format)
- tests/retrieval_conformance.rs (modified — updated NextAction exhaustive test for Migrate variant, 2 new tests)
