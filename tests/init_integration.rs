/// Integration tests for `tokenizor init` — proves idempotent hook installation.
///
/// Tests use a temporary directory in place of `~/.claude/settings.json` via the
/// `merge_hooks_into_settings(settings_path, binary_path)` public function.
use tempfile::TempDir;
use tokenizor_agentic_mcp::cli::init::merge_hooks_into_settings;

const FAKE_BINARY: &str = "/usr/local/bin/tokenizor";

fn fake_binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(FAKE_BINARY)
}

/// Read settings.json from the temp dir.
fn read_settings(dir: &TempDir) -> serde_json::Value {
    let path = dir.path().join("settings.json");
    let raw = std::fs::read_to_string(&path).expect("settings.json must exist");
    serde_json::from_str(&raw).expect("settings.json must be valid JSON")
}

// ---------------------------------------------------------------------------
// test_init_writes_hooks: init produces correct hook entries
// ---------------------------------------------------------------------------

#[test]
fn test_init_writes_hooks() {
    let dir = TempDir::new().unwrap();
    let settings_path = dir.path().join("settings.json");

    merge_hooks_into_settings(&settings_path, &fake_binary_path())
        .expect("merge_hooks_into_settings must succeed");

    let settings = read_settings(&dir);

    let post = settings["hooks"]["PostToolUse"]
        .as_array()
        .expect("PostToolUse must be an array");
    let session = settings["hooks"]["SessionStart"]
        .as_array()
        .expect("SessionStart must be an array");

    assert_eq!(post.len(), 1, "PostToolUse must have 1 entry (single stdin-routed entry)");
    assert_eq!(session.len(), 1, "SessionStart must have 1 entry");

    // Verify each entry has the correct binary path embedded.
    let all_commands: Vec<&str> = post
        .iter()
        .chain(session.iter())
        .flat_map(|e| e["hooks"].as_array().unwrap())
        .filter_map(|h| h["command"].as_str())
        .collect();

    for cmd in &all_commands {
        assert!(
            cmd.contains("tokenizor hook"),
            "command must contain 'tokenizor hook': {cmd}"
        );
        assert!(
            cmd.contains(FAKE_BINARY),
            "command must contain binary path {FAKE_BINARY}: {cmd}"
        );
    }

    // Verify the PostToolUse matcher covers all tools.
    let matcher = post[0]["matcher"].as_str().unwrap();
    assert_eq!(matcher, "Read|Edit|Write|Grep", "matcher must cover all tools");

    // Verify session-start hook is present.
    let has_session = all_commands.iter().any(|c| c.ends_with("hook session-start"));
    assert!(has_session, "SessionStart hook must be present");
}

// ---------------------------------------------------------------------------
// test_init_idempotent: running init twice produces identical output
// ---------------------------------------------------------------------------

#[test]
fn test_init_idempotent() {
    let dir = TempDir::new().unwrap();
    let settings_path = dir.path().join("settings.json");

    merge_hooks_into_settings(&settings_path, &fake_binary_path())
        .expect("first merge must succeed");
    let after_first = std::fs::read_to_string(&settings_path).unwrap();

    merge_hooks_into_settings(&settings_path, &fake_binary_path())
        .expect("second merge must succeed");
    let after_second = std::fs::read_to_string(&settings_path).unwrap();

    assert_eq!(
        after_first, after_second,
        "running merge_hooks_into_settings twice must produce identical output (idempotent)"
    );

    // Also assert entry count didn't grow.
    let settings = read_settings(&dir);
    let post_count = settings["hooks"]["PostToolUse"].as_array().unwrap().len();
    assert_eq!(post_count, 1, "second merge must not add duplicate entries");
}

// ---------------------------------------------------------------------------
// test_init_preserves_other_hooks: non-tokenizor hooks are preserved
// ---------------------------------------------------------------------------

#[test]
fn test_init_preserves_other_hooks() {
    let dir = TempDir::new().unwrap();
    let settings_path = dir.path().join("settings.json");

    // Start with an existing non-tokenizor hook.
    let initial = serde_json::json!({
        "hooks": {
            "PostToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": "/some/other/hook bash", "timeout": 10}]
                }
            ]
        }
    });
    std::fs::write(&settings_path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();

    merge_hooks_into_settings(&settings_path, &fake_binary_path())
        .expect("merge must succeed");

    let settings = read_settings(&dir);
    let post = settings["hooks"]["PostToolUse"]
        .as_array()
        .expect("PostToolUse must be an array");

    // 1 existing + 1 tokenizor = 2 total.
    assert_eq!(post.len(), 2, "existing hook + 1 tokenizor hook = 2 entries");

    // Non-tokenizor hook must still be present.
    let has_bash_hook = post.iter().any(|e| {
        e["hooks"][0]["command"]
            .as_str()
            .map(|c| c == "/some/other/hook bash")
            .unwrap_or(false)
    });
    assert!(has_bash_hook, "non-tokenizor hook must be preserved after merge");
}

// ---------------------------------------------------------------------------
// test_init_registers_mcp_server: MCP entry written to claude.json
// ---------------------------------------------------------------------------

#[test]
fn test_init_registers_mcp_server() {
    let dir = TempDir::new().unwrap();
    let claude_json_path = dir.path().join(".claude.json");
    let binary_path = "/usr/local/bin/tokenizor";

    tokenizor_agentic_mcp::cli::init::register_mcp_server(&claude_json_path, binary_path)
        .expect("register_mcp_server must succeed");

    let raw = std::fs::read_to_string(&claude_json_path).unwrap();
    let config: serde_json::Value = serde_json::from_str(&raw).unwrap();

    let tok = &config["mcpServers"]["tokenizor"];
    assert_eq!(tok["type"], "stdio");
    // On Windows, forward slashes are converted to backslashes for native process spawning.
    let expected_command = if cfg!(windows) {
        binary_path.replace('/', "\\")
    } else {
        binary_path.to_string()
    };
    assert_eq!(tok["command"], expected_command);
}

#[test]
fn test_init_mcp_registration_idempotent() {
    let dir = TempDir::new().unwrap();
    let claude_json_path = dir.path().join(".claude.json");
    let binary_path = "/usr/local/bin/tokenizor";

    tokenizor_agentic_mcp::cli::init::register_mcp_server(&claude_json_path, binary_path).unwrap();
    let first = std::fs::read_to_string(&claude_json_path).unwrap();

    tokenizor_agentic_mcp::cli::init::register_mcp_server(&claude_json_path, binary_path).unwrap();
    let second = std::fs::read_to_string(&claude_json_path).unwrap();

    assert_eq!(first, second, "register_mcp_server must be idempotent");
}

#[test]
fn test_init_mcp_registration_preserves_other_servers() {
    let dir = TempDir::new().unwrap();
    let claude_json_path = dir.path().join(".claude.json");

    // Pre-populate with another MCP server.
    let initial = serde_json::json!({
        "mcpServers": {
            "other-server": {"type": "stdio", "command": "other-binary"}
        }
    });
    std::fs::write(&claude_json_path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();

    tokenizor_agentic_mcp::cli::init::register_mcp_server(&claude_json_path, "/usr/local/bin/tokenizor").unwrap();

    let raw = std::fs::read_to_string(&claude_json_path).unwrap();
    let config: serde_json::Value = serde_json::from_str(&raw).unwrap();

    assert!(config["mcpServers"]["other-server"].is_object(), "other MCP server must be preserved");
    assert!(config["mcpServers"]["tokenizor"].is_object(), "tokenizor must be added");
}
