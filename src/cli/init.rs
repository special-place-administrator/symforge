//! `symforge init` command — client-aware Claude/Codex/Gemini/Kilo Code configuration.
//!
//! Strategy:
//! 1. Discover the absolute path of the running symforge binary.
//! 2. Configure Claude, Codex, Gemini, Kilo Code, or all based on the selected client target.
//! 3. For Claude, merge symforge hook entries into `~/.claude/settings.json`
//!    and register the MCP server in `~/.claude.json`.
//! 4. For Codex, register the MCP server in `~/.codex/config.toml`.
//! 5. For Kilo Code, register the MCP server in `.kilocode/mcp.json` (workspace-local).
//! 6. Create `.symforge/` in the current working directory (runtime needs it).
//!
//! Identification: any hook entry whose `hooks[].command` contains the substring
//! `"symforge hook"` or `"tokenizor hook"` (legacy) is considered a symforge-owned
//! entry and will be replaced.

use std::path::PathBuf;

use anyhow::Context;
use serde_json::{Value, json};
use toml_edit::{Array, DocumentMut, Item, Table, value};

use crate::cli::InitClient;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct InitPaths {
    claude_settings: PathBuf,
    claude_config: PathBuf,
    claude_memory: PathBuf,
    codex_config: PathBuf,
    codex_agents: PathBuf,
    gemini_settings: PathBuf,
    gemini_memory: PathBuf,
    kilo_vscode_config: PathBuf,
}

impl InitPaths {
    #[allow(dead_code)]

    fn from_home(home: &std::path::Path) -> Self {
        Self::from_home_and_working_dir(home, &std::env::current_dir().unwrap_or_default())
    }

    fn from_home_and_working_dir(home: &std::path::Path, working_dir: &std::path::Path) -> Self {
        Self {
            claude_settings: home.join(".claude").join("settings.json"),
            claude_config: home.join(".claude.json"),
            claude_memory: home.join(".claude").join("CLAUDE.md"),
            codex_config: home.join(".codex").join("config.toml"),
            codex_agents: home.join(".codex").join("AGENTS.md"),
            gemini_settings: home.join(".gemini").join("settings.json"),
            gemini_memory: home.join(".gemini").join("GEMINI.md"),
            kilo_vscode_config: working_dir.join(".kilocode").join("mcp.json"),
        }
    }
}

const CODEX_STARTUP_TIMEOUT_SEC: i64 = 30;
const CODEX_TOOL_TIMEOUT_SEC: i64 = 120;
const SYMFORGE_GUIDANCE_START: &str = "<!-- SYMFORGE START -->";
const SYMFORGE_GUIDANCE_END: &str = "<!-- SYMFORGE END -->";
/// Legacy marker strings for backward-compatible detection during upsert.
const LEGACY_GUIDANCE_START: &str = "<!-- TOKENIZOR START -->";
const LEGACY_GUIDANCE_END: &str = "<!-- TOKENIZOR END -->";

/// Entry point called by main.rs for `symforge init`.
pub fn run_init(client: InitClient) -> anyhow::Result<()> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    let working_dir =
        std::env::current_dir().context("cannot determine current working directory")?;
    let binary_path = discover_binary_path();

    run_init_with_context(client, &home, &working_dir, &binary_path)
}

/// Testable core for `symforge init` with injected paths.
pub fn run_init_with_context(
    client: InitClient,
    home_dir: &std::path::Path,
    working_dir: &std::path::Path,
    binary_path: &std::path::Path,
) -> anyhow::Result<()> {
    let paths = InitPaths::from_home_and_working_dir(home_dir, working_dir);
    let binary_path_str = binary_path.display().to_string();

    if matches!(client, InitClient::Claude | InitClient::All) {
        merge_hooks_into_settings(&paths.claude_settings, binary_path)?;
        eprintln!(
            "Claude hooks installed in {}",
            paths.claude_settings.display()
        );

        register_mcp_server(&paths.claude_config, &binary_path_str)?;
        eprintln!(
            "Claude MCP server registered in {}",
            paths.claude_config.display()
        );

        upsert_guidance_markdown(&paths.claude_memory, &claude_guidance_block())?;
        eprintln!(
            "Claude guidance written to {}",
            paths.claude_memory.display()
        );
    }

    if matches!(client, InitClient::Codex | InitClient::All) {
        register_codex_mcp_server(&paths.codex_config, &binary_path_str)?;
        eprintln!(
            "Codex MCP server registered in {}",
            paths.codex_config.display()
        );

        upsert_guidance_markdown(&paths.codex_agents, &codex_guidance_block())?;
        eprintln!("Codex guidance written to {}", paths.codex_agents.display());
        eprintln!(
            "note: Codex gets MCP tools only. No documented Codex hook/session-start enrichment interface was found, so transparent enrichment remains Claude-only."
        );
    }

    if matches!(client, InitClient::Gemini | InitClient::All) {
        register_gemini_mcp_server(&paths.gemini_settings, &binary_path_str)?;
        eprintln!(
            "Gemini MCP server registered in {}",
            paths.gemini_settings.display()
        );

        upsert_guidance_markdown(&paths.gemini_memory, &gemini_guidance_block())?;
        eprintln!(
            "Gemini guidance written to {}",
            paths.gemini_memory.display()
        );
    }

    if matches!(client, InitClient::KiloCode | InitClient::All) {
        register_kilo_mcp_server(&paths.kilo_vscode_config, &binary_path_str)?;
        eprintln!(
            "Kilo Code MCP server registered in {}",
            paths.kilo_vscode_config.display()
        );
    }

    std::fs::create_dir_all(working_dir.join(".symforge"))
        .with_context(|| format!("creating {}", working_dir.join(".symforge").display()))?;

    eprintln!("symforge init complete");

    Ok(())
}

