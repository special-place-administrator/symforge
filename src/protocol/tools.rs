/// Tool handler stub — Task 1 placeholder.
///
/// This minimal `#[tool_router]` impl makes `Self::tool_router()` available as a
/// `pub(crate)` function so `mod.rs` can call it in `TokenizorServer::new()`.
/// Task 2 will replace this with all 10 real tool handlers.
use rmcp::tool_router;

use super::TokenizorServer;

#[tool_router(vis = "pub(crate)")]
impl TokenizorServer {}
