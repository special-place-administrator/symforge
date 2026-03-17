# SymForge Rename — Orchestrator Task

## Objective

Rename "Tokenizor" / "tokenizor" to "SymForge" / "symforge" across the entire codebase. This is a coordinated rename — every reference must be updated in one pass. After this task completes, the project should build, test, and run entirely under the new name with zero references to the old name (except migration/backward-compat logic).

**Use Tokenizor MCP tools** (search_text, search_symbols, get_symbol, batch_rename, batch_edit, replace_symbol_body, etc.) for all codebase navigation and editing. Only fall back to raw file reads when necessary.

---

## Phase 1: Cargo & npm Identity

### 1a. Cargo.toml
- `[package] name` → `symforge`
- `[[bin]] name` → `symforge` (if present)
- Update any `description` field to reference SymForge

### 1b. npm/package.json
- `"name"` → `"symforge"`
- `"description"` → update to reference SymForge
- Binary entry: `"tokenizor-mcp"` → `"symforge"` (the `"bin"` field)
- Update any install scripts that reference `tokenizor-mcp`

### 1c. npm/bin/ scripts
- Rename `tokenizor-mcp` → `symforge` in any launcher scripts
- Update internal references to the binary name

### 1d. npm/scripts/install.js
- `tokenizor-mcp` → `symforge` (binary name)
- `tokenizor` → `symforge` (directory/path references)
- `.tokenizor` → `.symforge` (home directory)
- `TOKENIZOR_HOME` → `SYMFORGE_HOME`

### 1e. .github/.release-please-manifest.json
- Update component name if it references tokenizor

### 1f. Cargo.lock
- Will auto-update after `cargo check` — do NOT manually edit

---

## Phase 2: Environment Variables & Paths

### 2a. src/ — all Rust source files
Search for and replace:
- `TOKENIZOR_AUTO_INDEX` → `SYMFORGE_AUTO_INDEX`
- `TOKENIZOR_CB_THRESHOLD` → `SYMFORGE_CB_THRESHOLD`
- `TOKENIZOR_SIDECAR_BIND` → `SYMFORGE_SIDECAR_BIND`
- `TOKENIZOR_HOME` → `SYMFORGE_HOME`
- `".tokenizor"` (directory name) → `".symforge"`
- `tokenizor-mcp` (binary name in strings) → `symforge`
- `tokenizor_agentic_mcp` (crate name) → `symforge`

### 2b. Backward compatibility / migration
In `src/discovery/mod.rs` or wherever project root discovery happens, add fallback logic:
- Check for `.symforge/` first, fall back to `.tokenizor/` if it exists
- Log a deprecation warning when falling back to `.tokenizor/`

In `src/cli/init.rs` or the npm install script:
- If `~/.tokenizor/` exists and `~/.symforge/` does not, copy/migrate the contents
- Or at minimum, symlink `~/.symforge` → `~/.tokenizor` on first run

---

## Phase 3: MCP Server Identity

### 3a. Protocol — server name
In `src/protocol/mod.rs`, update `ServerInfo` / server name from "tokenizor" to "symforge".

### 3b. Init scripts — all client registrations
In `src/cli/init.rs`:
- `"tokenizor"` MCP server name → `"symforge"` in all JSON/TOML configs
- `mcpServers.tokenizor` → `mcpServers.symforge`
- `mcp_servers.tokenizor` → `mcp_servers.symforge` (Codex TOML)
- `mcp.tokenizor` → `mcp.symforge` (Kilo CLI)
- Hook commands: `tokenizor-mcp.exe` → `symforge.exe` (or `symforge` on Unix)
- `TOKENIZOR_TOOL_NAMES` constant → `SYMFORGE_TOOL_NAMES` (and update all `mcp__tokenizor__` prefixes to `mcp__symforge__`)
- `KILO_ALWAYS_ALLOW` — tool names stay the same (no prefix), but verify

### 3c. Guidance blocks
In `src/cli/init.rs` — `claude_guidance_block()`, `codex_guidance_block()`, `gemini_guidance_block()`:
- Replace all "Tokenizor" → "SymForge"
- Replace all "tokenizor" → "symforge"

### 3d. Hook detection
In `src/cli/init.rs` — `is_tokenizor_entry()`:
- Rename to `is_symforge_entry()`
- Also detect legacy "tokenizor" entries for migration
- Update `TOKENIZOR_GUIDANCE_START` / `TOKENIZOR_GUIDANCE_END` markers → `SYMFORGE_GUIDANCE_START` / `SYMFORGE_GUIDANCE_END`

---

## Phase 4: Sidecar & Daemon

### 4a. Sidecar port files
In `src/sidecar/port_file.rs`:
- `.tokenizor/sidecar.port` → `.symforge/sidecar.port`
- `.tokenizor/sidecar.pid` → `.symforge/sidecar.pid`
- `.tokenizor/sidecar.session` → `.symforge/sidecar.session`
- `ensure_tokenizor_dir()` → `ensure_symforge_dir()`

### 4b. Daemon metadata
In `src/daemon.rs`:
- `tokenizor` daemon directory references → `symforge`
- PID/port files under `SYMFORGE_HOME`

### 4c. Index persistence
In `src/live_index/persist.rs`:
- `.tokenizor/index.bin` → `.symforge/index.bin`

---

## Phase 5: Documentation

