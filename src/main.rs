use std::sync::{Arc, Mutex};

use tokenizor_agentic_mcp::{discovery, live_index, observability, protocol, watcher};
use rmcp::{serve_server, transport};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    observability::init_tracing()?;

    // INFR-02: Auto-index on startup (configurable via TOKENIZOR_AUTO_INDEX)
    let should_auto_index = std::env::var("TOKENIZOR_AUTO_INDEX")
        .map(|v| v != "false")
        .unwrap_or(true);

    let (index, project_name, watcher_root) = if should_auto_index {
        let root = discovery::find_git_root();
        tracing::info!(root = %root.display(), "auto-indexing from project root");

        let index = live_index::LiveIndex::load(&root)?;
        let guard = index.read().expect("lock poisoned");

        match guard.index_state() {
            live_index::IndexState::Ready => {
                let stats = guard.health_stats();
                tracing::info!(
                    files = stats.file_count,
                    symbols = stats.symbol_count,
                    parsed = stats.parsed_count,
                    partial = stats.partial_parse_count,
                    failed = stats.failed_count,
                    duration_ms = stats.load_duration.as_millis() as u64,
                    "LiveIndex ready"
                );
            }
            live_index::IndexState::CircuitBreakerTripped { ref summary } => {
                tracing::error!(%summary, "circuit breaker tripped — index degraded");
            }
            _ => {}
        }
        drop(guard);

        let name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
            .to_string();

        (index, name, Some(root))
    } else {
        tracing::info!("TOKENIZOR_AUTO_INDEX=false — starting with empty index");
        (live_index::LiveIndex::empty(), "project".to_string(), None)
    };

    // Spawn file watcher after initial load (only when auto-index is enabled).
    let watcher_info = Arc::new(Mutex::new(watcher::WatcherInfo::default()));

    if let Some(ref root) = watcher_root {
        let watcher_index = Arc::clone(&index);
        let watcher_root_clone = root.clone();
        let watcher_info_clone = Arc::clone(&watcher_info);
        tokio::spawn(async move {
            watcher::run_watcher(watcher_root_clone, watcher_index, watcher_info_clone).await;
        });
        tracing::info!("file watcher started");
    }

    // Create MCP server and serve on stdio transport
    let server = protocol::TokenizorServer::new(index, project_name, watcher_info, watcher_root);
    tracing::info!("starting MCP server on stdio transport");
    let service = serve_server(server, transport::stdio()).await?;
    service.waiting().await?;

    tracing::info!("MCP server shut down cleanly");
    Ok(())
}
