use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};

use crate::domain::IndexRunMode;
use crate::{ApplicationContext, TokenizorError};

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
        description = "Start an indexing run for a repository. Returns the run ID immediately without blocking on the full indexing pipeline. Parameters: repo_id (string, required), mode (string, optional: full|incremental|repair|verify, defaults to full)."
    )]
    fn index_folder(&self, params: rmcp::model::JsonObject) -> Result<CallToolResult, McpError> {
        let repo_id = params
            .get("repo_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("missing required parameter: repo_id", None))?
            .to_string();

        let mode_str = params.get("mode").and_then(|v| v.as_str());
        let run_mode = match mode_str {
            Some("full") | None => IndexRunMode::Full,
            Some("incremental") => IndexRunMode::Incremental,
            Some("repair") => IndexRunMode::Repair,
            Some("verify") => IndexRunMode::Verify,
            Some(other) => {
                return Err(McpError::invalid_params(
                    format!("unknown indexing mode: `{other}`. Valid modes: full, incremental, repair, verify"),
                    None,
                ));
            }
        };

        let run = self
            .application
            .start_indexing(&repo_id, run_mode)
            .map_err(to_mcp_error)?;

        let payload = serde_json::to_string(&run).map_err(|error| {
            McpError::internal_error(
                format!("failed to serialize index run: {error}"),
                None,
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }
}

#[tool_handler]
impl ServerHandler for TokenizorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions(
                "tokenizor_agentic_mcp is a Rust-native MCP server for indexing and retrieval. This foundation slice exposes deployment-aware health while the durable SpacetimeDB control plane and local byte-exact CAS are brought online.",
            )
    }
}

fn to_mcp_error(error: TokenizorError) -> McpError {
    match error {
        TokenizorError::Config(message) | TokenizorError::InvalidArgument(message) => {
            McpError::invalid_params(message, None)
        }
        TokenizorError::NotFound(message) => McpError::invalid_params(message, None),
        TokenizorError::Integrity(message) => {
            McpError::internal_error(format!("integrity violation: {message}"), None)
        }
        TokenizorError::Storage(message) => McpError::internal_error(message, None),
        TokenizorError::ControlPlane(message) => McpError::internal_error(message, None),
        TokenizorError::Io { path, source } => McpError::internal_error(
            format!("i/o error at `{}`: {source}", path.display()),
            None,
        ),
        TokenizorError::Serialization(message) => McpError::internal_error(message, None),
    }
}
