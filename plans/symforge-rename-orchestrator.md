# SymForge Rename — Orchestrator Task

## Objective

Rename "SymForge" / "SymForge" to "SymForge" / "symforge" across the entire codebase. This is a coordinated rename — every reference must be updated in one pass. After this task completes, the project should build, test, and run entirely under the new name with zero references to the old name (except migration/backward-compat logic).

**Use SymForge MCP tools** (search_text, search_symbols, get_symbol, batch_rename, batch_edit, replace_symbol_body, etc.) for all codebase navigation and editing. Only fall back to raw file reads when necessary.

---

## Phase 1: Cargo & npm Identity

### 1a. Cargo.toml
- `[package] name` → `symforge`
- `[[bin]] name` → `symforge` (if present)
- Update any `description` field to reference SymForge

### 1b. npm/package.json
- `"name"` → `"symforge"`
- `"description"` → update to reference SymForge
- Binary entry: `"symforge"` → `"symforge"` (the `"bin"` field)
- Update any install scripts that reference `symforge`

### 1c. npm/bin/ scripts
- Rename `symforge` → `symforge` in any launcher scripts
- Update internal references to the binary name

### 1d. npm/scripts/install.js
- `symforge` → `symforge` (binary name)
- `SymForge` → `symforge` (directory/path references)
- `.symforge` → `.symforge` (home directory)
- `SYMFORGE_HOME` → `SYMFORGE_HOME`

### 1e. .github/.release-please-manifest.json
- Update component name if it references SymForge

### 1f. Cargo.lock
- Will auto-update after `cargo check` — do NOT manually edit

---

## Phase 2: Environment Variables & Paths

### 2a. src/ — all Rust source files
Search for and replace:
- `SYMFORGE_AUTO_INDEX` → `SYMFORGE_AUTO_INDEX`
- `SYMFORGE_CB_THRESHOLD` → `SYMFORGE_CB_THRESHOLD`
- `SYMFORGE_SIDECAR_BIND` → `SYMFORGE_SIDECAR_BIND`
- `SYMFORGE_HOME` → `SYMFORGE_HOME`
- `".symforge"` (directory name) → `".symforge"`
- `symforge` (binary name in strings) → `symforge`
- `symforge` (crate name) → `symforge`

### 2b. Backward compatibility / migration
In `src/discovery/mod.rs` or wherever project root discovery happens, add fallback logic:
- Check for `.symforge/` first, fall back to `.symforge/` if it exists
- Log a deprecation warning when falling back to `.symforge/`

In `src/cli/init.rs` or the npm install script:
- If `~/.symforge/` exists and `~/.symforge/` does not, copy/migrate the contents
- Or at minimum, symlink `~/.symforge` → `~/.symforge` on first run

---

## Phase 3: MCP Server Identity

### 3a. Protocol — server name
In `src/protocol/mod.rs`, update `ServerInfo` / server name from "SymForge" to "symforge".

### 3b. Init scripts — all client registrations
In `src/cli/init.rs`:
- `"SymForge"` MCP server name → `"symforge"` in all JSON/TOML configs
- `mcpServers.symforge` → `mcpServers.symforge`
- `mcp_servers.symforge` → `mcp_servers.symforge` (Codex TOML)
- `mcp.symforge` → `mcp.symforge` (Kilo CLI)
- Hook commands: `symforge.exe` → `symforge.exe` (or `symforge` on Unix)
- `SYMFORGE_TOOL_NAMES` constant → `SYMFORGE_TOOL_NAMES` (and update all `mcp__SYMFORGE__` prefixes to `mcp__symforge__`)
- `KILO_ALWAYS_ALLOW` — tool names stay the same (no prefix), but verify

### 3c. Guidance blocks
In `src/cli/init.rs` — `claude_guidance_block()`, `codex_guidance_block()`, `gemini_guidance_block()`:
- Replace all "SymForge" → "SymForge"
- Replace all "SymForge" → "symforge"

