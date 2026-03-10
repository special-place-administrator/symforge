//! Sidecar server spawner.
//!
//! Binds to an OS-assigned ephemeral port, writes port/PID files,
//! and spawns an axum serve task with graceful shutdown support.

use tokio::net::TcpListener;
use tracing::info;

use crate::live_index::store::SharedIndex;
use super::{SidecarHandle, port_file, router};

/// Spawn the HTTP sidecar.
///
/// 1. Reads `TOKENIZOR_SIDECAR_BIND` env var (default `"127.0.0.1"`).
/// 2. Calls `port_file::check_stale(bind_host)` to clean up any stale files.
/// 3. Binds `TcpListener::bind("{bind_host}:0")` (OS assigns the port).
/// 4. Writes port and PID files via `port_file`.
/// 5. Builds the axum router via `router::build_router`.
/// 6. Spawns `axum::serve` with graceful shutdown wired to a oneshot channel.
/// 7. After the server completes, calls `port_file::cleanup_files()`.
/// 8. Returns `SidecarHandle { port, shutdown_tx }`.
pub async fn spawn_sidecar(
    index: SharedIndex,
    bind_host: &str,
) -> anyhow::Result<SidecarHandle> {
    // Allow overriding bind host via env var.
    let resolved_host = std::env::var("TOKENIZOR_SIDECAR_BIND")
        .unwrap_or_else(|_| bind_host.to_string());

    // Clean up stale files from a previous crashed sidecar.
    port_file::check_stale(&resolved_host);

    // Bind to an OS-assigned ephemeral port.
    let addr = format!("{resolved_host}:0");
    let listener = TcpListener::bind(&addr).await?;
    let port = listener.local_addr()?.port();

    // Write port and PID files so hook scripts can locate the sidecar.
    port_file::write_port_file(port)?;
    port_file::write_pid_file(std::process::id())?;

    info!("sidecar listening on {resolved_host}:{port}");

    // Build the router.
    let app = router::build_router(index);

    // Create graceful shutdown channel.
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // Spawn the server task.
    tokio::spawn(async move {
        let shutdown_signal = async move {
            let _ = shutdown_rx.await;
        };

        if let Err(e) = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await
        {
            tracing::error!("sidecar server error: {e}");
        }

        // Clean up port/PID files after shutdown.
        port_file::cleanup_files();
        tracing::info!("sidecar shut down, port/PID files cleaned up");
    });

    Ok(SidecarHandle { port, shutdown_tx })
}
