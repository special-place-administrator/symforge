use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexRun {
    pub run_id: String,
    pub repo_id: String,
    pub mode: IndexRunMode,
    pub status: IndexRunStatus,
    pub requested_at_unix_ms: u64,
    pub started_at_unix_ms: Option<u64>,
    pub finished_at_unix_ms: Option<u64>,
    pub idempotency_key: Option<String>,
    pub request_hash: Option<String>,
    pub checkpoint_cursor: Option<String>,
    pub error_summary: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IndexRunMode {
    Full,
    Incremental,
    Repair,
    Verify,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IndexRunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Checkpoint {
    pub run_id: String,
    pub cursor: String,
    pub files_processed: u64,
    pub symbols_written: u64,
    pub created_at_unix_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_interrupted_status() {
        let json = r#""interrupted""#;
        let status: IndexRunStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, IndexRunStatus::Interrupted);
    }

    #[test]
    fn test_serialize_interrupted_status() {
        let json = serde_json::to_string(&IndexRunStatus::Interrupted).unwrap();
        assert_eq!(json, r#""interrupted""#);
    }

    #[test]
    fn test_deserialize_existing_statuses_backward_compatible() {
        let cases = vec![
            (r#""queued""#, IndexRunStatus::Queued),
            (r#""running""#, IndexRunStatus::Running),
            (r#""succeeded""#, IndexRunStatus::Succeeded),
            (r#""failed""#, IndexRunStatus::Failed),
            (r#""cancelled""#, IndexRunStatus::Cancelled),
        ];
        for (json, expected) in cases {
            let status: IndexRunStatus = serde_json::from_str(json).unwrap();
            assert_eq!(status, expected, "failed for {json}");
        }
    }

    #[test]
    fn test_roundtrip_index_run_with_interrupted() {
        let run = IndexRun {
            run_id: "test-run".to_string(),
            repo_id: "repo-1".to_string(),
            mode: IndexRunMode::Full,
            status: IndexRunStatus::Interrupted,
            requested_at_unix_ms: 1000,
            started_at_unix_ms: Some(1001),
            finished_at_unix_ms: None,
            idempotency_key: None,
            request_hash: None,
            checkpoint_cursor: None,
            error_summary: Some("process exited unexpectedly".to_string()),
        };
        let json = serde_json::to_string(&run).unwrap();
        let deserialized: IndexRun = serde_json::from_str(&json).unwrap();
        assert_eq!(run, deserialized);
    }
}
