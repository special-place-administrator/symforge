use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{
        Annotated, CallToolResult, Content, Implementation, ListResourcesResult,
        PaginatedRequestParams, RawResource, ReadResourceRequestParams, ReadResourceResult,
        ResourceContents, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};

use schemars::JsonSchema;
use serde::Deserialize;

use crate::domain::{BatchRetrievalRequest, IndexRunMode, IndexRunStatus, SymbolKind};
use crate::{ApplicationContext, TokenizorError};

const RUN_STATUS_URI_PREFIX: &str = "tokenizor://runs/";
const RUN_STATUS_URI_SUFFIX: &str = "/status";
const VALID_KIND_FILTERS: &str = "function, method, class, struct, enum, interface, module, constant, variable, type, trait, impl, other";

// ---------------------------------------------------------------------------
// Input structs for typed MCP tool parameters
// ---------------------------------------------------------------------------

/// Input for the index_folder tool.
#[derive(Deserialize, JsonSchema)]
struct IndexFolderInput {
    /// Unique identifier for the repository.
    repo_id: String,
    /// Absolute path to the repository root on disk.
    repo_root: String,
    /// Indexing mode: full, incremental, repair, or verify. Defaults to full.
    mode: Option<String>,
}

/// Input for the get_index_run tool.
#[derive(Deserialize, JsonSchema)]
struct GetIndexRunInput {
    /// The indexing run identifier to inspect.
    run_id: String,
}

/// Input for the list_index_runs tool.
#[derive(Deserialize, JsonSchema)]
struct ListIndexRunsInput {
    /// Filter runs by repository identifier.
    repo_id: Option<String>,
    /// Filter runs by status: queued, running, succeeded, failed, cancelled, interrupted, or aborted.
    status: Option<String>,
}

/// Input for the cancel_index_run tool.
#[derive(Deserialize, JsonSchema)]
struct CancelIndexRunInput {
    /// The indexing run identifier to cancel.
    run_id: String,
}

/// Input for the checkpoint_now tool.
#[derive(Deserialize, JsonSchema)]
struct CheckpointNowInput {
    /// The indexing run identifier to checkpoint.
    run_id: String,
}

/// Input for the resume_index_run tool.
#[derive(Deserialize, JsonSchema)]
struct ResumeIndexRunInput {
    /// The indexing run identifier to resume.
    run_id: String,
    /// Absolute path to the repository root on disk.
    repo_root: String,
}

/// Input for the reindex_repository tool.
#[derive(Deserialize, JsonSchema)]
struct ReindexRepositoryInput {
    /// Unique identifier for the repository.
    repo_id: String,
    /// Absolute path to the repository root on disk.
    repo_root: String,
    /// Optional workspace identifier.
    workspace_id: Option<String>,
    /// Optional description of why re-indexing is needed.
    reason: Option<String>,
}

/// Input for the invalidate_indexed_state tool.
#[derive(Deserialize, JsonSchema)]
struct InvalidateIndexedStateInput {
    /// Unique identifier for the repository.
    repo_id: String,
    /// Optional workspace identifier.
    workspace_id: Option<String>,
    /// Optional description of why invalidation is needed.
    reason: Option<String>,
}

/// Input for the search_text tool.
#[derive(Deserialize, JsonSchema)]
struct SearchTextInput {
    /// Unique identifier for the repository.
    repo_id: String,
    /// Non-empty search text.
    query: String,
}

/// Input for the search_symbols tool.
#[derive(Deserialize, JsonSchema)]
struct SearchSymbolsInput {
    /// Unique identifier for the repository.
    repo_id: String,
    /// Non-empty search text.
    query: String,
    /// Optional symbol kind filter: function, method, class, struct, enum, interface, module, constant, variable, type, trait, impl, or other.
    kind_filter: Option<String>,
}

/// Input for the get_file_outline tool.
#[derive(Deserialize, JsonSchema)]
struct GetFileOutlineInput {
    /// Unique identifier for the repository.
    repo_id: String,
    /// File path relative to repository root.
    relative_path: String,
}

/// Input for the get_repo_outline tool.
#[derive(Deserialize, JsonSchema)]
struct GetRepoOutlineInput {
    /// Unique identifier for the repository.
    repo_id: String,
}

/// Input for the get_symbol tool.
#[derive(Deserialize, JsonSchema)]
struct GetSymbolInput {
    /// Unique identifier for the repository.
    repo_id: String,
    /// File path relative to repository root.
    relative_path: String,
    /// Exact symbol name to retrieve.
    symbol_name: String,
    /// Optional symbol kind filter: function, method, class, struct, enum, interface, module, constant, variable, type, trait, impl, or other.
    kind_filter: Option<String>,
}

/// Input for the get_symbols tool.
#[derive(Deserialize, JsonSchema)]
struct GetSymbolsInput {
    /// Unique identifier for the repository.
    repo_id: String,
    /// Ordered array of retrieval request objects with request_type=symbol or request_type=code_slice.
    targets: Option<Vec<serde_json::Value>>,
    /// Legacy array of symbol request objects.
    symbols: Option<Vec<serde_json::Value>>,
}

/// Input for the repair_index tool.
#[derive(Deserialize, JsonSchema)]
struct RepairIndexInput {
    /// Unique identifier for the repository.
    repository_id: String,
    /// Absolute path to the repository root on disk.
    repo_root: String,
    /// Repair scope: repository, run, or file. Defaults to repository.
    scope: Option<String>,
    /// Required when scope is run or file.
    run_id: Option<String>,
    /// Required when scope is file.
    relative_path: Option<String>,
}

/// Input for the inspect_repository_health tool.
#[derive(Deserialize, JsonSchema)]
struct InspectRepositoryHealthInput {
    /// Unique identifier for the repository.
    repository_id: String,
}