### 5a. README.md
- Title: `# Tokenizor MCP` → `# SymForge`
- All body text: "Tokenizor" → "SymForge", "tokenizor" → "symforge"
- Install command: `npm install -g tokenizor-mcp` → `npm install -g symforge`
- Binary references: `tokenizor-mcp` → `symforge`
- Environment variables table: update all `TOKENIZOR_*` → `SYMFORGE_*`
- Home directory: `~/.tokenizor` → `~/.symforge`
- Project directory: `.tokenizor/` → `.symforge/`
- Cargo package name: `tokenizor_agentic_mcp` → `symforge`
- Remove or update the "Rename: Tokenizor → SymForge" section (it's now done)
- Keep the "How we got here" naming story — it's good content

### 5b. CHANGELOG.md
- Add entry for the rename at the top
- Don't rewrite historical entries

### 5c. All docs/ files
- Search and replace "Tokenizor" → "SymForge" and "tokenizor" → "symforge"
- Be careful with historical references in planning docs — add "[formerly Tokenizor]" where appropriate

### 5d. tests/manual/
- Update any test plans referencing tokenizor

---

## Phase 6: Tests

### 6a. Unit tests in src/
- All string literals referencing "tokenizor" → "symforge"
- Test function names containing "tokenizor" → "symforge"
- Constants like `FAKE_BINARY` that reference tokenizor paths

### 6b. Integration tests in tests/
- Same string literal and path updates
- `tests/init_integration.rs` — update expected config paths and MCP server names

### 6c. npm tests
- `npm/tests/` — update binary name and path references

---

## Phase 7: Verification

Run these in order. ALL must pass before the task is complete:

```bash
# 1. Cargo builds clean
cargo check

# 2. No warnings about the rename (pre-existing warnings OK)
cargo check 2>&1 | grep -i tokenizor  # should return nothing

# 3. Unit tests pass
cargo test --lib

# 4. No remaining "tokenizor" references in source (except migration/compat)
grep -r "tokenizor" src/ --include="*.rs" | grep -v "migration\|compat\|legacy\|fallback\|formerly\|TOKENIZOR_GUIDANCE_START\|TOKENIZOR_GUIDANCE_END\|is_tokenizor_entry\|is_symforge_entry"

# 5. No remaining references in npm/
grep -r "tokenizor" npm/ --include="*.js" --include="*.json" | grep -v "migration\|compat\|legacy"

# 6. Build release binary
cargo build --release
```

The grep in step 4 may have legitimate hits for backward-compat code that detects old `.tokenizor/` paths — that's expected. Everything else should be clean.

---

## Phase 8: Post-rename (MANUAL — do NOT automate)

These steps happen AFTER the code rename is committed and verified:

1. **GitHub repo rename**: Settings → General → `tokenizor_agentic_mcp` → `symforge`
2. **Update local git remote**: `git remote set-url origin https://github.com/special-place-administrator/symforge.git`
3. **Push**: `git push origin main`
4. **npm deprecate**: `npm deprecate tokenizor-mcp "Renamed to symforge. Install with: npm install -g symforge"`
5. **Verify npm**: `npm view symforge` should show the new package

**DO NOT** perform Phase 8. Stop after Phase 7 verification and report results. The human will handle the git/npm rename manually.

---

## Important Notes

- **Do NOT rename the GitHub repo** — the human handles that
- **Do NOT push to remote** — the human handles that
- **DO preserve backward compatibility** for `.tokenizor/` → `.symforge/` migration
- **DO keep** the naming story in README ("How we got here" section)
- **DO update** the "How we got here" section to say the rename is complete
- **Commit message format**: `feat!: rename Tokenizor → SymForge` (breaking change)
- This is a **single coordinated commit** — all changes in one commit, not split across phases

---

## GitHub Actions: Add cargo publish step

The release workflow (`.github/workflows/release.yml`) currently publishes to npm but NOT to crates.io. Add a `cargo publish` step that uses the `CARGO_REGISTRY_TOKEN` secret (already configured in GitHub repo secrets).

Add after the npm-publish job or as a parallel job:

```yaml
  cargo-publish:
    needs: [prepare-release]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Publish to crates.io
        run: cargo publish
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
```

---

## Kilo Code Config Update

After the rename, the Kilo Code MCP config (`.kilocode/mcp.json` and `~/.config/kilo/kilo.json`) should reference the new binary and server name:

```json
{
  "mcpServers": {
    "symforge": {
      "command": "C:\\Users\\<you>\\.symforge\\bin\\symforge.exe",
      "args": [],
      "disabled": false,
      "alwaysAllow": [
        "get_file_context", "get_symbol", "get_symbol_context",
        "get_repo_map", "get_file_content", "search_symbols",
        "search_text", "search_files", "find_references",
        "find_dependents", "explore", "inspect_match",
        "health", "index_folder", "what_changed",
        "diff_symbols", "analyze_file_impact",
        "replace_symbol_body", "edit_within_symbol",
        "insert_symbol", "delete_symbol",
        "batch_edit", "batch_rename", "batch_insert"
      ]
    }
  }
}
```

The `register_kilo_mcp_server()` function in `src/cli/init.rs` must generate this format with the new paths. The Kilo CLI config (`~/.config/kilo/kilo.json`) uses a different format — see Phase 3 for details.
