//! Sidecar server spawner — stub, fully implemented in Task 2.

use crate::live_index::store::SharedIndex;
use super::SidecarHandle;

/// Spawn the HTTP sidecar, bind to an ephemeral port, write port/PID files,
/// and return a `SidecarHandle`.
pub async fn spawn_sidecar(
    _index: SharedIndex,
    _bind_host: &str,
) -> anyhow::Result<SidecarHandle> {
    unimplemented!("spawn_sidecar: implemented in Task 2")
}