/// Input for the get_operational_history tool.
#[derive(Deserialize, JsonSchema)]
struct GetOperationalHistoryInput {
    /// Unique identifier for the repository.
    repository_id: String,
    /// Filter by event name prefix, e.g. 'run', 'repair', 'integrity'.
    category: Option<String>,
    /// Only events at or after this timestamp (milliseconds since Unix epoch).
    since_unix_ms: Option<u64>,
    /// Maximum number of events to return, capped at 200.
    limit: Option<u64>,
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn require_non_empty(value: &str, field: &str) -> Result<(), McpError> {
    if value.trim().is_empty() {
        return Err(McpError::invalid_params(
            format!("invalid parameter `{field}`: expected non-empty string"),
            None,
        ));
    }
    Ok(())
}

fn invalid_kind_filter_type_error() -> McpError {
    McpError::invalid_params(
        format!(
            "invalid parameter `kind_filter`: expected string. Valid kinds: {VALID_KIND_FILTERS}"
        ),
        None,
    )
}

fn unknown_kind_filter_error(value: &str) -> McpError {
    McpError::invalid_params(
        format!("unknown kind_filter: `{value}`. Valid kinds: {VALID_KIND_FILTERS}"),
        None,
    )
}

fn parse_kind_filter_value(
    kind_value: Option<&serde_json::Value>,
) -> Result<Option<SymbolKind>, McpError> {
    let Some(value) = kind_value else {
        return Ok(None);
    };
    let kind_str = value.as_str().ok_or_else(invalid_kind_filter_type_error)?;
    let kind = match kind_str {
        "function" => SymbolKind::Function,
        "method" => SymbolKind::Method,
        "class" => SymbolKind::Class,
        "struct" => SymbolKind::Struct,
        "enum" => SymbolKind::Enum,
        "interface" => SymbolKind::Interface,
        "module" => SymbolKind::Module,
        "constant" => SymbolKind::Constant,
        "variable" => SymbolKind::Variable,
        "type" => SymbolKind::Type,
        "trait" => SymbolKind::Trait,
        "impl" => SymbolKind::Impl,
        "other" => SymbolKind::Other,
        other => return Err(unknown_kind_filter_error(other)),
    };
    Ok(Some(kind))
}

fn parse_kind_filter_str(kind_str: Option<&str>) -> Result<Option<SymbolKind>, McpError> {
    match kind_str {
        None => Ok(None),
        Some(s) => parse_kind_filter_value(Some(&serde_json::Value::String(s.to_string()))),
    }
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TokenizorServer {
    tool_router: ToolRouter<Self>,
    application: ApplicationContext,
}

#[tool_router]
impl TokenizorServer {
    pub fn new(application: ApplicationContext) -> Self {
        Self {
            tool_router: Self::tool_router(),
            application,
        }
    }

    #[tool(
        description = "Report runtime health for the MCP server, SpacetimeDB control plane, and local byte-exact CAS."
    )]
    fn health(&self) -> Result<CallToolResult, McpError> {
        let report = self.application.health_report().map_err(to_mcp_error)?;
        let payload = serde_json::to_string(&report).map_err(|error| {
            McpError::internal_error(format!("failed to serialize health report: {error}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    #[tool(
        description = "Start an indexing run for a repository. Returns the run ID immediately without blocking on the full indexing pipeline. Parameters: repo_id (string, required), repo_root (string, required — absolute path to repository), mode (string, optional: full|incremental|repair|verify, defaults to full)."
    )]
    fn index_folder(
        &self,
        params: Parameters<IndexFolderInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        require_non_empty(&input.repo_id, "repo_id")?;
        require_non_empty(&input.repo_root, "repo_root")?;

        let repo_root = std::path::PathBuf::from(&input.repo_root);

        let run_mode = match input.mode.as_deref() {
            Some("full") | None => IndexRunMode::Full,
            Some("incremental") => IndexRunMode::Incremental,
            Some("repair") => IndexRunMode::Repair,
            Some("verify") => IndexRunMode::Verify,
            Some(other) => {
                return Err(McpError::invalid_params(
                    format!(
                        "unknown indexing mode: `{other}`. Valid modes: full, incremental, repair, verify"
                    ),
                    None,
                ));
            }
        };

        let (run, _progress) = self
            .application
            .launch_indexing(&input.repo_id, run_mode, repo_root)
            .map_err(to_mcp_error)?;

        let payload = serde_json::to_string(&run).map_err(|error| {
            McpError::internal_error(format!("failed to serialize index run: {error}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    #[tool(
        description = "Inspect the status and health of an indexing run. Returns lifecycle state, health classification, structured action classification (condition, action_required, next_action guidance), progress (if active), and file outcome summary. The classification field explicitly distinguishes action-required states from normal health. Parameters: run_id (string, required)."
    )]
    fn get_index_run(
        &self,
        params: Parameters<GetIndexRunInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        let report = self
            .application
            .run_manager()
            .inspect_run(&input.run_id)
            .map_err(to_mcp_error)?;

        let json = serde_json::to_string_pretty(&report).map_err(|e| {
            McpError::internal_error(format!("failed to serialize run status report: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "List indexing runs, optionally filtered by repository or status. Returns status and health for each run. Parameters: repo_id (string, optional), status (string, optional: queued|running|succeeded|failed|cancelled|interrupted|aborted)."
    )]
    fn list_index_runs(
        &self,
        params: Parameters<ListIndexRunsInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        let status_filter = if let Some(ref status_str) = input.status {
            let parsed = match status_str.as_str() {
                "queued" => IndexRunStatus::Queued,
                "running" => IndexRunStatus::Running,
                "succeeded" => IndexRunStatus::Succeeded,
                "failed" => IndexRunStatus::Failed,
                "cancelled" => IndexRunStatus::Cancelled,
                "interrupted" => IndexRunStatus::Interrupted,
                "aborted" => IndexRunStatus::Aborted,
                other => {
                    return Err(McpError::invalid_params(
                        format!(
                            "unknown status: `{other}`. Valid statuses: queued, running, succeeded, failed, cancelled, interrupted, aborted"
                        ),
                        None,
                    ));
                }
            };
            Some(parsed)
        } else {
            None
        };

        let reports = self
            .application
            .run_manager()
            .list_runs_with_health(input.repo_id.as_deref(), status_filter.as_ref())
            .map_err(to_mcp_error)?;

        let json = serde_json::to_string_pretty(&reports).map_err(|e| {
            McpError::internal_error(format!("failed to serialize run status reports: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Cancel an active indexing run. Returns the updated run status report. If the run is already terminal, returns the current status without modification. Parameters: run_id (string, required)."
    )]
    fn cancel_index_run(
        &self,
        params: Parameters<CancelIndexRunInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        let report = self
            .application
            .run_manager()
            .cancel_run(&input.run_id)
            .map_err(to_mcp_error)?;

        let json = serde_json::to_string_pretty(&report).map_err(|e| {
            McpError::internal_error(format!("failed to serialize run status report: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Create a checkpoint for an active indexing run. Persists current progress so interrupted work can later resume. Returns the checkpoint details. Fails if the run is not active or has no committed work yet. Parameters: run_id (string, required)."
    )]
    fn checkpoint_now(
        &self,
        params: Parameters<CheckpointNowInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        let checkpoint = self
            .application
            .run_manager()
            .checkpoint_run(&input.run_id)
            .map_err(to_mcp_error)?;

        let json = serde_json::to_string(&checkpoint).map_err(|e| {
            McpError::internal_error(format!("failed to serialize checkpoint: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Attempt to resume an interrupted indexing run from its last durable checkpoint. Returns a structured outcome indicating whether the run resumed or why resume was rejected, including the next safe action. Non-blocking: on success it returns immediately with the managed run reference. Parameters: run_id (string, required), repo_root (string, required — absolute path to repository)."
    )]
    fn resume_index_run(
        &self,
        params: Parameters<ResumeIndexRunInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        let repo_root = std::path::PathBuf::from(&input.repo_root);

        let outcome = self
            .application
            .resume_index_run(&input.run_id, repo_root)
            .map_err(to_mcp_error)?;

        let json = serde_json::to_string_pretty(&outcome).map_err(|e| {
            McpError::internal_error(format!("failed to serialize resume run outcome: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Re-index a previously indexed repository. Creates a new indexing run with mode 'reindex', linking to the prior completed run for traceability. Prior state remains inspectable. Behaves idempotently on replay. Parameters: repo_id (string, required), repo_root (string, required — absolute path to repository), workspace_id (string, optional), reason (string, optional description of why re-indexing)."
    )]
    fn reindex_repository(
        &self,
        params: Parameters<ReindexRepositoryInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        let repo_root = std::path::PathBuf::from(&input.repo_root);

        let run = self
            .application
            .reindex_repository(
                &input.repo_id,
                input.workspace_id.as_deref(),
                input.reason.as_deref(),
                repo_root,
            )
            .map_err(to_mcp_error)?;

        let json = serde_json::to_string_pretty(&run).map_err(|e| {
            McpError::internal_error(format!("failed to serialize reindex run: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Invalidate indexed state for a repository so it is no longer treated as trusted. Use when indexed state should not be served to retrieval flows. Returns the invalidation result with guidance for recovery (re-index or repair). Parameters: repo_id (string, required), workspace_id (string, optional), reason (string, optional description of why invalidation is needed)."
    )]
    fn invalidate_indexed_state(
        &self,
        params: Parameters<InvalidateIndexedStateInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        let result = self
            .application
            .invalidate_repository(
                &input.repo_id,
                input.workspace_id.as_deref(),
                input.reason.as_deref(),
            )
            .map_err(to_mcp_error)?;

        let json = serde_json::to_string_pretty(&result).map_err(|e| {
            McpError::internal_error(
                format!("failed to serialize invalidation result: {e}"),
                None,
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Search indexed repository content by text. Returns matching code locations with line context, scoped to the specified repository. Results include provenance metadata (run_id, committed_at_unix_ms) for staleness assessment. Parameters: repo_id (string, required), query (string, required — non-empty search text)."
    )]
    fn search_text(
        &self,
        params: Parameters<SearchTextInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        require_non_empty(&input.repo_id, "repo_id")?;
        require_non_empty(&input.query, "query")?;

        let result = self
            .application
            .search_text(&input.repo_id, &input.query)
            .map_err(to_mcp_error)?;
        let json = serde_json::to_string_pretty(&result).map_err(|e| {
            McpError::internal_error(format!("failed to serialize search results: {e}"), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Search indexed repository symbols by name. Returns matching symbol metadata (name, kind, file path, line range, depth) with coverage transparency. Uses case-insensitive substring matching. Parameters: repo_id (string, required), query (string, required — non-empty search text), kind_filter (string, optional: function|method|class|struct|enum|interface|module|constant|variable|type|trait|impl|other)."
    )]
    fn search_symbols(
        &self,
        params: Parameters<SearchSymbolsInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        require_non_empty(&input.repo_id, "repo_id")?;
        require_non_empty(&input.query, "query")?;
        let kind_filter = parse_kind_filter_str(input.kind_filter.as_deref())?;

        let result = self
            .application
            .search_symbols(&input.repo_id, &input.query, kind_filter)
            .map_err(to_mcp_error)?;
        let json = serde_json::to_string_pretty(&result).map_err(|e| {
            McpError::internal_error(
                format!("failed to serialize symbol search results: {e}"),
                None,
            )
        })?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Retrieve the structural outline (symbol tree) for a specific file in an indexed repository. Returns symbol metadata including name, kind, line ranges, depth, and document order. Distinguishes files with no symbols from files with unsupported languages. Parameters: repo_id (string, required), relative_path (string, required — file path relative to repository root)."
    )]
    fn get_file_outline(
        &self,
        params: Parameters<GetFileOutlineInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        require_non_empty(&input.repo_id, "repo_id")?;
        require_non_empty(&input.relative_path, "relative_path")?;

        let result = self
            .application
            .get_file_outline(&input.repo_id, &input.relative_path)
            .map_err(to_mcp_error)?;
        let json = serde_json::to_string_pretty(&result).map_err(|e| {
            McpError::internal_error(format!("failed to serialize file outline: {e}"), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Retrieve the structural overview of all files in an indexed repository. Returns file-level metadata (path, language, byte size, symbol count, status) with coverage statistics distinguishing files with symbols, without symbols, quarantined, and failed. Parameters: repo_id (string, required)."
    )]
    fn get_repo_outline(
        &self,
        params: Parameters<GetRepoOutlineInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        require_non_empty(&input.repo_id, "repo_id")?;

        let result = self
            .application
            .get_repo_outline(&input.repo_id)
            .map_err(to_mcp_error)?;
        let json = serde_json::to_string_pretty(&result).map_err(|e| {
            McpError::internal_error(format!("failed to serialize repo outline: {e}"), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Retrieve verified source code for a specific symbol from an indexed repository. Returns the exact source text with byte-exact verification against stored content. Verification ensures blob integrity (content hash match), span validity, and raw source fidelity. Parameters: repo_id (string, required), relative_path (string, required — file path relative to repository root), symbol_name (string, required — exact symbol name to retrieve), kind_filter (string, optional: function|method|class|struct|enum|interface|module|constant|variable|type|trait|impl|other)."
    )]
    fn get_symbol(
        &self,
        params: Parameters<GetSymbolInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        require_non_empty(&input.repo_id, "repo_id")?;
        require_non_empty(&input.relative_path, "relative_path")?;
        require_non_empty(&input.symbol_name, "symbol_name")?;
        let kind_filter = parse_kind_filter_str(input.kind_filter.as_deref())?;

        let result = self
            .application
            .get_symbol(&input.repo_id, &input.relative_path, &input.symbol_name, kind_filter)
            .map_err(to_mcp_error)?;
        let json = serde_json::to_string_pretty(&result).map_err(|e| {
            McpError::internal_error(format!("failed to serialize get_symbol result: {e}"), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Retrieve verified source code for multiple symbols or raw code slices from an indexed repository in a single request. Each item is verified independently — one failure does not affect others. Returns per-item outcomes with trust and provenance metadata. Parameters: repo_id (string, required), targets (array, preferred — ordered items with request_type=symbol or request_type=code_slice), or legacy symbols (array of symbol requests). Maximum 50 items per request."
    )]
    fn get_symbols(
        &self,
        params: Parameters<GetSymbolsInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        require_non_empty(&input.repo_id, "repo_id")?;

        let requests = if let Some(ref targets_array) = input.targets {
            parse_batch_targets(targets_array)?
        } else if let Some(ref symbols_array) = input.symbols {
            parse_legacy_symbol_targets(symbols_array)?
        } else {
            return Err(McpError::invalid_params(
                "missing required parameter: targets or symbols",
                None,
            ));
        };

        let result = self
            .application
            .get_symbols(&input.repo_id, &requests)
            .map_err(to_mcp_error)?;
        let json = serde_json::to_string_pretty(&result).map_err(|e| {
            McpError::internal_error(format!("failed to serialize get_symbols result: {e}"), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Trigger deterministic repair for suspect, stale, quarantined, or incomplete indexed state. Parameters: repository_id (string, required), repo_root (string, required — absolute path to repository), scope (string, optional: repository|run|file, default: repository), run_id (string, required when scope is run or file), relative_path (string, required when scope is file)."
    )]
    fn repair_index(
        &self,
        params: Parameters<RepairIndexInput>,
    ) -> Result<CallToolResult, McpError> {
        use crate::domain::RepairScope;

        let input = params.0;

        let scope_str = input.scope.as_deref().unwrap_or("repository");

        let scope = match scope_str {
            "repository" => RepairScope::Repository,
            "run" => {
                let run_id = input.run_id.ok_or_else(|| {
                    McpError::invalid_params(
                        "missing required parameter: run_id (required when scope is run)",
                        None,
                    )
                })?;
                RepairScope::Run {
                    run_id,
                }
            }
            "file" => {
                let run_id = input.run_id.ok_or_else(|| {
                    McpError::invalid_params(
                        "missing required parameter: run_id (required when scope is file)",
                        None,
                    )
                })?;
                let relative_path = input.relative_path.ok_or_else(|| {
                    McpError::invalid_params(
                        "missing required parameter: relative_path (required when scope is file)",
                        None,
                    )
                })?;
                RepairScope::File {
                    run_id,
                    relative_path,
                }
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("invalid scope '{other}'; expected repository, run, or file"),
                    None,
                ));
            }
        };

        let repo_root = std::path::PathBuf::from(&input.repo_root);

        let result = self
            .application
            .repair_repository(&input.repository_id, scope, repo_root)
            .map_err(to_mcp_error)?;

        let json = serde_json::to_string_pretty(&result).map_err(|e| {
            McpError::internal_error(format!("failed to serialize repair result: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Inspect repository health and repair-required conditions. Reports explicit health status, structured action classification (condition, action_required, next_action guidance), file-level health, run context, and recent repair history. The classification field explicitly distinguishes action-required states from normal health. Parameters: repository_id (string, required)."
    )]
    fn inspect_repository_health(
        &self,
        params: Parameters<InspectRepositoryHealthInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        let result = self
            .application
            .inspect_repository_health(&input.repository_id)
            .map_err(to_mcp_error)?;

        let json = serde_json::to_string_pretty(&result).map_err(|e| {
            McpError::internal_error(
                format!("failed to serialize health inspection result: {e}"),
                None,
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Retrieve operational history for a repository. Returns time-ordered events covering run transitions, checkpoints, repairs, integrity changes, and startup sweeps. Parameters: repository_id (string, required), category (string, optional — filter by event name prefix e.g. 'run', 'repair', 'integrity'), since_unix_ms (number, optional — only events at or after this timestamp), limit (number, optional — max events to return, capped at 200)."
    )]
    fn get_operational_history(
        &self,
        params: Parameters<GetOperationalHistoryInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = params.0;

        let category = input.category;
        let since_unix_ms = input.since_unix_ms;
        let limit = input
            .limit
            .map(|l| std::cmp::min(l as usize, 200));

        let filter = crate::domain::OperationalEventFilter {
            category,
            since_unix_ms,
            limit: Some(limit.unwrap_or(50)),
        };

        let events = self
            .application
            .get_operational_history(&input.repository_id, &filter)
            .map_err(to_mcp_error)?;

        let json = serde_json::to_string_pretty(&events).map_err(|e| {
            McpError::internal_error(
                format!("failed to serialize operational history: {e}"),
                None,
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for TokenizorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::from_build_env())
        .with_instructions(
            "tokenizor_agentic_mcp is a Rust-native MCP server for code indexing and trusted retrieval. Retrieval tools (search_text, search_symbols, get_file_outline, get_repo_outline, get_symbol, get_symbols) provide verified code discovery with explicit trust and provenance metadata. get_symbol performs byte-exact verification against stored content before serving trusted source. Use get_symbols to retrieve multiple symbols or raw code slices in a single request for efficiency. Each item is verified independently — mixed outcomes are reported explicitly, including missing items. Prefer the targets parameter: an ordered array of objects with request_type=symbol or request_type=code_slice. Symbol targets use relative_path (required), symbol_name (required), and kind_filter (optional). Code-slice targets use relative_path (required) and byte_range ([start, end], required). The legacy symbols parameter remains accepted for symbol-only batches. Maximum 50 items per request. Request-level gating applies to the entire batch. Blocked or quarantined results include a next_action field indicating the recommended resolution (reindex, repair, wait, resolve_context). Repositories in quarantined state reject all retrieval requests with actionable guidance. Indexing tools manage durable run lifecycle. All retrieval tools require a repo_id parameter identifying the target repository.",
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let run_ids = self.application.run_manager().list_recent_run_ids(10);
        let resources = run_ids
            .iter()
            .map(|id| {
                Annotated::new(
                    RawResource {
                        uri: format!("{}{}{}", RUN_STATUS_URI_PREFIX, id, RUN_STATUS_URI_SUFFIX),
                        name: format!("Run {} Status", id),
                        title: None,
                        description: Some(format!("Status and health for indexing run {}", id)),
                        mime_type: Some("application/json".to_string()),
                        size: None,
                        icons: None,
                        meta: None,
                    },
                    None,
                )
            })
            .collect();
        Ok(ListResourcesResult::with_all_items(resources))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let run_id = parse_run_id_from_uri(&request.uri)?;
        let report = self
            .application
            .run_manager()
            .inspect_run(&run_id)
            .map_err(to_mcp_error)?;
        let json = serde_json::to_string_pretty(&report).map_err(|e| {
            McpError::internal_error(format!("failed to serialize run status report: {e}"), None)
        })?;
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            json,
            request.uri,
        )]))
    }
}

fn to_mcp_error(error: TokenizorError) -> McpError {
    match error {
        TokenizorError::Config(message) | TokenizorError::InvalidArgument(message) => {
            McpError::invalid_params(message, None)
        }
        TokenizorError::ConflictingReplay(message) => McpError::invalid_params(
            format!(
                "conflicting replay: {message} — retry with identical inputs or use a new idempotency key"
            ),
            None,
        ),
        TokenizorError::InvalidOperation(message) => {
            McpError::invalid_params(format!("invalid operation: {message}"), None)
        }
        TokenizorError::NotFound(message) => McpError::invalid_params(message, None),
        TokenizorError::Integrity(message) => {
            McpError::internal_error(format!("integrity violation: {message}"), None)
        }
        TokenizorError::Storage(message) => McpError::internal_error(message, None),
        TokenizorError::ControlPlane(message) => McpError::internal_error(message, None),
        TokenizorError::Io { path, source } => {
            McpError::internal_error(format!("i/o error at `{}`: {source}", path.display()), None)
        }
        TokenizorError::Serialization(message) => McpError::internal_error(message, None),
        TokenizorError::RequestGated { gate_error } => {
            McpError::invalid_params(format!("request gated: {gate_error}"), None)
        }
    }
}

fn parse_run_id_from_uri(uri: &str) -> Result<String, McpError> {
    let stripped = uri
        .strip_prefix(RUN_STATUS_URI_PREFIX)
        .and_then(|s| s.strip_suffix(RUN_STATUS_URI_SUFFIX))
        .ok_or_else(|| {
            McpError::invalid_params(
                format!(
                    "invalid resource URI: expected {}{{run_id}}{}",
                    RUN_STATUS_URI_PREFIX, RUN_STATUS_URI_SUFFIX
                ),
                None,
            )
        })?;
    if stripped.is_empty() {
        return Err(McpError::invalid_params(
            "invalid resource URI: run_id is empty",
            None,
        ));
    }
    Ok(stripped.to_string())
}

const MAX_BATCH_SIZE: usize = 50;

fn parse_batch_targets(
    targets_array: &[serde_json::Value],
) -> Result<Vec<BatchRetrievalRequest>, McpError> {
    validate_batch_size(targets_array.len())?;

    let mut requests = Vec::with_capacity(targets_array.len());
    for (i, item) in targets_array.iter().enumerate() {
        let obj = item.as_object().ok_or_else(|| {
            McpError::invalid_params(
                format!("targets[{i}]: expected object with request_type"),
                None,
            )
        })?;
        let request_type = obj
            .get("request_type")
            .and_then(|value| value.as_str())
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!(
                        "targets[{i}]: missing or invalid `request_type` (expected `symbol` or `code_slice`)"
                    ),
                    None,
                )
            })?;
        let relative_path = required_batch_string_field(obj, "targets", i, "relative_path")?;

        match request_type {
            "symbol" => {
                let symbol_name = required_batch_string_field(obj, "targets", i, "symbol_name")?;
                let kind_filter = parse_kind_filter_value(obj.get("kind_filter")).map_err(|err| {
                    McpError::invalid_params(
                        format!(
                            "targets[{i}]: invalid `kind_filter`: {}. Valid kinds: {VALID_KIND_FILTERS}",
                            err.message
                        ),
                        None,
                    )
                })?;
                requests.push(BatchRetrievalRequest::Symbol {
                    relative_path,
                    symbol_name,
                    kind_filter,
                });
            }
            "code_slice" => {
                let byte_range = required_byte_range_field(obj, "targets", i)?;
                requests.push(BatchRetrievalRequest::CodeSlice {
                    relative_path,
                    byte_range,
                });
            }
            other => {
                return Err(McpError::invalid_params(
                    format!(
                        "targets[{i}]: unknown `request_type` `{other}` (expected `symbol` or `code_slice`)"
                    ),
                    None,
                ));
            }
        }
    }

    Ok(requests)
}

fn parse_legacy_symbol_targets(
    symbols_array: &[serde_json::Value],
) -> Result<Vec<BatchRetrievalRequest>, McpError> {
    validate_batch_size(symbols_array.len())?;

    let mut requests = Vec::with_capacity(symbols_array.len());
    for (i, item) in symbols_array.iter().enumerate() {
        let obj = item.as_object().ok_or_else(|| {
            McpError::invalid_params(
                format!("symbols[{i}]: expected object with relative_path and symbol_name"),
                None,
            )
        })?;

        let relative_path = required_batch_string_field(obj, "symbols", i, "relative_path")?;
        let symbol_name = required_batch_string_field(obj, "symbols", i, "symbol_name")?;
        let kind_filter = parse_kind_filter_value(obj.get("kind_filter")).map_err(|err| {
            McpError::invalid_params(
                format!(
                    "symbols[{i}]: invalid `kind_filter`: {}. Valid kinds: {VALID_KIND_FILTERS}",
                    err.message
                ),
                None,
            )
        })?;

        requests.push(BatchRetrievalRequest::Symbol {
            relative_path,
            symbol_name,
            kind_filter,
        });
    }

    Ok(requests)
}

fn required_batch_string_field(
    obj: &serde_json::Map<String, serde_json::Value>,
    collection: &str,
    index: usize,
    field: &str,
) -> Result<String, McpError> {
    obj.get(field)
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            McpError::invalid_params(
                format!("{collection}[{index}]: missing or empty `{field}`"),
                None,
            )
        })
}

fn required_byte_range_field(
    obj: &serde_json::Map<String, serde_json::Value>,
    collection: &str,
    index: usize,
) -> Result<(u32, u32), McpError> {
    let byte_range = obj.get("byte_range").ok_or_else(|| {
        McpError::invalid_params(
            format!("{collection}[{index}]: missing required `byte_range`"),
            None,
        )
    })?;
    let range_array = byte_range.as_array().ok_or_else(|| {
        McpError::invalid_params(
            format!("{collection}[{index}]: invalid `byte_range`: expected [start, end]"),
            None,
        )
    })?;
    if range_array.len() != 2 {
        return Err(McpError::invalid_params(
            format!("{collection}[{index}]: invalid `byte_range`: expected exactly 2 integers"),
            None,
        ));
    }

    let start = range_array[0].as_u64().ok_or_else(|| {
        McpError::invalid_params(
            format!("{collection}[{index}]: invalid `byte_range[0]`: expected unsigned integer"),
            None,
        )
    })?;
    let end = range_array[1].as_u64().ok_or_else(|| {
        McpError::invalid_params(
            format!("{collection}[{index}]: invalid `byte_range[1]`: expected unsigned integer"),
            None,
        )
    })?;

    let start = u32::try_from(start).map_err(|_| {
        McpError::invalid_params(
            format!("{collection}[{index}]: invalid `byte_range[0]`: exceeds u32"),
            None,
        )
    })?;
    let end = u32::try_from(end).map_err(|_| {
        McpError::invalid_params(
            format!("{collection}[{index}]: invalid `byte_range[1]`: exceeds u32"),
            None,
        )
    })?;

    Ok((start, end))
}

fn validate_batch_size(size: usize) -> Result<(), McpError> {
    if size > MAX_BATCH_SIZE {
        return Err(McpError::invalid_params(
            format!("batch size {size} exceeds maximum of {MAX_BATCH_SIZE} items per request"),
            None,
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_run_id_from_uri_valid_uuid() {
        let uri = "tokenizor://runs/550e8400-e29b-41d4-a716-446655440000/status";
        let result = parse_run_id_from_uri(uri).unwrap();
        assert_eq!(result, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn test_parse_run_id_from_uri_simple_id() {
        let uri = "tokenizor://runs/run-123/status";
        let result = parse_run_id_from_uri(uri).unwrap();
        assert_eq!(result, "run-123");
    }

    #[test]
    fn test_parse_run_id_from_uri_missing_prefix() {
        let uri = "invalid://runs/abc/status";
        let result = parse_run_id_from_uri(uri);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_run_id_from_uri_missing_suffix() {
        let uri = "tokenizor://runs/abc";
        let result = parse_run_id_from_uri(uri);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_run_id_from_uri_empty_run_id() {
        let uri = "tokenizor://runs//status";
        let result = parse_run_id_from_uri(uri);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_run_id_from_uri_completely_invalid() {
        let result = parse_run_id_from_uri("garbage");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_run_id_from_uri_empty_string() {
        let result = parse_run_id_from_uri("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_run_id_from_uri_only_prefix() {
        let result = parse_run_id_from_uri("tokenizor://runs/");
        assert!(result.is_err());
    }

    #[test]
    fn test_run_status_uri_round_trip() {
        let run_id = "test-run-42";
        let uri = format!(
            "{}{}{}",
            RUN_STATUS_URI_PREFIX, run_id, RUN_STATUS_URI_SUFFIX
        );
        let parsed = parse_run_id_from_uri(&uri).unwrap();
        assert_eq!(parsed, run_id);
    }

    // --- kind_filter parsing tests ---

    #[test]
    fn test_parse_kind_filter_all_13_variants() {
        let cases = [
            ("function", SymbolKind::Function),
            ("method", SymbolKind::Method),
            ("class", SymbolKind::Class),
            ("struct", SymbolKind::Struct),
            ("enum", SymbolKind::Enum),
            ("interface", SymbolKind::Interface),
            ("module", SymbolKind::Module),
            ("constant", SymbolKind::Constant),
            ("variable", SymbolKind::Variable),
            ("type", SymbolKind::Type),
            ("trait", SymbolKind::Trait),
            ("impl", SymbolKind::Impl),
            ("other", SymbolKind::Other),
        ];
        assert_eq!(cases.len(), 13);
        for (input, expected) in cases {
            let value = serde_json::Value::String(input.to_string());
            let result = parse_kind_filter_value(Some(&value)).unwrap();
            assert_eq!(result, Some(expected), "failed for kind_filter: {input}");
        }
    }

    #[test]
    fn test_parse_kind_filter_rejects_unknown_value() {
        let value = serde_json::Value::String("unknown_kind".to_string());
        let err = parse_kind_filter_value(Some(&value)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown kind_filter: `unknown_kind`"));
        assert!(msg.contains("function"));
        assert!(msg.contains("other"));
    }

    #[test]
    fn test_parse_kind_filter_rejects_non_string_value() {
        let value = serde_json::json!(true);
        let err = parse_kind_filter_value(Some(&value)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid parameter `kind_filter`"));
        assert!(msg.contains("function"));
        assert!(msg.contains("other"));
    }

    #[test]
    fn test_parse_kind_filter_none_for_absent() {
        let result = parse_kind_filter_value(None).unwrap();
        assert_eq!(result, None);
    }

    // --- Input struct deserialization tests ---

    #[test]
    fn test_search_text_input_missing_repo_id() {
        let json = serde_json::json!({"query": "hello"});
        let result: Result<SearchTextInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_search_text_input_missing_query() {
        let json = serde_json::json!({"repo_id": "repo-1"});
        let result: Result<SearchTextInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_search_text_input_valid() {
        let json = serde_json::json!({"repo_id": "repo-1", "query": "hello"});
        let input: SearchTextInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.repo_id, "repo-1");
        assert_eq!(input.query, "hello");
    }

    #[test]
    fn test_search_symbols_input_missing_repo_id() {
        let json = serde_json::json!({"query": "hello"});
        let result: Result<SearchSymbolsInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_search_symbols_input_missing_query() {
        let json = serde_json::json!({"repo_id": "repo-1"});
        let result: Result<SearchSymbolsInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_search_symbols_input_valid_with_kind_filter() {
        let json = serde_json::json!({"repo_id": "repo-1", "query": "test", "kind_filter": "function"});
        let input: SearchSymbolsInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.repo_id, "repo-1");
        assert_eq!(input.query, "test");
        assert_eq!(input.kind_filter, Some("function".to_string()));
    }

    #[test]
    fn test_search_symbols_input_valid_without_kind_filter() {
        let json = serde_json::json!({"repo_id": "repo-1", "query": "test"});
        let input: SearchSymbolsInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.kind_filter, None);
    }

    #[test]
    fn test_get_file_outline_input_missing_repo_id() {
        let json = serde_json::json!({"relative_path": "src/main.rs"});
        let result: Result<GetFileOutlineInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_file_outline_input_missing_relative_path() {
        let json = serde_json::json!({"repo_id": "repo-1"});
        let result: Result<GetFileOutlineInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_repo_outline_input_missing_repo_id() {
        let json = serde_json::json!({});
        let result: Result<GetRepoOutlineInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_symbol_input_missing_repo_id() {
        let json = serde_json::json!({"relative_path": "src/main.rs", "symbol_name": "main"});
        let result: Result<GetSymbolInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_symbol_input_missing_relative_path() {
        let json = serde_json::json!({"repo_id": "repo-1", "symbol_name": "main"});
        let result: Result<GetSymbolInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_symbol_input_missing_symbol_name() {
        let json = serde_json::json!({"repo_id": "repo-1", "relative_path": "src/main.rs"});
        let result: Result<GetSymbolInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_symbol_input_valid_with_kind_filter() {
        let json = serde_json::json!({"repo_id": "repo-1", "relative_path": "src/main.rs", "symbol_name": "main", "kind_filter": "function"});
        let input: GetSymbolInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.repo_id, "repo-1");
        assert_eq!(input.relative_path, "src/main.rs");
        assert_eq!(input.symbol_name, "main");
        assert_eq!(input.kind_filter, Some("function".to_string()));
    }

    #[test]
    fn test_get_symbol_input_valid_without_kind_filter() {
        let json = serde_json::json!({"repo_id": "repo-1", "relative_path": "src/main.rs", "symbol_name": "main"});
        let input: GetSymbolInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.kind_filter, None);
    }

    #[test]
    fn test_get_symbols_input_missing_repo_id() {
        let json = serde_json::json!({"targets": []});
        let result: Result<GetSymbolsInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_symbols_input_valid_with_targets() {
        let json = serde_json::json!({"repo_id": "repo-1", "targets": [{"request_type": "symbol", "relative_path": "src/main.rs", "symbol_name": "main"}]});
        let input: GetSymbolsInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.repo_id, "repo-1");
        assert!(input.targets.is_some());
        assert!(input.symbols.is_none());
    }

    #[test]
    fn test_get_symbols_input_valid_with_symbols() {
        let json = serde_json::json!({"repo_id": "repo-1", "symbols": [{"relative_path": "src/main.rs", "symbol_name": "main"}]});
        let input: GetSymbolsInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.repo_id, "repo-1");
        assert!(input.targets.is_none());
        assert!(input.symbols.is_some());
    }

    #[test]
    fn test_index_folder_input_missing_repo_id() {
        let json = serde_json::json!({"repo_root": "/tmp/repo"});
        let result: Result<IndexFolderInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_index_folder_input_missing_repo_root() {
        let json = serde_json::json!({"repo_id": "repo-1"});
        let result: Result<IndexFolderInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_index_folder_input_valid_with_mode() {
        let json = serde_json::json!({"repo_id": "repo-1", "repo_root": "/tmp/repo", "mode": "incremental"});
        let input: IndexFolderInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.repo_id, "repo-1");
        assert_eq!(input.repo_root, "/tmp/repo");
        assert_eq!(input.mode, Some("incremental".to_string()));
    }

    #[test]
    fn test_index_folder_input_valid_without_mode() {
        let json = serde_json::json!({"repo_id": "repo-1", "repo_root": "/tmp/repo"});
        let input: IndexFolderInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.mode, None);
    }

    #[test]
    fn test_repair_index_input_missing_repository_id() {
        let json = serde_json::json!({"repo_root": "/tmp/repo"});
        let result: Result<RepairIndexInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_repair_index_input_missing_repo_root() {
        let json = serde_json::json!({"repository_id": "repo-1"});
        let result: Result<RepairIndexInput, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_repair_index_input_valid_with_scope() {
        let json = serde_json::json!({"repository_id": "repo-1", "repo_root": "/tmp/repo", "scope": "run", "run_id": "run-1"});
        let input: RepairIndexInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.repository_id, "repo-1");
        assert_eq!(input.scope, Some("run".to_string()));
        assert_eq!(input.run_id, Some("run-1".to_string()));
    }

    #[test]
    fn test_get_operational_history_input_valid() {
        let json = serde_json::json!({"repository_id": "repo-1", "category": "run", "since_unix_ms": 1000, "limit": 10});
        let input: GetOperationalHistoryInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.repository_id, "repo-1");
        assert_eq!(input.category, Some("run".to_string()));
        assert_eq!(input.since_unix_ms, Some(1000));
        assert_eq!(input.limit, Some(10));
    }

    #[test]
    fn test_get_operational_history_input_minimal() {
        let json = serde_json::json!({"repository_id": "repo-1"});
        let input: GetOperationalHistoryInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.repository_id, "repo-1");
        assert_eq!(input.category, None);
        assert_eq!(input.since_unix_ms, None);
        assert_eq!(input.limit, None);
    }

    // --- batch parsing tests (get_symbols helpers) ---

    #[test]
    fn test_get_symbols_legacy_symbols_array() {
        let symbols = serde_json::json!([
            {
                "relative_path": "src/main.rs",
                "symbol_name": "main",
                "kind_filter": "function"
            }
        ]);
        let symbols_array = symbols.as_array().unwrap();
        let result = parse_legacy_symbol_targets(symbols_array).unwrap();
        assert_eq!(
            result,
            vec![BatchRetrievalRequest::Symbol {
                relative_path: "src/main.rs".to_string(),
                symbol_name: "main".to_string(),
                kind_filter: Some(SymbolKind::Function),
            }]
        );
    }

    #[test]
    fn test_get_symbols_targets_with_code_slice() {
        let targets = serde_json::json!([
            {
                "request_type": "symbol",
                "relative_path": "src/main.rs",
                "symbol_name": "main"
            },
            {
                "request_type": "code_slice",
                "relative_path": "src/main.rs",
                "byte_range": [0, 12]
            }
        ]);
        let targets_array = targets.as_array().unwrap();
        let result = parse_batch_targets(targets_array).unwrap();
        assert_eq!(
            result,
            vec![
                BatchRetrievalRequest::Symbol {
                    relative_path: "src/main.rs".to_string(),
                    symbol_name: "main".to_string(),
                    kind_filter: None,
                },
                BatchRetrievalRequest::CodeSlice {
                    relative_path: "src/main.rs".to_string(),
                    byte_range: (0, 12),
                },
            ]
        );
    }

    #[test]
    fn test_get_symbols_rejects_invalid_code_slice_byte_range() {
        let targets = serde_json::json!([
            {
                "request_type": "code_slice",
                "relative_path": "src/main.rs",
                "byte_range": [0]
            }
        ]);
        let targets_array = targets.as_array().unwrap();
        let err = parse_batch_targets(targets_array).unwrap_err();
        assert!(err.message.contains("invalid `byte_range`"));
    }

    // --- non-empty string validation tests ---

    #[test]
    fn test_require_non_empty_rejects_empty() {
        let err = require_non_empty("", "repo_id").unwrap_err();
        assert!(err.message.contains("invalid parameter `repo_id`"));
    }

    #[test]
    fn test_require_non_empty_rejects_whitespace() {
        let err = require_non_empty("   ", "query").unwrap_err();
        assert!(err.message.contains("invalid parameter `query`"));
    }

    #[test]
    fn test_require_non_empty_accepts_valid() {
        assert!(require_non_empty("hello", "query").is_ok());
    }
}
