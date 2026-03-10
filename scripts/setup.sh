#!/usr/bin/env bash
# ============================================================================
# Tokenizor MCP — Setup
# ============================================================================
# Usage: bash scripts/setup.sh [--spacetimedb]
#
# Builds the Tokenizor MCP binary and prints the config for your MCP client.
#
# By default uses the local_registry backend (no external dependencies).
# Pass --spacetimedb to also install/start SpacetimeDB and publish the module.
#
# The MCP server is a standard stdio MCP — the CLI spawns it on start and
# kills it on exit. No daemons, no background processes, no hooks needed.
# ============================================================================

set -euo pipefail

# -- Colors ------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
err()   { echo -e "${RED}[ERROR]${NC} $*"; }
step()  { echo -e "\n${BOLD}${CYAN}==> $*${NC}"; }

# -- Parse args ---------------------------------------------------------------
USE_SPACETIMEDB=false
for arg in "$@"; do
    case "$arg" in
        --spacetimedb) USE_SPACETIMEDB=true ;;
        --help|-h)
            echo "Usage: bash scripts/setup.sh [--spacetimedb]"
            echo ""
            echo "  --spacetimedb   Install SpacetimeDB CLI, start runtime, publish module"
            echo "                  (default: use local_registry backend, no SpacetimeDB needed)"
            exit 0
            ;;
        *) err "Unknown argument: $arg"; exit 1 ;;
    esac
done

# -- Resolve project root ----------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# -- Platform detection -------------------------------------------------------
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*|Windows_NT) PLATFORM="windows"; BIN_EXT=".exe" ;;
    *)                                PLATFORM="unix";    BIN_EXT=""     ;;
esac

BINARY_NAME="tokenizor_agentic_mcp${BIN_EXT}"
RELEASE_BINARY="target/release/${BINARY_NAME}"

# ============================================================================
# Step 1: Check Rust toolchain
# ============================================================================
step "Checking Rust toolchain"

if command -v rustc &>/dev/null && command -v cargo &>/dev/null; then
    ok "Rust: $(rustc --version)"
else
    err "Rust toolchain not found. Install from https://rustup.rs"
    exit 1
fi

# ============================================================================
# Step 2: Build release binary
# ============================================================================
step "Building Tokenizor (release mode)"

info "This may take a few minutes on first build..."
cargo build --release 2>&1 | tail -3

if [[ -f "$RELEASE_BINARY" ]]; then
    ok "Binary: $RELEASE_BINARY"
else
    err "Build failed"
    exit 1
fi

BINARY_ABS_PATH="$(cd "$(dirname "$RELEASE_BINARY")" && pwd)/$(basename "$RELEASE_BINARY")"

# ============================================================================
# Step 3 (optional): SpacetimeDB setup
# ============================================================================
if [[ "$USE_SPACETIMEDB" == true ]]; then
    step "Setting up SpacetimeDB"

    # Install CLI if missing
    if ! command -v spacetime &>/dev/null; then
        info "Installing SpacetimeDB CLI..."
        curl -sSf https://install.spacetimedb.com/install.sh | bash
        export PATH="$HOME/.spacetime/bin:$PATH"
        [[ "$PLATFORM" == "windows" ]] && export PATH="$LOCALAPPDATA/SpacetimeDB/bin/current:$PATH"

        if command -v spacetime &>/dev/null; then
            ok "SpacetimeDB CLI installed"
        else
            err "SpacetimeDB CLI installation failed. Install manually: https://spacetimedb.com/install"
            exit 1
        fi
    else
        ok "SpacetimeDB CLI: $(spacetime --version 2>&1 | head -1)"
    fi

    # Start runtime if not running
    ENDPOINT="http://127.0.0.1:3007"
    if ! curl -s --connect-timeout 2 "$ENDPOINT" &>/dev/null; then
        info "Starting SpacetimeDB runtime..."
        spacetime start --edition standalone &>/dev/null &

        RETRIES=30
        while (( RETRIES > 0 )); do
            curl -s --connect-timeout 1 "$ENDPOINT" &>/dev/null && break
            sleep 1
            (( RETRIES-- ))
        done

        if ! curl -s --connect-timeout 2 "$ENDPOINT" &>/dev/null; then
            err "SpacetimeDB failed to start. Try: spacetime start"
            exit 1
        fi
    fi
    ok "SpacetimeDB runtime: $ENDPOINT"

    # Publish module
    info "Publishing module..."
    spacetime publish tokenizor \
        --module-path spacetime/tokenizor \
        --server local \
        --yes \
        --delete-data=on-conflict 2>&1 | tail -3
    ok "Module published"

    BACKEND="spacetimedb"
else
    BACKEND="local_registry"
fi

# ============================================================================
# Step 4: Verify readiness
# ============================================================================
step "Verifying readiness"

TOKENIZOR_CONTROL_PLANE_BACKEND="$BACKEND" "$RELEASE_BINARY" doctor 2>&1 || true

# ============================================================================
# Step 5: Print MCP config
# ============================================================================
step "MCP Client Configuration"

# Build JSON-safe path
if [[ "$PLATFORM" == "windows" ]]; then
    JSON_PATH=$(echo "$BINARY_ABS_PATH" | sed 's|/|\\\\|g; s|^\\\\c\\\\|C:\\\\|; s|^\\\\d\\\\|D:\\\\|')
else
    JSON_PATH="$BINARY_ABS_PATH"
fi

# Build env block based on backend
if [[ "$BACKEND" == "spacetimedb" ]]; then
    ENV_BLOCK=$(cat <<'ENVJSON'
        "TOKENIZOR_CONTROL_PLANE_BACKEND": "spacetimedb",
        "TOKENIZOR_SPACETIMEDB_ENDPOINT": "http://127.0.0.1:3007",
        "TOKENIZOR_SPACETIMEDB_DATABASE": "tokenizor",
        "TOKENIZOR_SPACETIMEDB_MODULE_PATH": "spacetime/tokenizor",
        "TOKENIZOR_SPACETIMEDB_SCHEMA_VERSION": "2"
ENVJSON
)
else
    ENV_BLOCK='        "TOKENIZOR_CONTROL_PLANE_BACKEND": "local_registry"'
fi

echo ""
echo -e "${BOLD}Add this to your MCP client config:${NC}"
echo ""
echo -e "${CYAN}--- Claude Code (.mcp.json) / Cursor (.cursor/mcp.json) / Claude Desktop ---${NC}"
cat <<MCPCONFIG
{
  "mcpServers": {
    "tokenizor": {
      "command": "${JSON_PATH}",
      "args": ["run"],
      "env": {
${ENV_BLOCK}
      }
    }
  }
}
MCPCONFIG

echo ""
echo -e "${BOLD}${GREEN}============================================================================${NC}"
echo -e "${BOLD}${GREEN}  Setup complete!${NC}"
echo -e "${BOLD}${GREEN}============================================================================${NC}"
echo ""
echo -e "  Binary:  ${BOLD}$BINARY_ABS_PATH${NC}"
echo -e "  Backend: ${BOLD}$BACKEND${NC}"
echo ""
echo -e "  The MCP server is a standard stdio process — your CLI manages its lifecycle."
echo -e "  CLI starts it on launch, kills it on exit. No daemons, no hooks needed."
echo ""
echo -e "  To verify:  ${BOLD}TOKENIZOR_CONTROL_PLANE_BACKEND=$BACKEND $RELEASE_BINARY doctor${NC}"
echo ""
