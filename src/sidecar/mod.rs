pub mod handlers;
pub mod port_file;
pub mod router;
pub mod server;

pub use server::spawn_sidecar;

/// Handle returned by `spawn_sidecar`. Dropping this or sending on `shutdown_tx`
/// gracefully stops the background axum server and cleans up port/PID files.
pub struct SidecarHandle {
    /// The ephemeral port the sidecar bound to.
    pub port: u16,
    /// Send `()` on this channel to initiate graceful shutdown.
    pub shutdown_tx: tokio::sync::oneshot::Sender<()>,
}