### 3d. Hook detection
In `src/cli/init.rs` — `is_SYMFORGE_entry()`:
- Rename to `is_symforge_entry()`
- Also detect legacy "SymForge" entries for migration
- Update `SYMFORGE_GUIDANCE_START` / `SYMFORGE_GUIDANCE_END` markers → `SYMFORGE_GUIDANCE_START` / `SYMFORGE_GUIDANCE_END`

---

## Phase 4: Sidecar & Daemon

### 4a. Sidecar port files
In `src/sidecar/port_file.rs`:
- `.symforge/sidecar.port` → `.symforge/sidecar.port`
- `.symforge/sidecar.pid` → `.symforge/sidecar.pid`
- `.symforge/sidecar.session` → `.symforge/sidecar.session`
- `ensure_SYMFORGE_dir()` → `ensure_symforge_dir()`

### 4b. Daemon metadata
In `src/daemon.rs`:
- `SymForge` daemon directory references → `symforge`
- PID/port files under `SYMFORGE_HOME`

### 4c. Index persistence
In `src/live_index/persist.rs`:
- `.symforge/index.bin` → `.symforge/index.bin`

---

## Phase 5: Documentation

### 5a. README.md
- Title: `# SymForge MCP` → `# SymForge`
- All body text: "SymForge" → "SymForge", "SymForge" → "symforge"
- Install command: `npm install -g symforge` → `npm install -g symforge`
- Binary references: `symforge` → `symforge`
- Environment variables table: update all `SYMFORGE_*` → `SYMFORGE_*`
- Home directory: `~/.symforge` → `~/.symforge`
- Project directory: `.symforge/` → `.symforge/`
- Cargo package name: `symforge` → `symforge`
- Remove or update the "Rename: SymForge → SymForge" section (it's now done)
- Keep the "How we got here" naming story — it's good content

### 5b. CHANGELOG.md
- Add entry for the rename at the top
- Don't rewrite historical entries

### 5c. All docs/ files
- Search and replace "SymForge" → "SymForge" and "SymForge" → "symforge"
- Be careful with historical references in planning docs — add "[formerly SymForge]" where appropriate

### 5d. tests/manual/
- Update any test plans referencing SymForge

---

## Phase 6: Tests

### 6a. Unit tests in src/
- All string literals referencing "SymForge" → "symforge"
- Test function names containing "SymForge" → "symforge"
- Constants like `FAKE_BINARY` that reference SymForge paths

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
cargo check 2>&1 | grep -i SymForge  # should return nothing

# 3. Unit tests pass
cargo test --lib

# 4. No remaining "SymForge" references in source (except migration/compat)
grep -r "SymForge" src/ --include="*.rs" | grep -v "migration\|compat\|legacy\|fallback\|formerly\|SYMFORGE_GUIDANCE_START\|SYMFORGE_GUIDANCE_END\|is_SYMFORGE_entry\|is_symforge_entry"

# 5. No remaining references in npm/
grep -r "SymForge" npm/ --include="*.js" --include="*.json" | grep -v "migration\|compat\|legacy"

# 6. Build release binary
cargo build --release
```

The grep in step 4 may have legitimate hits for backward-compat code that detects old `.symforge/` paths — that's expected. Everything else should be clean.

---

## Phase 8: Post-rename (MANUAL — do NOT automate)

These steps happen AFTER the code rename is committed and verified:

1. **GitHub repo rename**: Settings → General → `symforge` → `symforge`
2. **Update local git remote**: `git remote set-url origin https://github.com/special-place-administrator/symforge.git`
3. **Push**: `git push origin main`
4. **npm deprecate**: `npm deprecate symforge "Renamed to symforge. Install with: npm install -g symforge"`
5. **Verify npm**: `npm view symforge` should show the new package

**DO NOT** perform Phase 8. Stop after Phase 7 verification and report results. The human will handle the git/npm rename manually.

---

## Important Notes

- **Do NOT rename the GitHub repo** — the human handles that
- **Do NOT push to remote** — the human handles that
- **DO preserve backward compatibility** for `.symforge/` → `.symforge/` migration
- **DO keep** the naming story in README ("How we got here" section)
- **DO update** the "How we got here" section to say the rename is complete
- **Commit message format**: `feat!: rename SymForge → SymForge` (breaking change)
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
