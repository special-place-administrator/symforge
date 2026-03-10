//! Axum router wiring all 5 sidecar endpoints.

use axum::{Router, routing::get};

use crate::live_index::store::SharedIndex;
use super::handlers;

/// Build the axum `Router` with all 5 GET routes, injecting `SharedIndex` as state.
///
/// Routes:
/// - `GET /health`          → `health_handler`
/// - `GET /outline`         → `outline_handler`
/// - `GET /impact`          → `impact_handler`
/// - `GET /symbol-context`  → `symbol_context_handler`
/// - `GET /repo-map`        → `repo_map_handler`
pub fn build_router(index: SharedIndex) -> Router {
    Router::new()
        .route("/health", get(handlers::health_handler))
        .route("/outline", get(handlers::outline_handler))
        .route("/impact", get(handlers::impact_handler))
        .route("/symbol-context", get(handlers::symbol_context_handler))
        .route("/repo-map", get(handlers::repo_map_handler))
        .with_state(index)
}
