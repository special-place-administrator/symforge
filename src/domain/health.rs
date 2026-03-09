use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::index::{IndexRunMode, IndexRunStatus, RepairEvent};
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
        ComponentHealth, DeploymentReport, FileHealthSummary, HealthIssueCategory, HealthSeverity,
        HealthStatus, RepositoryHealthReport, RunHealthSummary, StatusContext,
    };
    use crate::domain::{
        IndexRunMode, IndexRunStatus, NextAction, RepairEvent, RepairOutcome, RepairScope,
        RepositoryStatus,
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
}