/// Merge symforge hook entries into `settings_path`, creating it if necessary.
///
/// This is the testable core of `run_init`. Integration tests can pass a temp-dir path
/// instead of the real `~/.claude/settings.json`.
///
/// `binary_path` is the absolute path of the symforge binary.
pub fn merge_hooks_into_settings(
    settings_path: &std::path::Path,
    binary_path: &std::path::Path,
) -> anyhow::Result<()> {
    // Ensure parent dir exists.
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    // Read existing settings or start with empty object.
    let mut settings: Value = if settings_path.exists() {
        let raw = std::fs::read_to_string(settings_path)
            .with_context(|| format!("reading {}", settings_path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", settings_path.display()))?
    } else {
        json!({})
    };

    // Normalise binary path to forward slashes for JSON command strings.
    let binary_str = binary_path.display().to_string().replace('\\', "/");

    // Merge hooks in-place.
    merge_symforge_hooks(&mut settings, &binary_str);

    // Write back.
    let pretty = serde_json::to_string_pretty(&settings)?;
    std::fs::write(settings_path, pretty)
        .with_context(|| format!("writing {}", settings_path.display()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tool name constants
// ---------------------------------------------------------------------------

const SYMFORGE_TOOL_NAMES: &[&str] = &[
    "mcp__symforge__health",
    "mcp__symforge__index_folder",
    "mcp__symforge__get_file_outline",
    "mcp__symforge__get_file_content",
    "mcp__symforge__get_file_tree",
    "mcp__symforge__get_symbol",
    "mcp__symforge__get_symbols",
    "mcp__symforge__get_repo_outline",
    "mcp__symforge__get_repo_map",
    "mcp__symforge__get_file_context",
    "mcp__symforge__get_symbol_context",
    "mcp__symforge__get_context_bundle",
    "mcp__symforge__search_symbols",
    "mcp__symforge__search_text",
    "mcp__symforge__search_files",
    "mcp__symforge__resolve_path",
    "mcp__symforge__find_references",
    "mcp__symforge__find_dependents",
    "mcp__symforge__find_implementations",
    "mcp__symforge__inspect_match",
    "mcp__symforge__analyze_file_impact",
    "mcp__symforge__what_changed",
    "mcp__symforge__get_co_changes",
    "mcp__symforge__diff_symbols",
    "mcp__symforge__explore",
    "mcp__symforge__replace_symbol_body",
    "mcp__symforge__edit_within_symbol",
    "mcp__symforge__insert_symbol",
    "mcp__symforge__delete_symbol",
    "mcp__symforge__batch_edit",
    "mcp__symforge__batch_insert",
    "mcp__symforge__batch_rename",
];

/// Tool names allowed by default for the Kilo Code VS Code extension.
///
/// Kilo Code uses bare tool names (no `mcp__symforge__` prefix).
const KILO_ALWAYS_ALLOW: &[&str] = &[
    "health",
    "get_repo_map",
    "search_symbols",
    "search_text",
    "get_file_context",
    "get_symbol",
    "get_symbols",
    "get_symbol_context",
    "trace_symbol",
    "find_references",
    "explore",
    "get_file_outline",
    "replace_symbol_body",
    "edit_within_symbol",
    "insert_symbol",
    "delete_symbol",
    "batch_edit",
    "batch_insert",
    "batch_rename",
];

/// Tool names registered in `alwaysAllow` for the Claude Code MCP entry in `~/.claude.json`.
///
/// These are bare tool names (no `mcp__symforge__` prefix) — Claude resolves them
/// against the declared server namespace automatically.
const CLAUDE_ALWAYS_ALLOW: &[&str] = &[
    "health",
    "get_repo_map",
    "explore",
    "get_file_content",
    "get_file_context",
    "get_symbol",
    "get_symbol_context",
    "search_symbols",
    "search_text",
    "search_files",
    "find_references",
    "find_dependents",
    "inspect_match",
    "what_changed",
    "analyze_file_impact",
    "diff_symbols",
    "index_folder",
    "replace_symbol_body",
    "edit_within_symbol",
    "insert_symbol",
    "delete_symbol",
    "batch_edit",
    "batch_rename",
    "batch_insert",
];

fn merge_allowed_tools(settings: &mut Value) {
    if !settings["allowedTools"].is_array() {
        settings["allowedTools"] = json!([]);
    }
    let allowed = settings["allowedTools"].as_array_mut().expect("is array");
    for tool_name in SYMFORGE_TOOL_NAMES {
        let val = Value::String(tool_name.to_string());
        if !allowed.contains(&val) {
            allowed.push(val);
        }
    }
}

// ---------------------------------------------------------------------------
// Core merge logic (pub for unit testing)
// ---------------------------------------------------------------------------

/// Merge symforge hook entries into an existing `settings` Value in-place.
///
/// `binary_path` is the absolute path of the symforge binary (already
/// normalised to forward-slash on Windows).
pub fn merge_symforge_hooks(settings: &mut Value, binary_path: &str) {
    // Ensure `hooks` key is an object.
    if !settings["hooks"].is_object() {
        settings["hooks"] = json!({});
    }

    // Build fresh symforge entries.
    let post_tool_use_entries = build_post_tool_use_entries(binary_path);
    let pre_tool_use_entries = build_pre_tool_use_entries(binary_path);
    let session_start_entries = build_session_start_entries(binary_path);
    let user_prompt_submit_entries = build_user_prompt_submit_entries(binary_path);

    {
        let hooks = settings["hooks"]
            .as_object_mut()
            .expect("hooks is an object");
        merge_event_entries(hooks, "PostToolUse", post_tool_use_entries);
        merge_event_entries(hooks, "PreToolUse", pre_tool_use_entries);
        merge_event_entries(hooks, "SessionStart", session_start_entries);
        merge_event_entries(hooks, "UserPromptSubmit", user_prompt_submit_entries);
    }

    merge_allowed_tools(settings);
}

// ---------------------------------------------------------------------------
// Entry builders
// ---------------------------------------------------------------------------

fn build_post_tool_use_entries(binary_path: &str) -> Vec<Value> {
    vec![json!({
        "matcher": "Read|Edit|Write|Grep",
        "hooks": [{"type": "command", "command": format!("{binary_path} hook"), "timeout": 5}]
    })]
}

fn build_pre_tool_use_entries(binary_path: &str) -> Vec<Value> {
    // One entry per tool so matchers are specific. The pre-tool handler reads
    // the tool_name from stdin and outputs a suggestion — no sidecar needed.
    vec![
        json!({
            "matcher": "Grep",
            "hooks": [{"type": "command", "command": format!("{binary_path} hook pre-tool"), "timeout": 2}]
        }),
        json!({
            "matcher": "Read",
            "hooks": [{"type": "command", "command": format!("{binary_path} hook pre-tool"), "timeout": 2}]
        }),
        json!({
            "matcher": "Glob",
            "hooks": [{"type": "command", "command": format!("{binary_path} hook pre-tool"), "timeout": 2}]
        }),
        json!({
            "matcher": "Edit",
            "hooks": [{"type": "command", "command": format!("{binary_path} hook pre-tool"), "timeout": 2}]
        }),
    ]
}

fn build_session_start_entries(binary_path: &str) -> Vec<Value> {
    vec![json!({
        "matcher": "startup|resume",
        "hooks": [{"type": "command", "command": format!("{binary_path} hook session-start"), "timeout": 5}]
    })]
}

fn build_user_prompt_submit_entries(binary_path: &str) -> Vec<Value> {
    vec![json!({
        "hooks": [{"type": "command", "command": format!("{binary_path} hook prompt-submit"), "timeout": 5}]
    })]
}

// ---------------------------------------------------------------------------
// Merge helpers
// ---------------------------------------------------------------------------

/// Returns `true` if a hook entry array contains a symforge or legacy tokenizor hook command.
///
/// The binary may be named `symforge`, `symforge.exe`, or legacy `tokenizor`/`tokenizor-mcp`
/// (with optional `.exe`), so we check for "symforge" OR "tokenizor" anywhere in the command
/// AND " hook" as the subcommand indicator.
fn is_symforge_entry(entry: &Value) -> bool {
    if let Some(hooks) = entry["hooks"].as_array() {
        hooks.iter().any(|h| {
            h["command"]
                .as_str()
                .map(|cmd| {
                    (cmd.contains("symforge") || cmd.contains("tokenizor")) && cmd.contains(" hook")
                })
                .unwrap_or(false)
        })
    } else {
        false
    }
}

/// Merge `new_entries` into the `event_key` array of the hooks object.
///
/// Existing symforge/tokenizor entries (identified by `is_symforge_entry`) are filtered
/// out before appending the fresh entries, which achieves idempotency.
fn merge_event_entries(
    hooks: &mut serde_json::Map<String, Value>,
    event_key: &str,
    new_entries: Vec<Value>,
) {
    let existing: Vec<Value> = hooks
        .get(event_key)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Keep only non-symforge entries.
    let mut retained: Vec<Value> = existing
        .into_iter()
        .filter(|e| !is_symforge_entry(e))
        .collect();

    // Append fresh symforge entries at the end.
    retained.extend(new_entries);

    hooks.insert(event_key.to_string(), Value::Array(retained));
}

/// Register symforge as an MCP server in `~/.claude.json` using the absolute binary path.
///
/// This ensures Claude Code launches the native binary directly — no shell, no .cmd wrapper,
/// no Node.js intermediary. Works on all platforms.
pub fn register_mcp_server(
    claude_json_path: &std::path::Path,
    binary_path: &str,
) -> anyhow::Result<()> {
    let mut config: Value = if claude_json_path.exists() {
        let raw = std::fs::read_to_string(claude_json_path)
            .with_context(|| format!("reading {}", claude_json_path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", claude_json_path.display()))?
    } else {
        json!({})
    };

    // Use backslashes on Windows for the command path (Claude Code spawns natively, not via shell).
    let command_path = native_command_path(binary_path);

    if !config["mcpServers"].is_object() {
        config["mcpServers"] = json!({});
    }

    let always_allow: Vec<Value> = CLAUDE_ALWAYS_ALLOW
        .iter()
        .map(|s| Value::String(s.to_string()))
        .collect();

    config["mcpServers"]["symforge"] = json!({
        "command": command_path,
        "args": [],
        "disabled": false,
        "alwaysAllow": always_allow
    });

    let pretty = serde_json::to_string_pretty(&config)?;
    std::fs::write(claude_json_path, pretty)
        .with_context(|| format!("writing {}", claude_json_path.display()))?;

    Ok(())
}

/// Register symforge as an MCP server in `~/.codex/config.toml`.
///
/// Codex stores MCP servers under `[mcp_servers.<name>]` tables in TOML.
/// We update only the `symforge` entry and preserve the rest of the file.
pub fn register_codex_mcp_server(
    codex_config_path: &std::path::Path,
    binary_path: &str,
) -> anyhow::Result<()> {
    if let Some(parent) = codex_config_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    let raw = if codex_config_path.exists() {
        std::fs::read_to_string(codex_config_path)
            .with_context(|| format!("reading {}", codex_config_path.display()))?
    } else {
        String::new()
    };

    let mut config = if raw.trim().is_empty() {
        DocumentMut::new()
    } else {
        raw.parse::<DocumentMut>()
            .with_context(|| format!("parsing {}", codex_config_path.display()))?
    };

    merge_symforge_codex_server(&mut config, binary_path);

    std::fs::write(codex_config_path, config.to_string())
        .with_context(|| format!("writing {}", codex_config_path.display()))?;

    Ok(())
}

fn merge_symforge_codex_server(config: &mut DocumentMut, binary_path: &str) {
    if !config.as_table().contains_key("mcp_servers") || !config["mcp_servers"].is_table() {
        config["mcp_servers"] = Item::Table(Table::new());
    }

    let mcp_servers = config["mcp_servers"]
        .as_table_mut()
        .expect("mcp_servers must be a table");

    if !mcp_servers.contains_key("symforge") || !mcp_servers["symforge"].is_table() {
        mcp_servers.insert("symforge", Item::Table(Table::new()));
    }

    let symforge = mcp_servers["symforge"]
        .as_table_mut()
        .expect("symforge server entry must be a table");

    symforge["command"] = value(native_command_path(binary_path));
    symforge["startup_timeout_sec"] = value(CODEX_STARTUP_TIMEOUT_SEC);
    symforge["tool_timeout_sec"] = value(CODEX_TOOL_TIMEOUT_SEC);

    let mut allow_array = Array::new();
    for tool_name in SYMFORGE_TOOL_NAMES {
        // Codex uses plain tool names without mcp__ prefix
        let short_name = tool_name
            .strip_prefix("mcp__symforge__")
            .unwrap_or(tool_name);
        allow_array.push(short_name);
    }
    symforge["allowed_tools"] = value(allow_array);

    merge_codex_project_doc_fallbacks(config);
}

fn merge_codex_project_doc_fallbacks(config: &mut DocumentMut) {
    let key = "project_doc_fallback_filenames";
    if !config.as_table().contains_key(key) || !config[key].is_array() {
        let mut fallbacks = Array::default();
        fallbacks.push("CLAUDE.md");
        config[key] = value(fallbacks);
        return;
    }

    let fallbacks = config[key]
        .as_array_mut()
        .expect("project_doc_fallback_filenames must be an array");
    let has_claude_md = fallbacks
        .iter()
        .any(|entry| entry.as_str() == Some("CLAUDE.md"));
    if !has_claude_md {
        fallbacks.push("CLAUDE.md");
    }
}

/// Register symforge as an MCP server in `~/.gemini/settings.json`.
///
/// Gemini CLI stores MCP servers under `mcpServers` in a JSON settings file.
/// We update only the `symforge` entry and preserve the rest of the file.
pub fn register_gemini_mcp_server(
    gemini_settings_path: &std::path::Path,
    binary_path: &str,
) -> anyhow::Result<()> {
    if let Some(parent) = gemini_settings_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    let mut config: Value = if gemini_settings_path.exists() {
        let raw = std::fs::read_to_string(gemini_settings_path)
            .with_context(|| format!("reading {}", gemini_settings_path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", gemini_settings_path.display()))?
    } else {
        json!({})
    };

    let command_path = native_command_path(binary_path);

    if !config["mcpServers"].is_object() {
        config["mcpServers"] = json!({});
    }
    config["mcpServers"]["symforge"] = json!({
        "command": command_path,
        "args": [],
        "timeout": 120000,
        "trust": true
    });

    let pretty = serde_json::to_string_pretty(&config)?;
    std::fs::write(gemini_settings_path, pretty)
        .with_context(|| format!("writing {}", gemini_settings_path.display()))?;
    Ok(())
}

/// Register symforge as an MCP server in `.kilocode/mcp.json` (workspace-local).
///
/// Kilo Code (VS Code extension) stores MCP servers under `mcpServers` in a JSON
/// config file. Unlike Claude/Codex/Gemini, this file lives in the project directory
/// rather than the user's home directory.
pub fn register_kilo_mcp_server(
    kilo_config_path: &std::path::Path,
    binary_path: &str,
) -> anyhow::Result<()> {
    if let Some(parent) = kilo_config_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    let mut config: Value = if kilo_config_path.exists() {
        let raw = std::fs::read_to_string(kilo_config_path)
            .with_context(|| format!("reading {}", kilo_config_path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", kilo_config_path.display()))?
    } else {
        json!({})
    };

    let command_path = native_command_path(binary_path);

    if !config["mcpServers"].is_object() {
        config["mcpServers"] = json!({});
    }

    let always_allow: Vec<Value> = KILO_ALWAYS_ALLOW
        .iter()
        .map(|s| Value::String(s.to_string()))
        .collect();

    config["mcpServers"]["symforge"] = json!({
        "command": command_path,
        "args": ["--stdio"],
        "alwaysAllow": always_allow
    });

    let pretty = serde_json::to_string_pretty(&config)?;
    std::fs::write(kilo_config_path, pretty)
        .with_context(|| format!("writing {}", kilo_config_path.display()))?;
    Ok(())
}

fn upsert_guidance_markdown(path: &std::path::Path, guidance_block: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    let existing = if path.exists() {
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?
    } else {
        String::new()
    };

    let merged = upsert_markdown_block(&existing, guidance_block);
    std::fs::write(path, merged).with_context(|| format!("writing {}", path.display()))?;

    Ok(())
}

fn upsert_markdown_block(existing: &str, guidance_block: &str) -> String {
    // Try new marker first, then fall back to legacy marker for backward compat.
    if let Some(start) = existing.find(SYMFORGE_GUIDANCE_START)
        && let Some(end_marker_start) = existing[start..].find(SYMFORGE_GUIDANCE_END)
    {
        let end = start + end_marker_start + SYMFORGE_GUIDANCE_END.len();
        let mut merged = String::new();
        merged.push_str(&existing[..start]);
        merged.push_str(guidance_block);
        merged.push_str(&existing[end..]);
        return merged;
    }

    // Backward compat: detect and replace legacy TOKENIZOR markers.
    if let Some(start) = existing.find(LEGACY_GUIDANCE_START)
        && let Some(end_marker_start) = existing[start..].find(LEGACY_GUIDANCE_END)
    {
        let end = start + end_marker_start + LEGACY_GUIDANCE_END.len();
        let mut merged = String::new();
        merged.push_str(&existing[..start]);
        merged.push_str(guidance_block);
        merged.push_str(&existing[end..]);
        return merged;
    }

    if existing.trim().is_empty() {
        return format!("{guidance_block}\n");
    }

    let mut merged = existing.trim_end_matches(['\r', '\n']).to_string();
    merged.push_str("\n\n");
    merged.push_str(guidance_block);
    merged.push('\n');
    merged
}

fn claude_guidance_block() -> String {
    format!(
        "{SYMFORGE_GUIDANCE_START}\n\
## SymForge MCP — Code Intelligence\n\
\n\
SymForge MCP is installed and active. It provides indexed code search, symbol extraction, and structural analysis that is faster and more token-efficient than raw file operations.\n\
\n\
### Decision Rules\n\
\n\
1. **Before reading a file**, call `get_file_context` — it returns the file's symbol outline, imports, and references, saving 70-95% of tokens vs reading raw source. Only read the full file if you need exact surrounding context that the outline doesn't provide.\n\
\n\
2. **Before grepping**, call `search_text` — it returns matches with enclosing symbol context and file structure awareness. Use `group_by='symbol'` to deduplicate and `follow_refs=true` to inline callers.\n\
\n\
3. **To find a function/class/type**, call `search_symbols` — it searches indexed symbol names across the entire repo in milliseconds.\n\
\n\
4. **To understand a symbol's source**, call `get_symbol` — it returns the full source of a specific function, struct, class, etc. with doc comments.\n\
\n\
5. **To get a project overview**, call `get_repo_map` — it returns a structured outline of the entire repository with file counts, languages, and symbol summaries.\n\
\n\
6. **To trace call relationships**, call `find_references` — it shows callers and callees without scanning files. Use `get_symbol_context` for comprehensive usage analysis.\n\
\n\
7. **To check repo health**, call `health` — it shows index status, file counts, and watcher state.\n\
\n\
8. **After editing a file**, call `analyze_file_impact` — it re-indexes the file and reports affected dependents.\n\
\n\
9. **When resuming work**, call `what_changed` — it shows uncommitted changes so you can pick up where you left off.\n\
\n\
### When to use raw file reads instead\n\
- Reading non-code files (docs, configs) where exact wording matters\n\
- When you need the full file content including whitespace and formatting\n\
- When SymForge tools return an error or the file isn't indexed\n\
\n\
## Tooling Preference\n\
\n\
When SymForge MCP is available, prefer its tools for repository and code\n\
inspection before falling back to direct file reads.\n\
\n\
Use SymForge first for:\n\
- symbol discovery\n\
- text/code search\n\
- file outlines and context\n\
- repository outlines\n\
- targeted symbol/source retrieval\n\
- surgical editing (symbol replacements, renames)\n\
- impact analysis (what changed, what breaks)\n\
- inspection of implementation code under `src/`, `tests/`, and similar\n\
  code-bearing directories\n\
\n\
Preferred tools for reading:\n\
- `search_text` — full-text search with enclosing symbol context\n\
- `search_symbols` — find symbols by name, kind, language, path\n\
- `search_files` — ranked file path discovery, co-change coupling\n\
- `get_file_context` — rich file summary with outline, imports, consumers\n\
- `get_file_content` — read files with line ranges or around a symbol\n\
- `get_repo_map` — repository overview at adjustable detail levels\n\
- `get_symbol` — look up symbols by name, batch mode supported\n\
- `get_symbol_context` — symbol body + callers + callees + type deps\n\
- `find_references` — call sites, imports, type usages, implementations\n\
- `find_dependents` — file-level dependency graph\n\
- `inspect_match` — deep-dive a search match with full symbol context\n\
- `analyze_file_impact` — re-read file, update index, report impact\n\
- `what_changed` — files changed since timestamp, ref, or uncommitted\n\
- `diff_symbols` — symbol-level diff between git refs\n\
- `explore` — concept-driven exploration across the codebase\n\
\n\
Preferred tools for editing:\n\
- `replace_symbol_body` — replace a symbol's entire definition by name\n\
- `edit_within_symbol` — scoped find-and-replace within a symbol's range\n\
- `insert_symbol` — insert code before or after a named symbol\n\
- `delete_symbol` — remove a symbol and its doc comments by name\n\
- `batch_edit` — multiple symbol-addressed edits atomically across files\n\
- `batch_rename` — rename a symbol and update all references project-wide\n\
- `batch_insert` — insert code before/after multiple symbols across files\n\
\n\
Default rule:\n\
- use SymForge to narrow and target code inspection first\n\
- use direct file reads only when exact full-file source or surrounding\n\
  context is still required after tool-based narrowing\n\
- use SymForge editing tools (`replace_symbol_body`, `batch_edit`,\n\
  `edit_within_symbol`) over text-based find-and-replace whenever\n\
  possible to ensure structural integrity and automatic re-indexing\n\
\n\
Direct file reads are still appropriate for:\n\
- exact document text in `docs/` or planning artifacts where literal\n\
  wording matters\n\
- configuration files where exact raw contents are the point of inspection\n\
\n\
Do not default to broad raw file reads for source-code inspection when\n\
SymForge can answer the question more directly.\n\
{SYMFORGE_GUIDANCE_END}"
    )
}

fn codex_guidance_block() -> String {
    format!(
        "{SYMFORGE_GUIDANCE_START}\n\
## SymForge MCP — Code Intelligence\n\
\n\
SymForge MCP is installed and active. It provides indexed code search, symbol extraction, and structural analysis that is faster and more token-efficient than raw file operations.\n\
\n\
### Decision Rules\n\
\n\
1. **Before reading a file**, call `get_file_context` — it returns the file's symbol outline, imports, and references, saving 70-95% of tokens vs reading raw source. Only read the full file if you need exact surrounding context that the outline doesn't provide.\n\
\n\
2. **Before grepping**, call `search_text` — it returns matches with enclosing symbol context and file structure awareness. Use `group_by='symbol'` to deduplicate and `follow_refs=true` to inline callers.\n\
\n\
3. **To find a function/class/type**, call `search_symbols` — it searches indexed symbol names across the entire repo in milliseconds.\n\
\n\
4. **To understand a symbol's source**, call `get_symbol` — it returns the full source of a specific function, struct, class, etc. with doc comments.\n\
\n\
5. **To get a project overview**, call `get_repo_map` — it returns a structured outline of the entire repository with file counts, languages, and symbol summaries.\n\
\n\
6. **To trace call relationships**, call `find_references` — it shows callers and callees without scanning files. Use `get_symbol_context` for comprehensive usage analysis.\n\
\n\
7. **To check repo health**, call `health` — it shows index status, file counts, and watcher state.\n\
\n\
8. **After editing a file**, call `analyze_file_impact` — it re-indexes the file and reports affected dependents.\n\
\n\
9. **When resuming work**, call `what_changed` — it shows uncommitted changes so you can pick up where you left off.\n\
\n\
### When to use raw file reads instead\n\
- Reading non-code files (docs, configs) where exact wording matters\n\
- When you need the full file content including whitespace and formatting\n\
- When SymForge tools return an error or the file isn't indexed\n\
\n\
Codex is configured to read `CLAUDE.md` project guidance too, so treat project SymForge instructions there as authoritative when `AGENTS.md` is absent.\n\
{SYMFORGE_GUIDANCE_END}"
    )
}

fn gemini_guidance_block() -> String {
    format!(
        "{SYMFORGE_GUIDANCE_START}\n\
## SymForge MCP — Code Intelligence\n\
\n\
SymForge MCP is installed and active. It provides indexed code search, symbol extraction, and structural analysis that is faster and more token-efficient than raw file operations.\n\
\n\
### Decision Rules\n\
\n\
1. **Before reading a file**, call `get_file_context` — it returns the file's symbol outline, imports, and references, saving 70-95% of tokens vs reading raw source. Only read the full file if you need exact surrounding context that the outline doesn't provide.\n\
\n\
2. **Before grepping**, call `search_text` — it returns matches with enclosing symbol context and file structure awareness. Use `group_by='symbol'` to deduplicate and `follow_refs=true` to inline callers.\n\
\n\
3. **To find a function/class/type**, call `search_symbols` — it searches indexed symbol names across the entire repo in milliseconds.\n\
\n\
4. **To understand a symbol's source**, call `get_symbol` — it returns the full source of a specific function, struct, class, etc. with doc comments.\n\
\n\
5. **To get a project overview**, call `get_repo_map` — it returns a structured outline of the entire repository with file counts, languages, and symbol summaries.\n\
\n\
6. **To trace call relationships**, call `find_references` — it shows callers and callees without scanning files. Use `get_symbol_context` for comprehensive usage analysis.\n\
\n\
7. **To check repo health**, call `health` — it shows index status, file counts, and watcher state.\n\
\n\
8. **After editing a file**, call `analyze_file_impact` — it re-indexes the file and reports affected dependents.\n\
\n\
9. **When resuming work**, call `what_changed` — it shows uncommitted changes so you can pick up where you left off.\n\
\n\
### When to use raw file reads instead\n\
- Reading non-code files (docs, configs) where exact wording matters\n\
- When you need the full file content including whitespace and formatting\n\
- When SymForge tools return an error or the file isn't indexed\n\
{SYMFORGE_GUIDANCE_END}"
    )
}

/// Returns the binary path of the currently running symforge executable.
fn discover_binary_path() -> PathBuf {
    match std::env::current_exe() {
        Ok(path) => {
            let s = path.display().to_string();
            // Warn if the binary is running from an unstable location.
            let is_npx_cache = s.contains("_npx") || s.contains("npx-cache");
            let is_node_modules = s.contains("node_modules");
            if is_npx_cache || is_node_modules || s.ends_with(".cmd") {
                eprintln!(
                    "warning: binary is inside node_modules or npx cache ({s}); \
                     updates will fail on Windows. Run: npm install -g symforge && symforge init --client all"
                );
            }
            path
        }
        Err(e) => {
            eprintln!("warning: could not determine symforge binary path: {e}");
            PathBuf::from("symforge")
        }
    }
}

fn native_command_path(binary_path: &str) -> String {
    if cfg!(windows) {
        binary_path.replace('/', "\\")
    } else {
        binary_path.to_string()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FAKE_BINARY: &str = "/usr/local/bin/symforge";

    fn run_merge(initial: Value) -> Value {
        let mut settings = initial;
        merge_symforge_hooks(&mut settings, FAKE_BINARY);
        settings
    }

    // --- test_init_creates_hooks_in_empty_settings ---

    #[test]
    fn test_init_creates_hooks_in_empty_settings() {
        let result = run_merge(json!({}));

        let post = result["hooks"]["PostToolUse"]
            .as_array()
            .expect("PostToolUse must be an array");
        let session = result["hooks"]["SessionStart"]
            .as_array()
            .expect("SessionStart must be an array");
        let prompt = result["hooks"]["UserPromptSubmit"]
            .as_array()
            .expect("UserPromptSubmit must be an array");

        assert_eq!(
            post.len(),
            1,
            "PostToolUse must have 1 entry (single stdin-routed entry)"
        );
        assert_eq!(session.len(), 1, "SessionStart must have 1 entry");
        assert_eq!(prompt.len(), 1, "UserPromptSubmit must have 1 entry");
    }

    #[test]
    fn test_init_entries_have_correct_commands() {
        let result = run_merge(json!({}));

        let post = &result["hooks"]["PostToolUse"];
        let entry = &post[0];
        let cmd = entry["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(
            cmd, "/usr/local/bin/symforge hook",
            "Single PostToolUse hook command must have no subcommand suffix"
        );

        let session = &result["hooks"]["SessionStart"][0];
        let session_cmd = session["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(session_cmd, "/usr/local/bin/symforge hook session-start");

        let prompt = &result["hooks"]["UserPromptSubmit"][0];
        let prompt_cmd = prompt["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(prompt_cmd, "/usr/local/bin/symforge hook prompt-submit");
    }

    #[test]
    fn test_init_new_entry_matcher_includes_write() {
        let result = run_merge(json!({}));
        let matcher = result["hooks"]["PostToolUse"][0]["matcher"]
            .as_str()
            .unwrap();
        assert_eq!(
            matcher, "Read|Edit|Write|Grep",
            "matcher must include Write"
        );
    }

    // --- test_init_preserves_existing_hooks ---

    #[test]
    fn test_init_preserves_existing_hooks() {
        let initial = json!({
            "hooks": {
                "PostToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [{"type": "command", "command": "/some/other/hook bash", "timeout": 10}]
                    }
                ]
            }
        });

        let result = run_merge(initial);
        let post = result["hooks"]["PostToolUse"]
            .as_array()
            .expect("PostToolUse must be an array");

        // 1 existing + 1 symforge = 2 total.
        assert_eq!(
            post.len(),
            2,
            "existing hook + 1 symforge hook = 2 entries; got {post:?}"
        );

        // The first entry is the preserved non-symforge hook.
        let first_cmd = post[0]["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(
            first_cmd, "/some/other/hook bash",
            "non-symforge hook must be preserved"
        );
    }

    // --- test_init_migrates_old_three_entry_format ---

    #[test]
    fn test_init_migrates_old_three_entry_format() {
        // Old 3-entry format from Phase 5 (legacy tokenizor binary name).
        let old_binary = "/usr/local/bin/tokenizor";
        let initial = json!({
            "hooks": {
                "PostToolUse": [
                    {
                        "matcher": "Read",
                        "hooks": [{"type": "command", "command": format!("{old_binary} hook read"), "timeout": 5}]
                    },
                    {
                        "matcher": "Edit|Write",
                        "hooks": [{"type": "command", "command": format!("{old_binary} hook edit"), "timeout": 5}]
                    },
                    {
                        "matcher": "Grep",
                        "hooks": [{"type": "command", "command": format!("{old_binary} hook grep"), "timeout": 5}]
                    }
                ]
            }
        });

        let result = run_merge(initial);
        let post = result["hooks"]["PostToolUse"].as_array().unwrap();

        // All 3 old entries must be replaced by exactly 1 new entry.
        assert_eq!(
            post.len(),
            1,
            "migration must replace 3 old entries with 1 new entry; got {post:?}"
        );

        let cmd = post[0]["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(
            cmd, "/usr/local/bin/symforge hook",
            "migrated entry must use new no-subcommand command format"
        );

        let matcher = post[0]["matcher"].as_str().unwrap();
        assert_eq!(
            matcher, "Read|Edit|Write|Grep",
            "migrated entry must use full matcher"
        );
    }

    // --- test_init_idempotent ---

    #[test]
    fn test_init_idempotent() {
        let mut settings = json!({});
        merge_symforge_hooks(&mut settings, FAKE_BINARY);
        let after_first = settings.clone();

        merge_symforge_hooks(&mut settings, FAKE_BINARY);
        let after_second = settings.clone();

        assert_eq!(
            after_first, after_second,
            "running merge twice must produce identical output (idempotent)"
        );
    }

    #[test]
    fn test_init_idempotent_entry_count() {
        let mut settings = json!({});
        merge_symforge_hooks(&mut settings, FAKE_BINARY);
        let count_first = settings["hooks"]["PostToolUse"].as_array().unwrap().len();

        merge_symforge_hooks(&mut settings, FAKE_BINARY);
        let count_second = settings["hooks"]["PostToolUse"].as_array().unwrap().len();

        assert_eq!(
            count_first, count_second,
            "second merge must not add duplicate symforge entries"
        );
    }

    // --- test_init_replaces_stale_symforge_entries ---

    #[test]
    fn test_init_replaces_stale_symforge_entries() {
        let old_binary = "/old/path/to/symforge";
        let new_binary = "/new/path/to/symforge";

        // Set up settings with the old binary path.
        let initial = json!({
            "hooks": {
                "PostToolUse": [
                    {
                        "matcher": "Read",
                        "hooks": [{"type": "command", "command": format!("{old_binary} hook read"), "timeout": 5}]
                    }
                ]
            }
        });

        let mut settings = initial;
        merge_symforge_hooks(&mut settings, new_binary);

        let post = settings["hooks"]["PostToolUse"].as_array().unwrap();

        // Old entry must be gone.
        let has_old = post.iter().any(|e| {
            e["hooks"][0]["command"]
                .as_str()
                .map(|c| c.contains(old_binary))
                .unwrap_or(false)
        });
        assert!(
            !has_old,
            "stale symforge entry with old binary path must be removed"
        );

        // New entry must be present.
        let has_new = post.iter().any(|e| {
            e["hooks"][0]["command"]
                .as_str()
                .map(|c| c.contains(new_binary))
                .unwrap_or(false)
        });
        assert!(
            has_new,
            "new symforge entry with new binary path must be present"
        );
    }

    // --- is_symforge_entry ---

    #[test]
    fn test_is_symforge_entry_detects_symforge_command() {
        let entry = json!({
            "matcher": "Read",
            "hooks": [{"type": "command", "command": "/path/symforge hook read"}]
        });
        assert!(is_symforge_entry(&entry));
    }

    #[test]
    fn test_is_symforge_entry_detects_legacy_tokenizor_command() {
        let entry = json!({
            "matcher": "Read",
            "hooks": [{"type": "command", "command": "/path/tokenizor hook read"}]
        });
        assert!(
            is_symforge_entry(&entry),
            "must detect legacy tokenizor hook command for backward compat"
        );
    }

    #[test]
    fn test_is_symforge_entry_detects_legacy_tokenizor_mcp_binary() {
        let entry = json!({
            "matcher": "Read|Edit|Write|Grep",
            "hooks": [{"type": "command", "command": "C:/Users/user/node_modules/tokenizor-mcp/bin/tokenizor-mcp.exe hook"}]
        });
        assert!(
            is_symforge_entry(&entry),
            "must detect legacy tokenizor-mcp.exe binary name"
        );
    }

    #[test]
    fn test_is_symforge_entry_ignores_non_symforge() {
        let entry = json!({
            "matcher": "Bash",
            "hooks": [{"type": "command", "command": "/some/other/script bash"}]
        });
        assert!(!is_symforge_entry(&entry));
    }

    #[test]
    fn test_merge_adds_allowed_tools() {
        let mut settings = json!({});
        merge_symforge_hooks(&mut settings, "/usr/bin/symforge");
        let allowed = settings["allowedTools"]
            .as_array()
            .expect("allowedTools should be array");
        assert!(
            allowed
                .iter()
                .any(|v| v.as_str() == Some("mcp__symforge__search_symbols")),
            "should include search_symbols, got: {allowed:?}"
        );
        assert!(
            allowed
                .iter()
                .any(|v| v.as_str() == Some("mcp__symforge__get_symbol")),
            "should include get_symbol"
        );
        let first_len = allowed.len();
        // Should not duplicate on re-run
        merge_symforge_hooks(&mut settings, "/usr/bin/symforge");
        let allowed2 = settings["allowedTools"].as_array().unwrap();
        assert_eq!(first_len, allowed2.len(), "should not duplicate entries");
    }

    #[test]
    fn test_codex_registration_includes_allow_list() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        register_codex_mcp_server(&config_path, "/usr/bin/symforge").unwrap();
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(
            content.contains("search_symbols"),
            "should contain tool names: {content}"
        );
    }

    #[test]
    fn test_gemini_registration_creates_config() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        register_gemini_mcp_server(&settings_path, "/usr/bin/symforge").unwrap();
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let config: Value = serde_json::from_str(&content).unwrap();
        assert!(config["mcpServers"]["symforge"]["command"].is_string());
    }

    #[test]
    fn test_gemini_registration_includes_trust() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        register_gemini_mcp_server(&settings_path, "/usr/bin/symforge").unwrap();
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let config: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            config["mcpServers"]["symforge"]["trust"],
            json!(true),
            "symforge server must have trust: true"
        );
    }

    #[test]
    fn test_gemini_registration_timeout_in_milliseconds() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        register_gemini_mcp_server(&settings_path, "/usr/bin/symforge").unwrap();
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let config: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            config["mcpServers"]["symforge"]["timeout"],
            json!(120000),
            "timeout must be in milliseconds (120000ms = 2 minutes)"
        );
    }

    #[test]
    fn test_gemini_registration_no_allowed_tools_key() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        register_gemini_mcp_server(&settings_path, "/usr/bin/symforge").unwrap();
        let content = std::fs::read_to_string(&settings_path).unwrap();
        let config: Value = serde_json::from_str(&content).unwrap();
        assert!(
            config.get("allowedTools").is_none(),
            "Gemini config must not include allowedTools (Claude-only concept)"
        );
    }

    #[test]
    fn test_gemini_registration_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        register_gemini_mcp_server(&settings_path, "/usr/bin/symforge").unwrap();
        let first = std::fs::read_to_string(&settings_path).unwrap();
        register_gemini_mcp_server(&settings_path, "/usr/bin/symforge").unwrap();
        let second = std::fs::read_to_string(&settings_path).unwrap();
        assert_eq!(
            first, second,
            "running registration twice must produce identical output"
        );
    }
}
