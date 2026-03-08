use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Repository {
    pub repo_id: String,
    pub kind: RepositoryKind,
    pub root_uri: String,
    #[serde(default)]
    pub project_identity: String,
    #[serde(default)]
    pub project_identity_kind: ProjectIdentityKind,
    pub default_branch: Option<String>,
    pub last_known_revision: Option<String>,
    pub status: RepositoryStatus,
    #[serde(default)]
    pub invalidated_at_unix_ms: Option<u64>,
    #[serde(default)]
    pub invalidation_reason: Option<String>,
    #[serde(default)]
    pub quarantined_at_unix_ms: Option<u64>,
    #[serde(default)]
    pub quarantine_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectIdentityKind {
    #[default]
    LegacyRootUri,
    LocalRootPath,
    GitCommonDir,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryKind {
    Local,
    Git,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryStatus {
    Pending,
    Ready,
    Degraded,
    Failed,
    Invalidated,
    Quarantined,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvalidationResult {
    pub repo_id: String,
    pub previous_status: RepositoryStatus,
    pub invalidated_at_unix_ms: u64,
    pub reason: Option<String>,
    pub action_required: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_deserialize_without_invalidation_fields_backward_compat() {
        let json = r#"{
            "repo_id": "repo-1",
            "kind": "git",
            "root_uri": "/tmp/test",
            "project_identity": "id-1",
            "project_identity_kind": "local_root_path",
            "default_branch": null,
            "last_known_revision": null,
            "status": "ready"
        }"#;
        let repo: Repository = serde_json::from_str(json).unwrap();
        assert_eq!(repo.repo_id, "repo-1");
        assert_eq!(repo.status, RepositoryStatus::Ready);
        assert!(repo.invalidated_at_unix_ms.is_none());
        assert!(repo.invalidation_reason.is_none());
        assert!(repo.quarantined_at_unix_ms.is_none());
        assert!(repo.quarantine_reason.is_none());
    }

    #[test]
    fn test_repository_roundtrip_with_invalidation_fields() {
        let repo = Repository {
            repo_id: "repo-1".to_string(),
            kind: RepositoryKind::Git,
            root_uri: "/tmp/test".to_string(),
            project_identity: "id-1".to_string(),
            project_identity_kind: ProjectIdentityKind::LocalRootPath,
            default_branch: None,
            last_known_revision: None,
            status: RepositoryStatus::Invalidated,
            invalidated_at_unix_ms: Some(1709827200000),
            invalidation_reason: Some("stale data after branch switch".to_string()),
            quarantined_at_unix_ms: None,
            quarantine_reason: None,
        };
        let json = serde_json::to_string(&repo).unwrap();
        let deserialized: Repository = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, RepositoryStatus::Invalidated);
        assert_eq!(deserialized.invalidated_at_unix_ms, Some(1709827200000));
        assert_eq!(
            deserialized.invalidation_reason.as_deref(),
            Some("stale data after branch switch"),
        );
        assert!(deserialized.quarantined_at_unix_ms.is_none());
        assert!(deserialized.quarantine_reason.is_none());
    }

    #[test]
    fn test_repository_roundtrip_with_quarantine_fields() {
        let repo = Repository {
            repo_id: "repo-2".to_string(),
            kind: RepositoryKind::Git,
            root_uri: "/tmp/test".to_string(),
            project_identity: "id-2".to_string(),
            project_identity_kind: ProjectIdentityKind::LocalRootPath,
            default_branch: None,
            last_known_revision: None,
            status: RepositoryStatus::Quarantined,
            invalidated_at_unix_ms: None,
            invalidation_reason: None,
            quarantined_at_unix_ms: Some(1709827300000),
            quarantine_reason: Some("quarantine policy triggered".to_string()),
        };
        let json = serde_json::to_string(&repo).unwrap();
        let deserialized: Repository = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, RepositoryStatus::Quarantined);
        assert_eq!(deserialized.quarantined_at_unix_ms, Some(1709827300000));
        assert_eq!(
            deserialized.quarantine_reason.as_deref(),
            Some("quarantine policy triggered")
        );
    }

    #[test]
    fn test_repository_status_invalidated_serializes_as_snake_case() {
        let json = serde_json::to_string(&RepositoryStatus::Invalidated).unwrap();
        assert_eq!(json, r#""invalidated""#);
        let deserialized: RepositoryStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, RepositoryStatus::Invalidated);
    }

    #[test]
    fn test_invalidation_result_roundtrip() {
        let result = InvalidationResult {
            repo_id: "repo-1".to_string(),
            previous_status: RepositoryStatus::Ready,
            invalidated_at_unix_ms: 1709827200000,
            reason: Some("manual invalidation".to_string()),
            action_required: "re-index or repair required".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: InvalidationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.repo_id, "repo-1");
        assert_eq!(deserialized.previous_status, RepositoryStatus::Ready);
        assert_eq!(deserialized.invalidated_at_unix_ms, 1709827200000);
        assert_eq!(deserialized.reason.as_deref(), Some("manual invalidation"));
    }
}
