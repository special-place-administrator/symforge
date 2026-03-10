use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::index::{
    IndexRun, IndexRunMode, IndexRunStatus, RecoveryStateKind, RepairEvent, RunHealth,
    RunRecoveryState,
};
use super::repository::RepositoryStatus;
use super::retrieval::NextAction;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ok,
    Degraded,
    Unavailable,
}

impl HealthStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ok)
    }

    fn severity(&self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::Degraded => 1,
            Self::Unavailable => 2,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthIssueCategory {
    Bootstrap,
    Dependency,
    Configuration,
    Compatibility,
    Storage,
    Recovery,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthSeverity {
    Info,
    Warning,
    Error,
}

impl HealthSeverity {
    pub fn blocks_readiness(&self) -> bool {
        matches!(self, Self::Error)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentHealth {
    pub name: String,
    pub category: HealthIssueCategory,
    pub status: HealthStatus,
    pub severity: HealthSeverity,
    pub detail: String,
    pub remediation: Option<String>,
    pub observed_at_unix_ms: u64,
}

impl ComponentHealth {
    pub fn ok(
        name: impl Into<String>,
        category: HealthIssueCategory,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(
            name,
            category,
            HealthStatus::Ok,
            HealthSeverity::Info,
            detail,
            None::<String>,
        )
    }

    pub fn warning(
        name: impl Into<String>,
        category: HealthIssueCategory,
        detail: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self::new(
            name,
            category,
            HealthStatus::Degraded,
            HealthSeverity::Warning,
            detail,
            Some(remediation.into()),
        )
    }

    pub fn error(
        name: impl Into<String>,
        category: HealthIssueCategory,
        detail: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self::new(
            name,
            category,
            HealthStatus::Unavailable,
            HealthSeverity::Error,
            detail,
            Some(remediation.into()),
        )
    }

    pub fn blocks_readiness(&self) -> bool {
        !self.status.is_ready() && self.severity.blocks_readiness()
    }

    fn new(
        name: impl Into<String>,
        category: HealthIssueCategory,
        status: HealthStatus,
        severity: HealthSeverity,
        detail: impl Into<String>,
        remediation: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            category,
            status,
            severity,
            detail: detail.into(),
            remediation,
            observed_at_unix_ms: unix_timestamp_ms(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceIdentity {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthReport {
    pub checked_at_unix_ms: u64,
    pub service: ServiceIdentity,
    pub overall_status: HealthStatus,
    pub components: Vec<ComponentHealth>,
}

impl HealthReport {
    pub fn new(service: ServiceIdentity, components: Vec<ComponentHealth>) -> Self {
        Self {
            checked_at_unix_ms: unix_timestamp_ms(),
            overall_status: aggregate_status(&components),
            service,
            components,
        }
    }

    pub fn summary(&self) -> String {
        let failing = self
            .components
            .iter()
            .filter(|component| !component.status.is_ready())
            .map(|component| {
                format!(
                    "{}={:?}/{:?}",
                    component.name, component.status, component.severity
                )
            })
            .collect::<Vec<_>>();

        if failing.is_empty() {
            "all components ready".to_string()
        } else {
            failing.join(", ")
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeploymentReport {
    pub checked_at_unix_ms: u64,
    pub overall_status: HealthStatus,
    pub ready_for_run: bool,
    pub control_plane_backend: String,
    pub blob_root: PathBuf,
    pub checks: Vec<ComponentHealth>,
}

impl DeploymentReport {
    pub fn new(
        control_plane_backend: impl Into<String>,
        blob_root: PathBuf,
        checks: Vec<ComponentHealth>,
    ) -> Self {
        let ready_for_run = checks.iter().all(|check| !check.blocks_readiness());

        Self {
            checked_at_unix_ms: unix_timestamp_ms(),
            overall_status: aggregate_status(&checks),
            ready_for_run,
            control_plane_backend: control_plane_backend.into(),
            blob_root,
            checks,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.ready_for_run
    }

    pub fn blocking_checks(&self) -> impl Iterator<Item = &ComponentHealth> {
        self.checks.iter().filter(|check| check.blocks_readiness())
    }

    pub fn blocking_summary(&self) -> String {
        let blocking = self
            .blocking_checks()
            .map(|check| match &check.remediation {
                Some(remediation) => format!(
                    "{} [{:?}/{:?}]: {} Remediation: {}",
                    check.name, check.category, check.severity, check.detail, remediation
                ),
                None => format!(
                    "{} [{:?}/{:?}]: {}",
                    check.name, check.category, check.severity, check.detail
                ),
            })
            .collect::<Vec<_>>();

        if blocking.is_empty() {
            "all deployment prerequisites are satisfied".to_string()
        } else {
            blocking.join("; ")
        }
    }
}

pub fn aggregate_status(checks: &[ComponentHealth]) -> HealthStatus {
    checks.iter().fold(HealthStatus::Ok, |current, check| {
        if check.status.severity() > current.severity() {
            check.status.clone()
        } else {
            current
        }
    })
}

pub fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// --- Action classification types (Story 4.7) ---

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionCondition {
    Healthy,
    Pending,
    ActiveRun,
    Degraded,
    Failed,
    Invalidated,
    Quarantined,
    Interrupted,
    Stale,
    TerminalComplete,
}

impl ActionCondition {
    pub fn is_action_required(&self) -> bool {
        matches!(
            self,
            Self::Degraded
                | Self::Failed
                | Self::Invalidated
                | Self::Quarantined
                | Self::Interrupted
                | Self::Stale
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionClassification {
    pub condition: ActionCondition,
    pub action_required: bool,
    pub next_action: Option<NextAction>,
    pub detail: String,
}

pub const STALE_QUEUED_ABORTED_SUMMARY: &str =
    "stale queued run aborted during startup sweep because no durable work was found";

pub fn classify_run_action(
    run: &IndexRun,
    health: &RunHealth,
    recovery_state: Option<&RunRecoveryState>,
) -> ActionClassification {
    if let Some(recovery) = recovery_state {
        if recovery.state == RecoveryStateKind::Resumed
            && matches!(
                run.status,
                IndexRunStatus::Interrupted | IndexRunStatus::Running
            )
        {
            return ActionClassification {
                condition: ActionCondition::ActiveRun,
                action_required: false,
                next_action: Some(NextAction::Wait),
                detail: format!(
                    "Run {} resume accepted. Waiting for the managed run to complete.",
                    run.run_id
                ),
            };
        }
        if recovery.state == RecoveryStateKind::ResumeRejected {
            let next = recovery
                .next_action
                .clone()
                .unwrap_or(NextAction::Reindex);
            let reason = recovery
                .detail
                .as_deref()
                .unwrap_or("resume was rejected for this run");
            return ActionClassification {
                condition: ActionCondition::Failed,
                action_required: true,
                next_action: Some(next),
                detail: format!("Run {} resume rejected: {reason}.", run.run_id),
            };
        }
    }

    match &run.status {
        IndexRunStatus::Queued => ActionClassification {
            condition: ActionCondition::Pending,
            action_required: false,
            next_action: None,
            detail: format!("Run {} is queued and awaiting execution.", run.run_id),
        },
        IndexRunStatus::Running => ActionClassification {
            condition: ActionCondition::ActiveRun,
            action_required: false,
            next_action: Some(NextAction::Wait),
            detail: format!("Run {} is actively running.", run.run_id),
        },
        IndexRunStatus::Succeeded => match health {
            RunHealth::Degraded => ActionClassification {
                condition: ActionCondition::Degraded,
                action_required: true,
                next_action: Some(NextAction::Repair),
                detail: format!(
                    "Run {} completed with degraded files. Review partial/failed outcomes.",
                    run.run_id
                ),
            },
            RunHealth::Unhealthy => ActionClassification {
                condition: ActionCondition::Failed,
                action_required: true,
                next_action: Some(NextAction::Repair),
                detail: format!(
                    "Run {} marked succeeded but health is unhealthy. Investigate file-level errors.",
                    run.run_id
                ),
            },
            RunHealth::Healthy => ActionClassification {
                condition: ActionCondition::TerminalComplete,
                action_required: false,
                next_action: None,
                detail: format!("Run {} succeeded.", run.run_id),
            },
        },
        IndexRunStatus::Failed => {
            let error = run.error_summary.as_deref().unwrap_or("unknown error");
            ActionClassification {
                condition: ActionCondition::Failed,
                action_required: true,
                next_action: Some(NextAction::Repair),
                detail: format!("Run {} failed: {error}.", run.run_id),
            }
        }
        IndexRunStatus::Interrupted => {
            let has_checkpoint = run.checkpoint_cursor.is_some();
            ActionClassification {
                condition: ActionCondition::Interrupted,
                action_required: true,
                next_action: Some(if has_checkpoint {
                    NextAction::Resume
                } else {
                    NextAction::Reindex
                }),
                detail: format!(
                    "Run {} was interrupted. {}",
                    run.run_id,
                    if has_checkpoint {
                        "A checkpoint exists; resume is possible."
                    } else {
                        "No checkpoint exists; re-index required."
                    }
                ),
            }
        }
        IndexRunStatus::Cancelled => ActionClassification {
            condition: ActionCondition::TerminalComplete,
            action_required: false,
            next_action: None,
            detail: format!("Run {} was cancelled.", run.run_id),
        },
        IndexRunStatus::Aborted => {
            let is_stale_queued =
                run.error_summary.as_deref() == Some(STALE_QUEUED_ABORTED_SUMMARY);
            if is_stale_queued {
                ActionClassification {
                    condition: ActionCondition::Failed,
                    action_required: true,
                    next_action: Some(NextAction::Reindex),
                    detail: format!(
                        "Run {} was abandoned during startup recovery because no durable work existed. Start a fresh index.",
                        run.run_id
                    ),
                }
            } else {
                ActionClassification {
                    condition: ActionCondition::Failed,
                    action_required: true,
                    next_action: Some(NextAction::Repair),
                    detail: format!(
                        "Run {} aborted (circuit breaker). Check file-level errors, consider repair.",
                        run.run_id
                    ),
                }
            }
        }
    }
}

pub fn classify_repository_action(
    status: &RepositoryStatus,
    has_completed_run: bool,
    has_active_run: bool,
    invalidation_reason: &Option<String>,
    quarantine_reason: &Option<String>,
) -> ActionClassification {
    match status {
        RepositoryStatus::Ready => ActionClassification {
            condition: ActionCondition::Healthy,
            action_required: false,
            next_action: None,
            detail: "Repository is healthy. Retrieval is safe.".to_string(),
        },
        RepositoryStatus::Pending if !has_completed_run && !has_active_run => {
            ActionClassification {
                condition: ActionCondition::Pending,
                action_required: true,
                next_action: Some(NextAction::Reindex),
                detail: "Repository has never been indexed. Run indexing to enable retrieval."
                    .to_string(),
            }
        }
        RepositoryStatus::Pending if has_active_run => ActionClassification {
            condition: ActionCondition::ActiveRun,
            action_required: false,
            next_action: Some(NextAction::Wait),
            detail: "Initial indexing is in progress.".to_string(),
        },
        RepositoryStatus::Pending => ActionClassification {
            condition: ActionCondition::Pending,
            action_required: false,
            next_action: None,
            detail: "Repository is pending. A previous run completed but status has not transitioned to ready.".to_string(),
        },
        RepositoryStatus::Degraded => ActionClassification {
            condition: ActionCondition::Degraded,
            action_required: true,
            next_action: Some(NextAction::Repair),
            detail: "Repository has degraded indexed state. Some files failed or are missing. Trigger repair to assess and restore.".to_string(),
        },
        RepositoryStatus::Failed => ActionClassification {
            condition: ActionCondition::Failed,
            action_required: true,
            next_action: Some(NextAction::Repair),
            detail: "Repository indexing failed. Trigger repair to attempt recovery or reindex."
                .to_string(),
        },
        RepositoryStatus::Invalidated => {
            let reason = invalidation_reason.as_deref().unwrap_or("unknown reason");
            ActionClassification {
                condition: ActionCondition::Invalidated,
                action_required: true,
                next_action: Some(NextAction::Reindex),
                detail: format!(
                    "Repository has been invalidated: {reason}. Reindex required to restore trusted state."
                ),
            }
        }
        RepositoryStatus::Quarantined => {
            let reason = quarantine_reason.as_deref().unwrap_or("unknown reason");
            ActionClassification {
                condition: ActionCondition::Quarantined,
                action_required: true,
                next_action: Some(NextAction::Repair),
                detail: format!(
                    "Repository has quarantined files: {reason}. Trigger repair to re-verify or reindex affected files."
                ),
            }
        }
    }
}

// --- Repository-level health inspection types (Story 4.5) ---

/// Captures the reason and timestamp for invalidation or quarantine state.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusContext {
    pub reason: String,
    pub occurred_at_unix_ms: u64,
}

/// Summarizes file-level health from the latest completed run.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileHealthSummary {
    pub total_files: usize,
    pub committed: usize,
    pub quarantined: usize,
    pub failed: usize,
    pub empty_symbols: usize,
}

/// Summarizes the latest completed run for health context.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunHealthSummary {
    pub run_id: String,
    pub status: IndexRunStatus,
    pub mode: IndexRunMode,
    pub started_at_unix_ms: u64,
    pub completed_at_unix_ms: Option<u64>,
}

/// Comprehensive repository health report synthesizing status, context, and guidance.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryHealthReport {
    pub repo_id: String,
    pub status: RepositoryStatus,
    pub classification: ActionClassification,
    pub action_required: bool,
    pub next_action: Option<NextAction>,
    pub status_detail: String,
    pub file_health: Option<FileHealthSummary>,
    pub latest_run: Option<RunHealthSummary>,
    pub active_run_id: Option<String>,
    pub recent_repairs: Vec<RepairEvent>,
    pub invalidation_context: Option<StatusContext>,
    pub quarantine_context: Option<StatusContext>,
    pub checked_at_unix_ms: u64,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        ActionClassification, ActionCondition, ComponentHealth, DeploymentReport,
        FileHealthSummary, HealthIssueCategory, HealthSeverity, HealthStatus,
        RepositoryHealthReport, RunHealthSummary, StatusContext,
    };
    use super::{classify_repository_action, classify_run_action};
    use crate::domain::{
        IndexRun, IndexRunMode, IndexRunStatus, NextAction, RecoveryStateKind, RepairEvent,
        RepairOutcome, RepairScope, RepositoryStatus, ResumeRejectReason, RunHealth,
        RunRecoveryState,
    };

    #[test]
    fn warnings_do_not_block_readiness() {
        let report = DeploymentReport::new(
            "spacetimedb",
            PathBuf::from(".tokenizor"),
            vec![ComponentHealth::warning(
                "spacetimedb_schema_compatibility",
                HealthIssueCategory::Compatibility,
                "schema compatibility is not fully verified yet",
                "Run `tokenizor_agentic_mcp doctor` after the compatibility probe is implemented.",
            )],
        );

        assert_eq!(report.overall_status, HealthStatus::Degraded);
        assert!(report.is_ready());
        assert_eq!(report.blocking_checks().count(), 0);
    }

    #[test]
    fn errors_block_readiness_and_preserve_remediation() {
        let report = DeploymentReport::new(
            "spacetimedb",
            PathBuf::from(".tokenizor"),
            vec![ComponentHealth::error(
                "spacetimedb_cli",
                HealthIssueCategory::Dependency,
                "`spacetimedb` is not installed",
                "Install the SpacetimeDB CLI and ensure it is on PATH.",
            )],
        );

        let blocking = report.blocking_checks().collect::<Vec<_>>();

        assert!(!report.is_ready());
        assert_eq!(blocking.len(), 1);
        assert_eq!(blocking[0].severity, HealthSeverity::Error);
        assert_eq!(
            blocking[0].remediation.as_deref(),
            Some("Install the SpacetimeDB CLI and ensure it is on PATH.")
        );
        assert!(
            report
                .blocking_summary()
                .contains("Remediation: Install the SpacetimeDB CLI")
        );
    }

    #[test]
    fn serializes_machine_readable_metadata() {
        let check = ComponentHealth::error(
            "blob_store",
            HealthIssueCategory::Storage,
            "local CAS layout is missing required directories",
            "Run `tokenizor_agentic_mcp init` to create the CAS layout.",
        );

        let value = serde_json::to_value(&check).expect("component health should serialize");

        assert_eq!(value["category"], "storage");
        assert_eq!(value["status"], "unavailable");
        assert_eq!(value["severity"], "error");
        assert_eq!(
            value["remediation"],
            "Run `tokenizor_agentic_mcp init` to create the CAS layout."
        );
    }

    #[test]
    fn repository_health_report_serializes_all_fields() {
        let report = RepositoryHealthReport {
            repo_id: "repo-1".to_string(),
            status: RepositoryStatus::Ready,
            classification: ActionClassification {
                condition: ActionCondition::Healthy,
                action_required: false,
                next_action: None,
                detail: "Repository is healthy. Retrieval is safe.".to_string(),
            },
            action_required: false,
            next_action: None,
            status_detail: "Repository is healthy. Retrieval is safe.".to_string(),
            file_health: Some(FileHealthSummary {
                total_files: 10,
                committed: 8,
                quarantined: 1,
                failed: 0,
                empty_symbols: 1,
            }),
            latest_run: Some(RunHealthSummary {
                run_id: "run-1".to_string(),
                status: IndexRunStatus::Succeeded,
                mode: IndexRunMode::Full,
                started_at_unix_ms: 1000,
                completed_at_unix_ms: Some(2000),
            }),
            active_run_id: None,
            recent_repairs: vec![],
            invalidation_context: None,
            quarantine_context: None,
            checked_at_unix_ms: 3000,
        };

        let value =
            serde_json::to_value(&report).expect("repository health report should serialize");
        assert_eq!(value["repo_id"], "repo-1");
        assert_eq!(value["status"], "ready");
        assert_eq!(value["action_required"], false);
        assert!(value["next_action"].is_null());
        assert_eq!(
            value["status_detail"],
            "Repository is healthy. Retrieval is safe."
        );
        assert_eq!(value["file_health"]["total_files"], 10);
        assert_eq!(value["file_health"]["committed"], 8);
        assert_eq!(value["file_health"]["quarantined"], 1);
        assert_eq!(value["latest_run"]["run_id"], "run-1");
        assert_eq!(value["latest_run"]["status"], "succeeded");
        assert_eq!(value["latest_run"]["mode"], "full");
        assert_eq!(value["checked_at_unix_ms"], 3000);
    }

    #[test]
    fn repository_health_report_roundtrip_with_contexts() {
        let report = RepositoryHealthReport {
            repo_id: "repo-2".to_string(),
            status: RepositoryStatus::Invalidated,
            classification: ActionClassification {
                condition: ActionCondition::Invalidated,
                action_required: true,
                next_action: Some(NextAction::Reindex),
                detail: "Repository has been invalidated: stale data.".to_string(),
            },
            action_required: true,
            next_action: Some(NextAction::Reindex),
            status_detail: "Repository has been invalidated: stale data.".to_string(),
            file_health: None,
            latest_run: None,
            active_run_id: None,
            recent_repairs: vec![RepairEvent {
                repo_id: "repo-2".to_string(),
                scope: RepairScope::Repository,
                previous_status: RepositoryStatus::Degraded,
                outcome: RepairOutcome::Restored,
                detail: "repaired degraded state".to_string(),
                timestamp_unix_ms: 5000,
            }],
            invalidation_context: Some(StatusContext {
                reason: "stale data".to_string(),
                occurred_at_unix_ms: 4000,
            }),
            quarantine_context: None,
            checked_at_unix_ms: 6000,
        };

        let json = serde_json::to_string(&report).unwrap();
        let deserialized: RepositoryHealthReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, report);
    }

    #[test]
    fn file_health_summary_serializes_counts() {
        let summary = FileHealthSummary {
            total_files: 20,
            committed: 15,
            quarantined: 2,
            failed: 1,
            empty_symbols: 2,
        };

        let value =
            serde_json::to_value(&summary).expect("file health summary should serialize");
        assert_eq!(value["total_files"], 20);
        assert_eq!(value["committed"], 15);
        assert_eq!(value["quarantined"], 2);
        assert_eq!(value["failed"], 1);
        assert_eq!(value["empty_symbols"], 2);
    }

    #[test]
    fn status_context_roundtrip() {
        let ctx = StatusContext {
            reason: "quarantine policy triggered".to_string(),
            occurred_at_unix_ms: 7000,
        };

        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: StatusContext = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ctx);
    }

    #[test]
    fn run_health_summary_roundtrip() {
        let summary = RunHealthSummary {
            run_id: "run-42".to_string(),
            status: IndexRunStatus::Failed,
            mode: IndexRunMode::Incremental,
            started_at_unix_ms: 1000,
            completed_at_unix_ms: Some(2000),
        };

        let json = serde_json::to_string(&summary).unwrap();
        let deserialized: RunHealthSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, summary);
    }

    // --- Task 5.1: ActionCondition and ActionClassification type tests ---

    #[test]
    fn test_action_condition_is_action_required() {
        assert!(!ActionCondition::Healthy.is_action_required());
        assert!(!ActionCondition::Pending.is_action_required());
        assert!(!ActionCondition::ActiveRun.is_action_required());
        assert!(!ActionCondition::TerminalComplete.is_action_required());
        assert!(ActionCondition::Degraded.is_action_required());
        assert!(ActionCondition::Failed.is_action_required());
        assert!(ActionCondition::Invalidated.is_action_required());
        assert!(ActionCondition::Quarantined.is_action_required());
        assert!(ActionCondition::Interrupted.is_action_required());
        assert!(ActionCondition::Stale.is_action_required());
    }

    #[test]
    fn test_action_classification_serialization_roundtrip() {
        let classification = ActionClassification {
            condition: ActionCondition::Degraded,
            action_required: true,
            next_action: Some(NextAction::Repair),
            detail: "Run run-1 completed with degraded files.".to_string(),
        };
        let json = serde_json::to_string(&classification).unwrap();
        let deserialized: ActionClassification = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, classification);
    }

    #[test]
    fn test_action_condition_serializes_snake_case() {
        let cases = vec![
            (ActionCondition::Healthy, "\"healthy\""),
            (ActionCondition::Pending, "\"pending\""),
            (ActionCondition::ActiveRun, "\"active_run\""),
            (ActionCondition::Degraded, "\"degraded\""),
            (ActionCondition::Failed, "\"failed\""),
            (ActionCondition::Invalidated, "\"invalidated\""),
            (ActionCondition::Quarantined, "\"quarantined\""),
            (ActionCondition::Interrupted, "\"interrupted\""),
            (ActionCondition::Stale, "\"stale\""),
            (ActionCondition::TerminalComplete, "\"terminal_complete\""),
        ];
        for (variant, expected) in cases {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, expected, "ActionCondition::{variant:?} serialization mismatch");
        }
    }

    // --- Task 5.2: classify_run_action tests ---

    fn sample_run(status: IndexRunStatus) -> IndexRun {
        IndexRun {
            run_id: "run-test".into(),
            repo_id: "repo-test".into(),
            mode: IndexRunMode::Full,
            status,
            requested_at_unix_ms: 1000,
            started_at_unix_ms: Some(1001),
            finished_at_unix_ms: None,
            idempotency_key: None,
            request_hash: None,
            checkpoint_cursor: None,
            error_summary: None,
            not_yet_supported: None,
            prior_run_id: None,
            description: None,
            recovery_state: None,
        }
    }

    #[test]
    fn test_classify_queued_run_not_action_required() {
        let run = sample_run(IndexRunStatus::Queued);
        let result = classify_run_action(&run, &RunHealth::Healthy, None);
        assert_eq!(result.condition, ActionCondition::Pending);
        assert!(!result.action_required);
        assert!(result.next_action.is_none());
    }

    #[test]
    fn test_classify_running_run_active() {
        let run = sample_run(IndexRunStatus::Running);
        let result = classify_run_action(&run, &RunHealth::Healthy, None);
        assert_eq!(result.condition, ActionCondition::ActiveRun);
        assert_eq!(result.next_action, Some(NextAction::Wait));
        assert!(!result.action_required);
    }

    #[test]
    fn test_classify_succeeded_healthy_terminal() {
        let run = sample_run(IndexRunStatus::Succeeded);
        let result = classify_run_action(&run, &RunHealth::Healthy, None);
        assert_eq!(result.condition, ActionCondition::TerminalComplete);
        assert!(!result.action_required);
        assert!(result.next_action.is_none());
    }

    #[test]
    fn test_classify_succeeded_degraded_action_required() {
        let run = sample_run(IndexRunStatus::Succeeded);
        let result = classify_run_action(&run, &RunHealth::Degraded, None);
        assert_eq!(result.condition, ActionCondition::Degraded);
        assert!(result.action_required);
        assert_eq!(result.next_action, Some(NextAction::Repair));
    }

    #[test]
    fn test_classify_failed_run_action_required() {
        let mut run = sample_run(IndexRunStatus::Failed);
        run.error_summary = Some("disk full".to_string());
        let result = classify_run_action(&run, &RunHealth::Unhealthy, None);
        assert_eq!(result.condition, ActionCondition::Failed);
        assert!(result.action_required);
        assert_eq!(result.next_action, Some(NextAction::Repair));
        assert!(result.detail.contains("disk full"));
    }

    #[test]
    fn test_classify_interrupted_with_checkpoint_resume() {
        let mut run = sample_run(IndexRunStatus::Interrupted);
        run.checkpoint_cursor = Some("file_a.rs".to_string());
        let result = classify_run_action(&run, &RunHealth::Healthy, None);
        assert_eq!(result.condition, ActionCondition::Interrupted);
        assert!(result.action_required);
        assert_eq!(result.next_action, Some(NextAction::Resume));
    }

    #[test]
    fn test_classify_interrupted_no_checkpoint_reindex() {
        let run = sample_run(IndexRunStatus::Interrupted);
        let result = classify_run_action(&run, &RunHealth::Healthy, None);
        assert_eq!(result.condition, ActionCondition::Interrupted);
        assert!(result.action_required);
        assert_eq!(result.next_action, Some(NextAction::Reindex));
    }

    #[test]
    fn test_classify_cancelled_terminal_complete() {
        let run = sample_run(IndexRunStatus::Cancelled);
        let result = classify_run_action(&run, &RunHealth::Healthy, None);
        assert_eq!(result.condition, ActionCondition::TerminalComplete);
        assert!(!result.action_required);
        assert!(result.next_action.is_none());
    }

    #[test]
    fn test_classify_aborted_action_required() {
        let run = sample_run(IndexRunStatus::Aborted);
        let result = classify_run_action(&run, &RunHealth::Unhealthy, None);
        assert_eq!(result.condition, ActionCondition::Failed);
        assert!(result.action_required);
        assert_eq!(result.next_action, Some(NextAction::Repair));
    }

    #[test]
    fn test_classify_resumed_run_active() {
        let run = sample_run(IndexRunStatus::Running);
        let recovery = RunRecoveryState {
            state: RecoveryStateKind::Resumed,
            rejection_reason: None,
            next_action: None,
            detail: None,
            updated_at_unix_ms: 2000,
        };
        let result = classify_run_action(&run, &RunHealth::Healthy, Some(&recovery));
        assert_eq!(result.condition, ActionCondition::ActiveRun);
        assert_eq!(result.next_action, Some(NextAction::Wait));
        assert!(!result.action_required);
    }

    #[test]
    fn test_classify_resume_rejected_failed() {
        let run = sample_run(IndexRunStatus::Interrupted);
        let recovery = RunRecoveryState {
            state: RecoveryStateKind::ResumeRejected,
            rejection_reason: Some(ResumeRejectReason::MissingCheckpoint),
            next_action: Some(NextAction::Reindex),
            detail: Some("resume was rejected".to_string()),
            updated_at_unix_ms: 2000,
        };
        let result = classify_run_action(&run, &RunHealth::Healthy, Some(&recovery));
        assert_eq!(result.condition, ActionCondition::Failed);
        assert!(result.action_required);
        assert_eq!(result.next_action, Some(NextAction::Reindex));
    }

    #[test]
    fn test_classify_detail_includes_run_context() {
        let mut run = sample_run(IndexRunStatus::Failed);
        run.error_summary = Some("disk full".to_string());
        let result = classify_run_action(&run, &RunHealth::Unhealthy, None);
        assert!(result.detail.contains("disk full"));
    }

    #[test]
    fn test_classify_aborted_stale_queued_reindex() {
        let mut run = sample_run(IndexRunStatus::Aborted);
        run.error_summary = Some(super::STALE_QUEUED_ABORTED_SUMMARY.to_string());
        let result = classify_run_action(&run, &RunHealth::Unhealthy, None);
        assert_eq!(result.condition, ActionCondition::Failed);
        assert!(result.action_required);
        assert_eq!(result.next_action, Some(NextAction::Reindex));
        assert!(result.detail.contains("startup recovery"));
    }

    // --- Task 5.3: classify_repository_action tests ---

    #[test]
    fn test_classify_ready_repository_healthy() {
        let result = classify_repository_action(
            &RepositoryStatus::Ready,
            true,
            false,
            &None,
            &None,
        );
        assert_eq!(result.condition, ActionCondition::Healthy);
        assert!(!result.action_required);
        assert!(result.next_action.is_none());
    }

    #[test]
    fn test_classify_pending_never_indexed() {
        let result = classify_repository_action(
            &RepositoryStatus::Pending,
            false,
            false,
            &None,
            &None,
        );
        assert_eq!(result.condition, ActionCondition::Pending);
        assert!(result.action_required);
        assert_eq!(result.next_action, Some(NextAction::Reindex));
    }

    #[test]
    fn test_classify_pending_active_run() {
        let result = classify_repository_action(
            &RepositoryStatus::Pending,
            false,
            true,
            &None,
            &None,
        );
        assert_eq!(result.condition, ActionCondition::ActiveRun);
        assert!(!result.action_required);
        assert_eq!(result.next_action, Some(NextAction::Wait));
    }

    #[test]
    fn test_classify_degraded_repository() {
        let result = classify_repository_action(
            &RepositoryStatus::Degraded,
            true,
            false,
            &None,
            &None,
        );
        assert_eq!(result.condition, ActionCondition::Degraded);
        assert!(result.action_required);
        assert_eq!(result.next_action, Some(NextAction::Repair));
    }

    #[test]
    fn test_classify_failed_repository() {
        let result = classify_repository_action(
            &RepositoryStatus::Failed,
            true,
            false,
            &None,
            &None,
        );
        assert_eq!(result.condition, ActionCondition::Failed);
        assert!(result.action_required);
        assert_eq!(result.next_action, Some(NextAction::Repair));
    }

    #[test]
    fn test_classify_invalidated_repository() {
        let result = classify_repository_action(
            &RepositoryStatus::Invalidated,
            true,
            false,
            &None,
            &None,
        );
        assert_eq!(result.condition, ActionCondition::Invalidated);
        assert!(result.action_required);
        assert_eq!(result.next_action, Some(NextAction::Reindex));
    }

    #[test]
    fn test_classify_quarantined_repository() {
        let result = classify_repository_action(
            &RepositoryStatus::Quarantined,
            true,
            false,
            &None,
            &None,
        );
        assert_eq!(result.condition, ActionCondition::Quarantined);
        assert!(result.action_required);
        assert_eq!(result.next_action, Some(NextAction::Repair));
    }

    #[test]
    fn test_classify_invalidated_includes_reason() {
        let result = classify_repository_action(
            &RepositoryStatus::Invalidated,
            true,
            false,
            &Some("stale data".to_string()),
            &None,
        );
        assert_eq!(result.condition, ActionCondition::Invalidated);
        assert!(result.detail.contains("stale data"));
    }

    // --- Task 5.6: RepositoryHealthReport enrichment tests ---

    #[test]
    fn test_repository_health_report_includes_classification() {
        let report = RepositoryHealthReport {
            repo_id: "repo-1".to_string(),
            status: RepositoryStatus::Ready,
            classification: ActionClassification {
                condition: ActionCondition::Healthy,
                action_required: false,
                next_action: None,
                detail: "Repository is healthy. Retrieval is safe.".to_string(),
            },
            action_required: false,
            next_action: None,
            status_detail: "Repository is healthy. Retrieval is safe.".to_string(),
            file_health: None,
            latest_run: None,
            active_run_id: None,
            recent_repairs: vec![],
            invalidation_context: None,
            quarantine_context: None,
            checked_at_unix_ms: 3000,
        };

        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["classification"]["condition"], "healthy");
    }

    #[test]
    fn test_repository_health_report_backward_compat() {
        let report = RepositoryHealthReport {
            repo_id: "repo-2".to_string(),
            status: RepositoryStatus::Invalidated,
            classification: ActionClassification {
                condition: ActionCondition::Invalidated,
                action_required: true,
                next_action: Some(NextAction::Reindex),
                detail: "Repository has been invalidated: stale data.".to_string(),
            },
            action_required: true,
            next_action: Some(NextAction::Reindex),
            status_detail: "Repository has been invalidated: stale data.".to_string(),
            file_health: None,
            latest_run: None,
            active_run_id: None,
            recent_repairs: vec![],
            invalidation_context: None,
            quarantine_context: None,
            checked_at_unix_ms: 4000,
        };

        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["action_required"], true);
        assert_eq!(value["next_action"], "reindex");
        assert_eq!(
            value["status_detail"],
            "Repository has been invalidated: stale data."
        );
        assert_eq!(value["classification"]["condition"], "invalidated");
        assert_eq!(value["classification"]["action_required"], true);
        assert_eq!(value["classification"]["next_action"], "reindex");
    }
}
