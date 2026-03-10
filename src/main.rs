use tokenizor_agentic_mcp::observability;

// TODO: rewrite in Plan 03 — v1 application layer removed, v2 MCP server not yet implemented

fn main() -> anyhow::Result<()> {
    observability::init_tracing()?;
    tracing::info!("tokenizor v2 — not yet implemented (see Plan 03)");
    Ok(())
}
